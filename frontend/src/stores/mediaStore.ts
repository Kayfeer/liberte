import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { PeerInfo } from "@/lib/types";
import { MediaSession, type CallSession, type CallSignalEvent } from "@/lib/mediaSession";
import { SfuSession } from "@/lib/sfuSession";
import { useIdentityStore } from "./identityStore";
import * as tauri from "@/lib/tauri";

interface MediaState {
  inCall: boolean;
  channelId: string | null;
  participants: PeerInfo[];
  localStream: MediaStream | null;
  remoteStreams: Record<string, MediaStream>;
  isMuted: boolean;
  isVideoEnabled: boolean;
  busy: boolean;
  error: string | null;
  mode: "mesh" | "sfu";
  unlisteners: UnlistenFn[];
  startCall: (channelId: string) => Promise<void>;
  endCall: () => Promise<void>;
  toggleMute: () => Promise<void>;
  toggleVideo: () => Promise<void>;
  setParticipants: (participants: PeerInfo[]) => void;
  setMode: (mode: "mesh" | "sfu") => Promise<void>;
  retryConnections: () => void;
  setError: (error: string | null) => void;
  setupListeners: () => Promise<void>;
  cleanup: () => void;
}

let session: CallSession | null = null;
let ending: Promise<void> | null = null;
let generation = 0;
let listenerGeneration = 0;
let setupPromise: Promise<void> | null = null;
let backendOperations: Promise<unknown> = Promise.resolve();
let pendingSignals: CallSignalEvent[] = [];
const pendingPeers = new Set<string>();

// Tauri may execute independent IPC requests concurrently. Preserve call-state
// order so a delayed start or control cannot run after its corresponding hangup.
function updateBackend<T>(operation: () => Promise<T>): Promise<T> {
  const pending = backendOperations.then(operation);
  backendOperations = pending.catch(() => {});
  return pending;
}

function clearPendingPeer(userId: string) {
  pendingPeers.delete(userId);
  pendingSignals = pendingSignals.filter((event) => event.sender !== userId);
}
const idle = { inCall: false, channelId: null, participants: [], localStream: null, remoteStreams: {}, isMuted: false, isVideoEnabled: false };
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);

export const useMediaStore = create<MediaState>((set, get) => ({
  ...idle, busy: false, error: null, mode: "mesh", unlisteners: [],

  startCall: async (channelId) => {
    if (get().busy || get().inCall) return;
    const identity = useIdentityStore.getState().identity;
    if (!identity) throw new Error("Identité indisponible.");
    const callGeneration = ++generation;
    const mode = get().mode;
    set({ busy: true, error: null });
    pendingSignals = [];
    pendingPeers.clear();
    let backendStarted = false;
    const current: CallSession = new (mode === "sfu" ? SfuSession : MediaSession)({
      userId: identity.publicKey,
      channelId,
      sendSignal: (target, signalType) => invoke("send_call_signal", { channelId, target, signalType }),
      announce: () => invoke("announce_call"),
      onLocalStream: (localStream) => { if (session === current) set({ localStream }); },
      onRemoteStream: (userId, stream) => {
        if (session !== current) return;
        set((state) => {
          const remoteStreams = { ...state.remoteStreams };
          if (stream) remoteStreams[userId] = stream; else delete remoteStreams[userId];
          return { remoteStreams };
        });
      },
      onPeer: (userId, patch) => {
        if (session !== current) return;
        set((state) => {
          const existing = state.participants.find((peer) => peer.userId === userId);
          const participants = state.participants.filter((peer) => peer.userId !== userId);
          if (patch) participants.push({ userId, displayName: userId.slice(0, 8), isMuted: false, isVideoEnabled: false, state: "connecting", ...existing, ...patch });
          return { participants };
        });
      },
      onError: (error) => { if (session === current) set({ error }); },
      onClosed: (error) => {
        if (session !== current) return;
        void get().endCall().catch(() => {}).finally(() => set({ error }));
      },
      onCameraEnded: () => {
        if (session !== current || !get().isVideoEnabled) return;
        set({ isVideoEnabled: false });
        void updateBackend(() => tauri.toggleVideo(false)).catch((error) => set({ error: errorText(error) }));
      },
    });
    session = current;
    try {
      await get().setupListeners();
      await updateBackend(() => tauri.setCallMode(mode));
      if (generation !== callGeneration) return;
      set({ channelId, participants: [], remoteStreams: {} });
      await current.start();
      if (generation !== callGeneration) return;
      // Set the channel before publication so an immediate answer is accepted.
      await updateBackend(() => generation === callGeneration ? tauri.startCall(channelId) : Promise.resolve());
      backendStarted = true;
      if (generation !== callGeneration) return;
      set({ inCall: true, isMuted: false, isVideoEnabled: false, mode });
      // Presence can arrive while microphone permission or start_call IPC is
      // pending. Negotiate only once Rust accepts outgoing call signals.
      for (const userId of pendingPeers) current.ensurePeer?.(userId);
      pendingPeers.clear();
      for (const event of pendingSignals.splice(0)) {
        void current.receive?.(event).catch((error) => { if (session === current) set({ error: errorText(error) }); });
      }
      current.announce();
    } catch (error) {
      await current.stop();
      if (backendStarted) await updateBackend(() => tauri.endCall()).catch(() => {});
      if (session === current) session = null;
      if (generation === callGeneration) {
        set({ ...idle, error: errorText(error) });
        throw error;
      }
    } finally {
      if (generation === callGeneration) set({ busy: false });
    }
  },

  endCall: () => {
    if (ending) return ending;
    ++generation;
    const current = session;
    session = null;
    pendingSignals = [];
    pendingPeers.clear();
    set({ ...idle, busy: true, error: null });
    const pending = (async () => {
      try {
        await current?.stop();
        await updateBackend(() => tauri.endCall());
      } catch (error) { set({ error: errorText(error) }); throw error; }
      finally { ending = null; set({ busy: false }); }
    })();
    ending = pending;
    return pending;
  },

  toggleMute: async () => {
    if (!session || get().busy) return;
    set({ busy: true, error: null });
    const current = session;
    const desired = !get().isMuted;
    // Silence immediately, even before the IPC roundtrip completes.
    if (!get().isMuted) current.setMuted(true);
    try {
      const isMuted = await updateBackend(() => tauri.toggleMute(desired));
      if (session === current) { current.setMuted(isMuted); set({ isMuted }); }
    } catch (error) {
      if (session === current) { current.setMuted(true); set({ isMuted: true, error: errorText(error) }); }
      throw error;
    } finally { if (session === current) set({ busy: false }); }
  },

  toggleVideo: async () => {
    if (!session || get().busy) return;
    const current = session;
    const enabled = !get().isVideoEnabled;
    set({ busy: true, error: null });
    try {
      await current.setVideoEnabled(enabled);
      if (session !== current) return;
      const isVideoEnabled = await updateBackend(() => tauri.toggleVideo(enabled));
      if (session === current) set({ isVideoEnabled });
    } catch (error) {
      if (session === current) {
        await current.setVideoEnabled(false).catch(() => {});
        set({ isVideoEnabled: false, error: errorText(error) });
      }
      throw error;
    } finally { if (session === current) set({ busy: false }); }
  },

  setParticipants: (participants) => set({ participants }),
  setMode: async (mode) => {
    if (get().inCall || get().busy) throw new Error("Quittez l'appel avant de changer de transport.");
    try { await updateBackend(() => tauri.setCallMode(mode)); set({ mode, error: null }); }
    catch (error) { set({ error: errorText(error) }); throw error; }
  },
  retryConnections: () => { set({ error: null }); session?.retryConnections(); },
  setError: (error) => set({ error }),

  setupListeners: () => {
    if (setupPromise) return setupPromise;
    if (get().unlisteners.length) return Promise.resolve();
    const setupGeneration = listenerGeneration;
    const unlisteners: UnlistenFn[] = [];
    const promise = (async () => {
      try {
        unlisteners.push(await listen<CallSignalEvent>("call-signal", ({ payload }) => {
          const current = session;
          if (!current || get().channelId !== payload.channelId) return;
          if (!get().inCall) {
            if (payload.signalType === "Hangup") clearPendingPeer(payload.sender);
            else if (pendingSignals.length < 256) pendingSignals.push(payload);
            return;
          }
          void current.receive?.(payload).catch((error) => { if (session === current) set({ error: errorText(error) }); });
        }));
        unlisteners.push(await listen<{ channelId: string; userId: string }>("voice-peer-joined", ({ payload }) => {
          if (get().channelId === payload.channelId) {
            if (!get().inCall) { pendingPeers.add(payload.userId); return; }
            try { session?.ensurePeer?.(payload.userId); }
            catch (error) { set({ error: errorText(error) }); }
          }
        }));
        unlisteners.push(await listen<{ channelId: string; userId: string }>("voice-peer-left", ({ payload }) => {
          if (get().channelId === payload.channelId) {
            clearPendingPeer(payload.userId);
            session?.removePeer?.(payload.userId);
          }
        }));
        unlisteners.push(await listen<{ channelId: string; userId: string; muted: boolean }>("voice-peer-muted", ({ payload }) => {
          if (get().channelId === payload.channelId) set((state) => ({ participants: state.participants.map((peer) => peer.userId === payload.userId ? { ...peer, isMuted: payload.muted } : peer) }));
        }));
        // A WebView reload may outlive its JS tracks but leave backend presence
        // active. Reconcile it before accepting the next call.
        if (!session && setupGeneration === listenerGeneration) await updateBackend(() => tauri.endCall());
        if (setupGeneration !== listenerGeneration) unlisteners.forEach((unlisten) => unlisten());
        else set({ unlisteners });
      } catch (error) { unlisteners.forEach((unlisten) => unlisten()); throw error; }
    })();
    setupPromise = promise;
    void promise.finally(() => { if (setupPromise === promise) setupPromise = null; }).catch(() => {});
    return promise;
  },

  cleanup: () => {
    ++listenerGeneration;
    setupPromise = null;
    get().unlisteners.forEach((unlisten) => unlisten());
    set({ unlisteners: [] });
    if (session) void get().endCall().catch(() => {});
  },
}));
