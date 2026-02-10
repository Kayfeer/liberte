import { create } from "zustand";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PeerInfo } from "@/lib/types";
import * as tauri from "@/lib/tauri";

interface MediaState {
  inCall: boolean;
  channelId: string | null;
  participants: PeerInfo[];
  isMuted: boolean;
  isVideoEnabled: boolean;
  mode: "mesh" | "sfu";
  unlisteners: UnlistenFn[];

  startCall: (channelId: string) => Promise<void>;
  endCall: () => Promise<void>;
  toggleMute: () => Promise<void>;
  toggleVideo: () => Promise<void>;
  setParticipants: (participants: PeerInfo[]) => void;
  setMode: (mode: "mesh" | "sfu") => Promise<void>;
  setupListeners: () => Promise<void>;
  cleanup: () => void;
}

export const useMediaStore = create<MediaState>((set, get) => ({
  inCall: false,
  channelId: null,
  participants: [],
  isMuted: false,
  isVideoEnabled: false,
  mode: "mesh",
  unlisteners: [],

  startCall: async (channelId: string) => {
    await tauri.startCall(channelId);
    set({ inCall: true, channelId, isMuted: false, participants: [] });
  },

  endCall: async () => {
    await tauri.endCall();
    set({ inCall: false, channelId: null, participants: [], isMuted: false });
  },

  toggleMute: async () => {
    const muted = await tauri.toggleMute();
    set({ isMuted: muted });
  },

  toggleVideo: async () => {
    const enabled = await tauri.toggleVideo();
    set({ isVideoEnabled: enabled });
  },

  setParticipants: (participants) => set({ participants }),

  setMode: async (mode) => {
    await tauri.setCallMode(mode);
    set({ mode });
  },

  setupListeners: async () => {
    const unlisteners: UnlistenFn[] = [];

    unlisteners.push(
      await listen<{ channelId: string; userId: string }>(
        "voice-peer-joined",
        (event) => {
          const { channelId, userId } = event.payload;
          const state = get();
          if (state.channelId !== channelId) return;
          // Add peer if not already present
          if (!state.participants.find((p) => p.userId === userId)) {
            set({
              participants: [
                ...state.participants,
                {
                  userId,
                  displayName: userId.slice(0, 8),
                  isMuted: false,
                  isVideoEnabled: false,
                  state: "connected",
                },
              ],
            });
          }
        }
      )
    );

    unlisteners.push(
      await listen<{ channelId: string; userId: string }>(
        "voice-peer-left",
        (event) => {
          const { channelId, userId } = event.payload;
          const state = get();
          if (state.channelId !== channelId) return;
          set({
            participants: state.participants.filter((p) => p.userId !== userId),
          });
        }
      )
    );

    unlisteners.push(
      await listen<{ channelId: string; userId: string; muted: boolean }>(
        "voice-peer-muted",
        (event) => {
          const { channelId, userId, muted } = event.payload;
          const state = get();
          if (state.channelId !== channelId) return;
          set({
            participants: state.participants.map((p) =>
              p.userId === userId ? { ...p, isMuted: muted } : p
            ),
          });
        }
      )
    );

    set({ unlisteners });
  },

  cleanup: () => {
    const { unlisteners } = get();
    unlisteners.forEach((fn) => fn());
    set({ unlisteners: [] });
  },
}));
