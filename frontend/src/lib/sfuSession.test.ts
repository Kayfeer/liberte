import { beforeEach, describe, expect, it, vi } from "vitest";
import { SfuSession } from "./sfuSession";
import type { SessionOptions } from "./mediaSession";

const mocks = vi.hoisted(() => ({
  invoke: vi.fn(), supported: vi.fn(), capture: vi.fn(), captureVideo: vi.fn(), dispose: vi.fn(),
  setKey: vi.fn(), terminate: vi.fn(), register: vi.fn(), release: vi.fn(),
  room: null as MockRoom | null, order: [] as string[],
}));
interface MockRoom {
  handlers: Map<string, (...args: unknown[]) => void>;
  localParticipant: { identity: string; isE2EEEnabled: boolean; publishTrack: ReturnType<typeof vi.fn>; unpublishTrack: ReturnType<typeof vi.fn> };
  disconnect: ReturnType<typeof vi.fn>;
  connect: ReturnType<typeof vi.fn>;
}
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("livekit-client/e2ee-worker?worker", () => ({ default: class { terminate = mocks.terminate; } }));
vi.mock("./mediaSession", () => ({
  preferences: () => ({ iceServers: [], inputVolume: 100 }),
  captureAudio: mocks.capture, disposeAudio: mocks.dispose,
  captureVideo: mocks.captureVideo, registerMediaSession: mocks.register, releaseMediaSession: mocks.release,
}));
vi.mock("livekit-client", () => ({
  isE2EESupported: mocks.supported,
  ExternalE2EEKeyProvider: class { setKey = mocks.setKey; },
  LocalAudioTrack: class {
    constructor(public mediaStreamTrack: MediaStreamTrack) {}
    stop = vi.fn(); mute = vi.fn(); unmute = vi.fn();
  },
  LocalVideoTrack: class {
    constructor(public mediaStreamTrack: MediaStreamTrack) {}
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
    mocks.supported.mockReturnValue(true);
    mocks.invoke.mockResolvedValue({ url: "ws://localhost:7880", token: "token", roomName: "room", encryptionKey: "b".repeat(64) });
    mocks.setKey.mockImplementation(async () => { mocks.order.push("key"); });
    const track = { kind: "audio", enabled: true, addEventListener: vi.fn(), stop: vi.fn() };
    mocks.capture.mockResolvedValue({ raw: { getAudioTracks: () => [track] }, stream: { getAudioTracks: () => [track] } });
    vi.stubGlobal("MediaStream", class { constructor(public tracks: unknown[]) {} });
    Object.defineProperty(navigator, "mediaDevices", { configurable: true, value: { getUserMedia: vi.fn() } });
  });

  it("refuses unsupported encryption before requesting a token or microphone", async () => {
    mocks.supported.mockReturnValue(false);
    await expect(new SfuSession(options()).start()).rejects.toThrow("chiffrés");
    expect(mocks.invoke).not.toHaveBeenCalled();
    expect(mocks.capture).not.toHaveBeenCalled();
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
});

