import { beforeEach, describe, expect, it, vi } from "vitest";
import { SfuSession } from "./sfuSession";
import type { AudioCapture, SessionOptions } from "./mediaSession";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), supported: vi.fn(), capture: vi.fn(), captureVideo: vi.fn(), dispose: vi.fn(),
  setKey: vi.fn(), terminate: vi.fn(), register: vi.fn(), release: vi.fn(),
  room: null as MockRoom | null, order: [] as string[], preferences: vi.fn(),
  audioTrack: null as MockLocalTrack | null, videoTrack: null as MockLocalTrack | null,
}));
interface MockLocalTrack {
  mediaStreamTrack: MediaStreamTrack;
  replaceTrack: ReturnType<typeof vi.fn>;
  mute: ReturnType<typeof vi.fn>;
  unmute: ReturnType<typeof vi.fn>;
}
interface MockRoom {
  handlers: Map<string, (...args: unknown[]) => void>;
  localParticipant: { identity: string; isE2EEEnabled: boolean; publishTrack: ReturnType<typeof vi.fn>; unpublishTrack: ReturnType<typeof vi.fn> };
  disconnect: ReturnType<typeof vi.fn>;
  connect: ReturnType<typeof vi.fn>;
}
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("livekit-client/e2ee-worker?worker", () => ({ default: class { terminate = mocks.terminate; } }));
vi.mock("./mediaSession", () => ({
  preferences: mocks.preferences,
  captureAudio: mocks.capture, disposeAudio: mocks.dispose,
  captureVideo: mocks.captureVideo, registerMediaSession: mocks.register, releaseMediaSession: mocks.release,
}));
vi.mock("livekit-client", () => ({
  isE2EESupported: mocks.supported,
  ExternalE2EEKeyProvider: class { setKey = mocks.setKey; },
  LocalAudioTrack: class {
    constructor(public mediaStreamTrack: MediaStreamTrack) { mocks.audioTrack = this; }
    replaceTrack = vi.fn(async (track: MediaStreamTrack) => { this.mediaStreamTrack = track; });
    stop = vi.fn(); mute = vi.fn(); unmute = vi.fn();
  },
  LocalVideoTrack: class {
    constructor(public mediaStreamTrack: MediaStreamTrack) { mocks.videoTrack = this; }
    replaceTrack = vi.fn(async (track: MediaStreamTrack) => { this.mediaStreamTrack = track; });
    mute = vi.fn(); unmute = vi.fn();
    stop() { this.mediaStreamTrack.stop(); }
  },
  Track: { Source: { Microphone: "microphone", Camera: "camera" } },
  RoomEvent: Object.fromEntries([
    "EncryptionError", "ParticipantEncryptionStatusChanged", "ParticipantConnected", "ParticipantDisconnected",
    "TrackPublished", "TrackSubscribed", "TrackUnsubscribed", "TrackMuted", "TrackUnmuted",
    "Reconnecting", "Reconnected", "Moved", "Disconnected",
  ].map((event) => [event, event])),
  Room: class {
    name = "room";
    handlers = new Map();
    remoteParticipants = new Map();
    localParticipant = {
      identity: "a".repeat(64), isE2EEEnabled: false,
      publishTrack: vi.fn(async () => { mocks.order.push("publish"); return { isEncrypted: true }; }),
      unpublishTrack: vi.fn(async () => {}),
    };
    constructor() { mocks.room = this; }
    on(event: string, handler: (...args: unknown[]) => void) { this.handlers.set(event, handler); return this; }
    async setE2EEEnabled(enabled: boolean) {
      mocks.order.push("encrypt"); this.localParticipant.isE2EEEnabled = enabled;
    }
    connect = vi.fn(async () => { mocks.order.push("connect"); });
    disconnect = vi.fn(async () => {});
    removeAllListeners() { this.handlers.clear(); }
  },
}));

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (cause: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}
function mediaTrack(kind: "audio" | "video") {
  return { kind, enabled: true, addEventListener: vi.fn(), stop: vi.fn() };
}
function audioCapture() {
  const raw = mediaTrack("audio");
  const processed = mediaTrack("audio");
  return {
    raw: { getAudioTracks: () => [raw], getTracks: () => [raw] },
    stream: { getAudioTracks: () => [processed], getTracks: () => [processed] },
    gain: { gain: { value: 1 } },
  };
}
function options(): SessionOptions {
  return {
    userId: "a".repeat(64), channelId: "channel", sendSignal: vi.fn(), announce: vi.fn(),
    onLocalStream: vi.fn(), onRemoteStream: vi.fn(), onPeer: vi.fn(), onError: vi.fn(), onCameraEnded: vi.fn(), onClosed: vi.fn(),
  };
}
function emit(event: string, ...args: unknown[]) { mocks.room?.handlers.get(event)?.(...args); }

describe("SFU mandatory encryption", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.order = [];
    mocks.room = null;
    mocks.audioTrack = null; mocks.videoTrack = null;
    mocks.preferences.mockReturnValue({ iceServers: [], inputVolume: 100 });
    mocks.captureVideo.mockReset();
    mocks.dispose.mockImplementation((capture: AudioCapture | null) => {
      capture?.raw.getTracks().forEach((track) => track.stop());
      capture?.stream.getTracks().forEach((track) => track.stop());
    });
    mocks.supported.mockReturnValue(true);
    mocks.invoke.mockResolvedValue({ url: "ws://localhost:7880", token: "token", roomName: "room", encryptionKey: "b".repeat(64) });
    mocks.setKey.mockImplementation(async () => { mocks.order.push("key"); });
    mocks.capture.mockResolvedValue(audioCapture());
    vi.stubGlobal("MediaStream", class { constructor(public tracks: unknown[]) {} });
    Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getUserMedia: vi.fn() } });
  });

  it("refuses unsupported encryption before requesting a token or microphone", async () => {
    mocks.supported.mockReturnValue(false);
    await expect(new SfuSession(options()).start()).rejects.toThrow("chiffrés");
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(mocks.capture).not.toHaveBeenCalled();
  });

  it("honors native engine rejection even when the browser exposes encryption APIs", async () => {
    const reason = "Les appels SFU chiffrés sont indisponibles sous Linux : le moteur WebKitGTK ne garantit pas leur chiffrement. Choisissez P2P direct pour appeler.";
    mocks.supported.mockReturnValue(true);
    mocks.invoke.mockRejectedValueOnce(new Error(reason));
    const handlers = options();
    await expect(new SfuSession(handlers).start()).rejects.toThrow(reason);
    expect(mocks.invoke).toHaveBeenCalledOnce();
    expect(mocks.invoke).toHaveBeenCalledWith("get_sfu_connection", { channelId: "channel" });
    expect(mocks.capture).not.toHaveBeenCalled();
    expect(mocks.captureVideo).not.toHaveBeenCalled();
    expect(mocks.setKey).not.toHaveBeenCalled();
    expect(mocks.room).toBeNull();
    expect(mocks.register).not.toHaveBeenCalled();
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(null);
  });

  it("imports the key and enables encryption before connecting and publishing", async () => {
    const session = new SfuSession(options());
    await session.start();
    expect(mocks.order).toEqual(["key", "encrypt", "connect", "publish"]);
    expect(mocks.room?.connect).toHaveBeenCalledWith("ws://localhost:7880", "token", {
      autoSubscribe: false, rtcConfig: { iceServers: [] },
    });
    await session.stop();
    expect(mocks.terminate).toHaveBeenCalledOnce();
    expect(mocks.dispose).toHaveBeenCalled();
  });

  it("never captures or connects with an invalid channel key", async () => {
    mocks.invoke.mockResolvedValue({ url: "wss://example.test", token: "token", roomName: "room", encryptionKey: "invalid" });
    await expect(new SfuSession(options()).start()).rejects.toThrow("Clé");
    expect(mocks.capture).not.toHaveBeenCalled();
    expect(mocks.room).toBeNull();
  });

  it("rejects unencrypted remote publications before subscribing", async () => {
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    const setSubscribed = vi.fn();
    emit("TrackPublished", { isEncrypted: false, setSubscribed }, { identity: "c".repeat(64) });
    expect(setSubscribed).not.toHaveBeenCalled();
    expect(handlers.onRemoteStream).not.toHaveBeenCalled();
    await vi.waitFor(() => expect(handlers.onClosed).toHaveBeenCalled());
    expect(mocks.room?.disconnect).toHaveBeenCalledOnce();
  });

  it("stops capture and disconnects on encryption downgrade", async () => {
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    emit("ParticipantEncryptionStatusChanged", false, { isLocal: true });
    expect(mocks.dispose).toHaveBeenCalled();
    await vi.waitFor(() => expect(handlers.onClosed).toHaveBeenCalled());
    expect(mocks.room?.disconnect).toHaveBeenCalledOnce();
    await expect(session.setVideoEnabled(true)).rejects.toThrow("terminé");
  });

  it("stops the camera even when unpublishing fails", async () => {
    const handlers = options();
    const video = { kind: "video", enabled: true, stop: vi.fn(), addEventListener: vi.fn() };
    mocks.captureVideo.mockResolvedValue(video);
    const session = new SfuSession(handlers);
    await session.start();
    await session.setVideoEnabled(true);
    mocks.room!.localParticipant.unpublishTrack.mockImplementation(async () => {
      expect(video.stop).toHaveBeenCalledOnce();
      throw new Error("transport unavailable");
    });
    await expect(session.setVideoEnabled(false)).rejects.toThrow("transport unavailable");
    expect(video.enabled).toBe(false);
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(expect.objectContaining({ tracks: expect.arrayContaining([expect.objectContaining({ kind: "audio" })]) }));
    await session.setVideoEnabled(false);
    expect(mocks.captureVideo).toHaveBeenCalledOnce();
    await session.stop();
  });
  it("stops a camera pending publication immediately on hangup", async () => {
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    const raw = mediaTrack("video");
    mocks.captureVideo.mockResolvedValue(raw);
    const publication = deferred<{ isEncrypted: boolean }>();
    mocks.room!.localParticipant.publishTrack.mockReturnValueOnce(publication.promise);
    const enabling = session.setVideoEnabled(true);
    const result = enabling.catch((cause) => cause);
    await vi.waitFor(() => expect(mocks.room!.localParticipant.publishTrack).toHaveBeenCalledTimes(2));
    await session.stop();
    expect(raw.stop).toHaveBeenCalledOnce();
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(null);
    publication.resolve({ isEncrypted: true });
    expect(await result).toBeInstanceOf(Error);
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(null);
  });

  it("stops both microphones when hangup interrupts a pending audio replacement", async () => {
    const current = audioCapture();
    const next = audioCapture();
    mocks.capture.mockResolvedValueOnce(current).mockResolvedValueOnce(next);
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    mocks.preferences.mockReturnValue({ iceServers: [], inputVolume: 100, selectedAudioInput: "next-mic" });
    const replacement = deferred<void>();
    mocks.audioTrack!.replaceTrack.mockReturnValueOnce(replacement.promise);
    const updating = session.applyPreferences().catch((cause) => cause);
    await vi.waitFor(() => expect(mocks.audioTrack!.replaceTrack).toHaveBeenCalledOnce());
    await session.stop();
    expect(current.raw.getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    expect(next.raw.getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    expect(next.stream.getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    replacement.resolve();
    expect(await updating).toBeInstanceOf(Error);
    expect(next.raw.getAudioTracks()[0].stop).toHaveBeenCalledOnce();
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(null);
  });

  it("applies mute to a pending replacement immediately and again after SDK completion", async () => {
    const current = audioCapture();
    const next = audioCapture();
    mocks.capture.mockResolvedValueOnce(current).mockResolvedValueOnce(next);
    const session = new SfuSession(options());
    await session.start();
    mocks.preferences.mockReturnValue({ iceServers: [], inputVolume: 100, selectedAudioInput: "next-mic" });
    const replacement = deferred<void>();
    mocks.audioTrack!.replaceTrack.mockImplementationOnce(async () => {
      await replacement.promise;
      // LiveKit may update its enabled state when completing replacement.
      next.stream.getAudioTracks()[0].enabled = true;
    });
    const updating = session.applyPreferences();
    await vi.waitFor(() => expect(mocks.audioTrack!.replaceTrack).toHaveBeenCalledOnce());
    session.setMuted(true);
    expect(current.raw.getAudioTracks()[0].enabled).toBe(false);
    expect(next.raw.getAudioTracks()[0].enabled).toBe(false);
    expect(next.stream.getAudioTracks()[0].enabled).toBe(false);
    replacement.resolve();
    await updating;
    expect(next.raw.getAudioTracks()[0].enabled).toBe(false);
    expect(next.stream.getAudioTracks()[0].enabled).toBe(false);
    await session.stop();
  });

  it("invalidates a camera permission result when video is disabled while it is pending", async () => {
    const session = new SfuSession(options());
    await session.start();
    const capture = deferred<ReturnType<typeof mediaTrack>>();
    mocks.captureVideo.mockReturnValueOnce(capture.promise);
    const enabling = session.setVideoEnabled(true);
    await vi.waitFor(() => expect(mocks.captureVideo).toHaveBeenCalledOnce());
    await session.setVideoEnabled(false);
    const raw = mediaTrack("video");
    capture.resolve(raw);
    await enabling;
    expect(raw.stop).toHaveBeenCalledOnce();
    expect(mocks.room!.localParticipant.publishTrack).toHaveBeenCalledOnce(); // audio only
    await session.stop();
  });

  it("cuts a pending video publication before its acknowledgement and discards its late result", async () => {
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    const raw = mediaTrack("video");
    mocks.captureVideo.mockResolvedValue(raw);
    const publication = deferred<{ isEncrypted: boolean }>();
    mocks.room!.localParticipant.publishTrack.mockReturnValueOnce(publication.promise);
    const enabling = session.setVideoEnabled(true);
    await vi.waitFor(() => expect(mocks.room!.localParticipant.publishTrack).toHaveBeenCalledTimes(2));
    await session.setVideoEnabled(false);
    expect(raw.stop).toHaveBeenCalledOnce();
    publication.resolve({ isEncrypted: true });
    await enabling;
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(expect.objectContaining({ tracks: [expect.objectContaining({ kind: "audio" })] }));
    expect(mocks.room!.localParticipant.unpublishTrack).toHaveBeenCalledTimes(2);
    await session.stop();
  });

  it("cuts current and pending cameras when video is disabled during replacement", async () => {
    const handlers = options();
    const session = new SfuSession(handlers);
    await session.start();
    const current = mediaTrack("video");
    const next = mediaTrack("video");
    mocks.captureVideo.mockResolvedValueOnce(current).mockResolvedValueOnce(next);
    await session.setVideoEnabled(true);
    mocks.preferences.mockReturnValue({ iceServers: [], inputVolume: 100, selectedVideoInput: "next-camera" });
    const replacement = deferred<void>();
    mocks.videoTrack!.replaceTrack.mockImplementationOnce(async (track: MediaStreamTrack) => {
      await replacement.promise;
      mocks.videoTrack!.mediaStreamTrack = track;
    });
    const updating = session.applyPreferences();
    await vi.waitFor(() => expect(mocks.videoTrack!.replaceTrack).toHaveBeenCalledOnce());
    await session.setVideoEnabled(false);
    expect(current.stop).toHaveBeenCalledOnce();
    expect(next.stop).toHaveBeenCalledOnce();
    replacement.resolve();
    await updating;
    expect(handlers.onLocalStream).toHaveBeenLastCalledWith(expect.objectContaining({ tracks: [expect.objectContaining({ kind: "audio" })] }));
    await session.stop();
  });

});

