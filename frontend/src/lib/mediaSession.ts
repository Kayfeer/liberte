import type { PeerInfo } from "./types";

export type CallSignal = { Offer: string } | { Answer: string } | { IceCandidate: string } | "Hangup";
export interface CallSignalEvent {
  channelId: string;
  sender: string;
  target: string;
  signalType: CallSignal;
}

export interface Preferences {
  selectedAudioInput: string;
  selectedAudioOutput: string;
  selectedVideoInput: string;
  inputVolume: number;
  outputVolume: number;
  noiseSuppression: boolean;
  echoCancellation: boolean;
  autoGainControl: boolean;
  voiceIsolation: boolean;
  iceServers: RTCIceServer[];
}

const defaults: Preferences = {
  selectedAudioInput: "default", selectedAudioOutput: "default", selectedVideoInput: "",
  inputVolume: 100, outputVolume: 100, noiseSuppression: true,
  echoCancellation: true, autoGainControl: true, voiceIsolation: false, iceServers: [],
};

export function preferences(): Preferences {
  try {
    const saved: Partial<Preferences> = JSON.parse(localStorage.getItem("liberte-media-settings") || "{}");
    return { ...defaults, ...saved, iceServers: Array.isArray(saved.iceServers) ? saved.iceServers : [] };
  } catch { return { ...defaults }; }
}

function audioConstraints(settings: Preferences): MediaTrackConstraints {
  const constraints: MediaTrackConstraints & { voiceIsolation?: boolean } = {
    echoCancellation: settings.echoCancellation,
    noiseSuppression: settings.noiseSuppression,
    autoGainControl: settings.autoGainControl,
  };
  if (settings.selectedAudioInput && settings.selectedAudioInput !== "default") {
    constraints.deviceId = { exact: settings.selectedAudioInput };
  }
  const supported = navigator.mediaDevices.getSupportedConstraints() as MediaTrackSupportedConstraints & { voiceIsolation?: boolean };
  if (settings.voiceIsolation && !supported.voiceIsolation) {
    throw new Error("L'isolation vocale n'est pas prise en charge par ce navigateur.");
  }
  if (supported.voiceIsolation) constraints.voiceIsolation = settings.voiceIsolation;
  return constraints;
}

export interface AudioCapture {
  raw: MediaStream;
  stream: MediaStream;
  context: AudioContext;
  gain: GainNode;
}

export function disposeAudio(capture: AudioCapture | null) {
  if (!capture) return;
  capture.raw.getTracks().forEach((track) => track.stop());
  capture.stream.getTracks().forEach((track) => track.stop());
  void capture.context.close().catch(() => {});
}

export async function captureAudio(settings: Preferences): Promise<AudioCapture> {
  const raw = await navigator.mediaDevices.getUserMedia({ audio: audioConstraints(settings), video: false });
  let context: AudioContext | undefined;
  try {
    context = new AudioContext();
    await context.resume();
    const gain = context.createGain();
    gain.gain.value = Math.max(0, Math.min(2, settings.inputVolume / 100));
    const destination = context.createMediaStreamDestination();
    context.createMediaStreamSource(raw).connect(gain).connect(destination);
    return { raw, stream: destination.stream, context, gain };
  } catch (error) {
    raw.getTracks().forEach((track) => track.stop());
    void context?.close().catch(() => {});
    throw error;
  }
}

export async function captureVideo(settings: Preferences): Promise<MediaStreamTrack> {
  const video: MediaTrackConstraints = { width: { ideal: 1280 }, height: { ideal: 720 }, frameRate: { ideal: 30 } };
  if (settings.selectedVideoInput) video.deviceId = { exact: settings.selectedVideoInput };
  const stream = await navigator.mediaDevices.getUserMedia({ audio: false, video });
  const track = stream.getVideoTracks()[0];
  if (!track) { stream.getTracks().forEach((item) => item.stop()); throw new Error("Caméra indisponible."); }
  return track;
}

const playbackElements = new Map<HTMLMediaElement, boolean>();
type SinkElement = HTMLMediaElement & { setSinkId?: (id: string) => Promise<void> };

async function updatePlayback(element: HTMLMediaElement, local: boolean) {
  const settings = preferences();
  element.muted = local;
  element.volume = Math.max(0, Math.min(1, settings.outputVolume / 100));
  if (local) return;
  const sink = element as SinkElement;
  if (sink.setSinkId) await sink.setSinkId(settings.selectedAudioOutput === "default" ? "" : settings.selectedAudioOutput);
  else if (settings.selectedAudioOutput !== "default" && settings.selectedAudioOutput) {
    throw new Error("La sélection de sortie audio n'est pas prise en charge par ce navigateur.");
  }
}

/** Registers one playback element. The returned cleanup releases its stream. */
export async function configurePlayback(element: HTMLMediaElement, mutedLocal = false): Promise<void> {
  playbackElements.set(element, mutedLocal);
  await updatePlayback(element, mutedLocal);
}

export function releasePlayback(element: HTMLMediaElement) {
  playbackElements.delete(element);
  element.srcObject = null;
}

export interface CallSession {
  start(): Promise<void>;
  stop(): Promise<void>;
  announce(): void;
  setMuted(muted: boolean): void;
  setVideoEnabled(enabled: boolean): Promise<void>;
  applyPreferences(): Promise<void>;
  retryConnections(): void;
  ensurePeer?(userId: string): unknown;
  removePeer?(userId: string): void;
  receive?(event: CallSignalEvent): Promise<void>;
}

let activeSession: CallSession | null = null;
export function registerMediaSession(session: CallSession) { activeSession = session; }
export function releaseMediaSession(session: CallSession) { if (activeSession === session) activeSession = null; }

/** Called by settings after persistence; applies devices and levels to an active call. */
export async function applyMediaPreferences(): Promise<void> {
  await activeSession?.applyPreferences();
  await Promise.all([...playbackElements].map(([element, local]) => updatePlayback(element, local)));
}

interface Peer {
  pc: RTCPeerConnection;
  audio: RTCRtpSender;
  video: RTCRtpSender;
  data: RTCDataChannel;
  stream: MediaStream;
  polite: boolean;
  makingOffer: boolean;
  ignoreOffer: boolean;
  settingAnswer: boolean;
  candidates: RTCIceCandidateInit[];
  inbox: Promise<void>;
  lastSeen: number;
  timeout?: ReturnType<typeof setTimeout>;
}

export interface SessionOptions {
  userId: string;
  channelId: string;
  sendSignal: (target: string, signal: CallSignal) => Promise<void>;
  announce: () => Promise<void>;
  onLocalStream: (stream: MediaStream | null) => void;
  onRemoteStream: (userId: string, stream: MediaStream | null) => void;
  onPeer: (userId: string, patch: Partial<PeerInfo> | null) => void;
  onError: (message: string) => void;
  onCameraEnded: () => void;
  onClosed?: (message: string) => void;
}

export class MediaSession implements CallSession {
  private closed = false;
  private started = false;
  private audio: AudioCapture | null = null;
  private video: MediaStreamTrack | null = null;
  private muted = false;
  private settings = preferences();
  private peers = new Map<string, Peer>();
  private heartbeat?: ReturnType<typeof setInterval>;
  private mutations: Promise<void> = Promise.resolve();

  constructor(private options: SessionOptions) {}

  private error(error: unknown) {
    if (!this.closed) this.options.onError(error instanceof Error ? error.message : String(error));
  }

  async start(): Promise<void> {
    if (this.started || this.closed) throw new Error("Session déjà démarrée ou terminée.");
    if (!navigator.mediaDevices?.getUserMedia || typeof RTCPeerConnection === "undefined") {
      throw new Error("Les appels WebRTC ne sont pas disponibles dans ce navigateur.");
    }
    this.started = true;
    const capture = await captureAudio(this.settings);
    if (this.closed) { disposeAudio(capture); throw new Error("Appel annulé."); }
    this.audio = capture;
    registerMediaSession(this);
    capture.raw.getAudioTracks()[0]?.addEventListener("ended", () => this.error("Le microphone a été déconnecté. Choisissez un périphérique dans les paramètres."));
    this.publishLocalStream();
  }

  /** Start presence only after the backend accepted the call. */
  announce() {
    if (this.closed || this.heartbeat) return;
    const tick = () => {
      void this.options.announce().catch((error) => this.error(error));
      for (const [id, peer] of this.peers) {
        if (Date.now() - peer.lastSeen > 35_000) this.removePeer(id);
      }
    };
    tick();
    this.heartbeat = setInterval(tick, 8_000);
  }

  private publishLocalStream() {
    if (this.closed) return;
    const tracks = [...(this.audio?.stream.getAudioTracks() ?? []), ...(this.video ? [this.video] : [])];
    this.options.onLocalStream(new MediaStream(tracks));
  }

  setMuted(muted: boolean) {
    this.muted = muted;
    this.audio?.raw.getAudioTracks().forEach((track) => { track.enabled = !muted; });
    this.audio?.stream.getAudioTracks().forEach((track) => { track.enabled = !muted; });
    this.broadcastState();
  }

  private mutate(operation: () => Promise<void>): Promise<void> {
    const result = this.mutations.then(async () => { if (!this.closed) await operation(); });
    this.mutations = result.catch(() => {});
    return result;
  }

  private async replaceTrack(kind: "audio" | "video", next: MediaStreamTrack | null, previous: MediaStreamTrack | null) {
    const senders = [...this.peers.values()].map((peer) => peer[kind]);
    const results = await Promise.allSettled(senders.map((sender) => sender.replaceTrack(next)));
    if (results.some((result) => result.status === "rejected")) {
      await Promise.allSettled(senders.map((sender) => sender.replaceTrack(previous)));
      throw new Error("Le changement de périphérique a échoué. Réessayez.");
    }
  }

  setVideoEnabled(enabled: boolean): Promise<void> {
    return this.mutate(async () => {
      if (enabled === !!this.video) return;
      const next = enabled ? await captureVideo(this.settings) : null;
      if (this.closed) { next?.stop(); return; }
      try { await this.replaceTrack("video", next, this.video); }
      catch (error) { next?.stop(); throw error; }
      if (this.closed) { next?.stop(); return; }
      this.video?.stop();
      this.video = next;
      this.watchVideo();
      this.publishLocalStream();
      this.broadcastState();
    });
  }

  private watchVideo() {
    const track = this.video;
    track?.addEventListener("ended", () => {
      if (this.closed || this.video !== track) return;
      void this.setVideoEnabled(false).then(() => this.options.onCameraEnded()).catch((error) => this.error(error));
    }, { once: true });
  }

  applyPreferences(): Promise<void> {
    return this.mutate(async () => {
      const settings = preferences();
      const keys: (keyof Preferences)[] = ["selectedAudioInput", "noiseSuppression", "echoCancellation", "autoGainControl", "voiceIsolation"];
      if (keys.some((key) => settings[key] !== this.settings[key])) {
        const next = await captureAudio(settings);
        if (this.closed) { disposeAudio(next); return; }
        next.raw.getAudioTracks().forEach((track) => { track.enabled = !this.muted; });
        next.stream.getAudioTracks().forEach((track) => { track.enabled = !this.muted; });
        try { await this.replaceTrack("audio", next.stream.getAudioTracks()[0], this.audio?.stream.getAudioTracks()[0] ?? null); }
        catch (error) { disposeAudio(next); throw error; }
        if (this.closed) { disposeAudio(next); return; }
        disposeAudio(this.audio);
        this.audio = next;
      }
      if (this.audio) this.audio.gain.gain.value = Math.max(0, Math.min(2, settings.inputVolume / 100));
      if (this.video && settings.selectedVideoInput !== this.settings.selectedVideoInput) {
        const next = await captureVideo(settings);
        if (this.closed) { next.stop(); return; }
        try { await this.replaceTrack("video", next, this.video); }
        catch (error) { next.stop(); throw error; }
        if (this.closed) { next.stop(); return; }
        this.video.stop();
        this.video = next;
        this.watchVideo();
      }
      const iceChanged = JSON.stringify(settings.iceServers) !== JSON.stringify(this.settings.iceServers);
      this.settings = settings;
      if (iceChanged) for (const peer of this.peers.values()) {
        peer.pc.setConfiguration({ iceServers: settings.iceServers });
        peer.pc.restartIce();
      }
      this.publishLocalStream();
    });
  }

  ensurePeer(userId: string): Peer | undefined {
    if (this.closed || !this.audio || userId === this.options.userId || !/^[0-9a-f]{64}$/i.test(userId)) return;
    const existing = this.peers.get(userId);
    if (existing) { existing.lastSeen = Date.now(); return existing; }
    if (this.peers.size >= 7) { this.error("Les appels P2P sont limités à 8 participants."); return; }
    const pc = new RTCPeerConnection({ iceServers: this.settings.iceServers, bundlePolicy: "max-bundle" });
    const stream = new MediaStream();
    const audioTrack = this.audio.stream.getAudioTracks()[0];
    const audio = pc.addTransceiver(audioTrack, { direction: "sendrecv", streams: [this.audio.stream] }).sender;
    const video = pc.addTransceiver(this.video ?? "video", { direction: "sendrecv" }).sender;
    const data = pc.createDataChannel("liberte-media-state", { negotiated: true, id: 0 });
    const peer: Peer = {
      pc, audio, video, data, stream, polite: this.options.userId > userId,
      makingOffer: false, ignoreOffer: false, settingAnswer: false, candidates: [],
      inbox: Promise.resolve(), lastSeen: Date.now(),
    };
    this.peers.set(userId, peer);
    this.options.onPeer(userId, { state: "connecting", isMuted: false, isVideoEnabled: false });
    pc.onicecandidate = ({ candidate }) => {
      if (candidate && !this.closed) void this.options.sendSignal(userId, { IceCandidate: JSON.stringify(candidate.toJSON()) }).catch((error) => this.error(error));
    };
    pc.onnegotiationneeded = async () => {
      // One deterministic initial offer prevents redundant m-lines on initial glare.
      if (this.closed || (peer.polite && !pc.remoteDescription)) return;
      try {
        peer.makingOffer = true;
        await pc.setLocalDescription();
        if (pc.localDescription?.type === "offer") await this.options.sendSignal(userId, { Offer: pc.localDescription.sdp });
      } catch (error) { this.error(error); }
      finally { peer.makingOffer = false; }
    };
    pc.ontrack = ({ track }) => {
      stream.addTrack(track);
      this.options.onRemoteStream(userId, stream);
      if (track.kind === "video") {
        const refresh = () => this.options.onPeer(userId, { isVideoEnabled: !track.muted && track.readyState === "live" });
        track.addEventListener("unmute", refresh);
        track.addEventListener("mute", refresh);
        track.addEventListener("ended", refresh);
        refresh();
      }
    };
    pc.onconnectionstatechange = () => {
      const state = pc.connectionState;
      if (this.closed || !this.peers.has(userId)) return;
      this.options.onPeer(userId, { state: state === "new" ? "connecting" : state === "closed" ? "disconnected" : state });
      clearTimeout(peer.timeout);
      if (state === "failed") this.error("Connexion média impossible. Vérifiez la configuration STUN/TURN dans les paramètres.");
      if (state === "disconnected") peer.timeout = setTimeout(() => { if (!this.closed) pc.restartIce(); }, 5_000);
    };
    data.onopen = () => this.sendState(peer);
    data.onmessage = (event) => {
      if (typeof event.data !== "string" || event.data.length > 256) return;
      try {
        const value = JSON.parse(event.data);
        if (typeof value.muted === "boolean" && typeof value.video === "boolean") {
          this.options.onPeer(userId, { isMuted: value.muted, isVideoEnabled: value.video });
        }
      } catch { /* Ignore malformed state from peers. */ }
    };
    peer.timeout = setTimeout(() => {
      if (!this.closed && pc.connectionState !== "connected") {
        this.options.onPeer(userId, { state: "failed" });
        this.error("L'autre participant ne répond pas. Vérifiez la connexion et les paramètres STUN/TURN.");
      }
    }, 25_000);
    return peer;
  }

  retryConnections() {
    for (const peer of this.peers.values()) peer.pc.restartIce();
    void this.options.announce().catch((error) => this.error(error));
  }

  /** Browser RTC statistics for connection diagnostics and integration tests. */
  getStats(): Promise<RTCStatsReport[]> {
    return Promise.all([...this.peers.values()].map((peer) => peer.pc.getStats()));
  }

  async receive(event: CallSignalEvent): Promise<void> {
    if (this.closed || event.channelId !== this.options.channelId || event.target !== this.options.userId) return;
    if (event.signalType === "Hangup") { this.removePeer(event.sender); return; }
    const peer = this.ensurePeer(event.sender);
    if (!peer) return;
    const signal = event.signalType;
    const task = peer.inbox.then(async () => {
      if (this.closed || !this.peers.has(event.sender)) return;
      if ("IceCandidate" in signal) {
        if (signal.IceCandidate.length > 8192) return;
        const candidate: RTCIceCandidateInit = JSON.parse(signal.IceCandidate);
        if (peer.ignoreOffer) return;
        if (!peer.pc.remoteDescription) {
          if (peer.candidates.length < 128) peer.candidates.push(candidate);
        } else await peer.pc.addIceCandidate(candidate);
        return;
      }
      const type = "Offer" in signal ? "offer" : "answer";
      const sdp = "Offer" in signal ? signal.Offer : signal.Answer;
      if (!sdp || sdp.length > 128 * 1024) throw new Error("Description média invalide.");
      const ready = !peer.makingOffer && (peer.pc.signalingState === "stable" || peer.settingAnswer);
      const collision = type === "offer" && !ready;
      peer.ignoreOffer = !peer.polite && collision;
      if (peer.ignoreOffer) { peer.candidates = []; return; }
      peer.settingAnswer = type === "answer";
      try { await peer.pc.setRemoteDescription({ type, sdp }); }
      finally { peer.settingAnswer = false; }
      for (const candidate of peer.candidates.splice(0)) await peer.pc.addIceCandidate(candidate);
      if (type === "offer") {
        await peer.pc.setLocalDescription();
        if (peer.pc.localDescription) await this.options.sendSignal(event.sender, { Answer: peer.pc.localDescription.sdp });
      }
    });
    peer.inbox = task.catch((error) => this.error(error));
    await peer.inbox;
  }

  private sendState(peer: Peer) {
    if (peer.data.readyState === "open") peer.data.send(JSON.stringify({ muted: this.muted, video: !!this.video }));
  }
  private broadcastState() { this.peers.forEach((peer) => this.sendState(peer)); }

  removePeer(userId: string) {
    const peer = this.peers.get(userId);
    if (!peer) return;
    this.peers.delete(userId);
    clearTimeout(peer.timeout);
    peer.pc.onconnectionstatechange = null;
    peer.pc.ontrack = null;
    peer.pc.onicecandidate = null;
    peer.pc.onnegotiationneeded = null;
    peer.data.close();
    peer.pc.close();
    peer.stream.getTracks().forEach((track) => track.stop());
    this.options.onRemoteStream(userId, null);
    this.options.onPeer(userId, null);
  }

  async stop(): Promise<void> {
    if (this.closed) return;
    this.closed = true;
    releaseMediaSession(this);
    clearInterval(this.heartbeat);
    const hangups = [...this.peers.keys()].map((userId) => this.options.sendSignal(userId, "Hangup"));
    for (const userId of [...this.peers.keys()]) this.removePeer(userId);
    disposeAudio(this.audio);
    this.audio = null;
    this.video?.stop();
    this.video = null;
    this.options.onLocalStream(null);
    await Promise.allSettled(hangups);
  }
}
