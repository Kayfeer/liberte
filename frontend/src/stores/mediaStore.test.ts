import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  start: vi.fn(), stop: vi.fn(), announce: vi.fn(), mute: vi.fn(), camera: vi.fn(),
  startCall: vi.fn(), endCall: vi.fn(), toggleMute: vi.fn(), toggleVideo: vi.fn(),
  setCallMode: vi.fn(), listen: vi.fn(), unlisten: vi.fn(),
  receive: vi.fn(), ensurePeer: vi.fn(), removePeer: vi.fn(),
  listeners: new Map<string, (event: { payload: unknown }) => void>(),
}));
vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn().mockResolvedValue(undefined) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@/lib/tauri", () => ({
  startCall: mocks.startCall, endCall: mocks.endCall, toggleMute: mocks.toggleMute,
  toggleVideo: mocks.toggleVideo, setCallMode: mocks.setCallMode,
}));
vi.mock("./identityStore", () => ({ useIdentityStore: { getState: () => ({ identity: { publicKey: "a".repeat(64) } }) } }));
vi.mock("@/lib/mediaSession", () => ({ MediaSession: class {
  start = mocks.start; stop = mocks.stop; announce = mocks.announce;
  setMuted = mocks.mute; setVideoEnabled = mocks.camera;
  receive = mocks.receive; ensurePeer = mocks.ensurePeer; removePeer = mocks.removePeer;
} }));
vi.mock("@/lib/sfuSession", () => ({ SfuSession: class {
  start = mocks.start; stop = mocks.stop; announce = mocks.announce;
  setMuted = mocks.mute; setVideoEnabled = mocks.camera;
  receive = mocks.receive; ensurePeer = mocks.ensurePeer; removePeer = mocks.removePeer;
} }));

import { useMediaStore } from "./mediaStore";

beforeEach(() => {
  mocks.start.mockResolvedValue(undefined);
  mocks.stop.mockResolvedValue(undefined);
  mocks.camera.mockResolvedValue(undefined);
  mocks.startCall.mockResolvedValue(undefined);
  mocks.endCall.mockResolvedValue(undefined);
  mocks.toggleMute.mockResolvedValue(true);
  mocks.toggleVideo.mockResolvedValue(true);
  mocks.setCallMode.mockResolvedValue("mesh");
  mocks.receive.mockResolvedValue(undefined);
  mocks.listeners.clear();
  mocks.listen.mockImplementation(async (event, callback) => { mocks.listeners.set(event, callback); return mocks.unlisten; });
  useMediaStore.setState({ mode: "mesh", error: null, busy: false });
});
afterEach(async () => {
  await useMediaStore.getState().endCall().catch(() => {});
  useMediaStore.getState().cleanup();
});

describe("media store lifecycle", () => {
  it("rolls back local capture if the backend rejects the call", async () => {
    mocks.startCall.mockRejectedValueOnce(new Error("Channel not found"));
    await expect(useMediaStore.getState().startCall("missing")).rejects.toThrow("Channel not found");
    expect(mocks.stop).toHaveBeenCalledOnce();
    expect(useMediaStore.getState()).toMatchObject({ inCall: false, channelId: null, busy: false, error: "Channel not found" });
  });

  it("hangs up locally even if backend teardown fails", async () => {
    await useMediaStore.getState().startCall("channel");
    mocks.endCall.mockRejectedValueOnce(new Error("IPC unavailable"));
    await expect(useMediaStore.getState().endCall()).rejects.toThrow("IPC unavailable");
    expect(mocks.stop).toHaveBeenCalledOnce();
    expect(useMediaStore.getState()).toMatchObject({ inCall: false, channelId: null, localStream: null, participants: [], busy: false });
  });

  it("does not revive a call cancelled during microphone permission", async () => {
    let finish!: () => void;
    mocks.start.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const starting = useMediaStore.getState().startCall("channel");
    await vi.waitFor(() => expect(mocks.start).toHaveBeenCalled());
    await useMediaStore.getState().endCall();
    finish();
    await starting;
    expect(mocks.startCall).not.toHaveBeenCalled();
    expect(useMediaStore.getState().inCall).toBe(false);
  });

  it("waits for native call acceptance before forwarding queued presence and signaling", async () => {
    let finish!: () => void;
    mocks.startCall.mockImplementationOnce(() => new Promise<void>((resolve) => { finish = resolve; }));
    const starting = useMediaStore.getState().startCall("channel");
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    const userId = "b".repeat(64);
    const signal = { channelId: "channel", sender: userId, target: "a".repeat(64), signalType: { Offer: "sdp" } };
    mocks.listeners.get("voice-peer-joined")!({ payload: { channelId: "channel", userId } });
    mocks.listeners.get("call-signal")!({ payload: signal });
    const departedId = "c".repeat(64);
    mocks.listeners.get("voice-peer-joined")!({ payload: { channelId: "channel", userId: departedId } });
    mocks.listeners.get("call-signal")!({ payload: { ...signal, sender: departedId } });
    mocks.listeners.get("voice-peer-left")!({ payload: { channelId: "channel", userId: departedId } });
    expect(mocks.ensurePeer).not.toHaveBeenCalled();
    expect(mocks.receive).not.toHaveBeenCalled();
    finish();
    await starting;
    expect(mocks.ensurePeer).toHaveBeenCalledOnce();
    expect(mocks.ensurePeer).toHaveBeenCalledWith(userId);
    expect(mocks.receive).toHaveBeenCalledOnce();
    expect(mocks.receive).toHaveBeenCalledWith(signal);
  });

  it("orders a pending native start before hangup so cancellation cannot leave ghost presence", async () => {
    let finish!: () => void;
    let nativeInCall = false;
    mocks.startCall.mockImplementationOnce(() => new Promise<void>((resolve) => {
      finish = () => { nativeInCall = true; resolve(); };
    }));
    mocks.endCall.mockImplementationOnce(async () => { nativeInCall = false; });
    const starting = useMediaStore.getState().startCall("channel");
    await vi.waitFor(() => expect(finish).toBeTypeOf("function"));
    const ending = useMediaStore.getState().endCall();
    await Promise.resolve();
    expect(mocks.endCall).not.toHaveBeenCalled();
    finish();
    await Promise.all([starting, ending]);
    expect(nativeInCall).toBe(false);
    expect(useMediaStore.getState()).toMatchObject({ inCall: false, busy: false });
  });

  it("silences the microphone before awaiting the backend", async () => {
    await useMediaStore.getState().startCall("channel");
    let resolve!: (muted: boolean) => void;
    mocks.toggleMute.mockImplementationOnce(() => new Promise<boolean>((done) => { resolve = done; }));
    const muting = useMediaStore.getState().toggleMute();
    expect(mocks.mute).toHaveBeenCalledWith(true);
    resolve(true);
    await muting;
    expect(useMediaStore.getState().isMuted).toBe(true);
  });

  it("deduplicates listeners and cleans registrations that finish after unmount", async () => {
    let release!: () => void;
    mocks.listen.mockImplementationOnce(() => new Promise<() => void>((resolve) => { release = () => resolve(mocks.unlisten); }));
    const first = useMediaStore.getState().setupListeners();
    const second = useMediaStore.getState().setupListeners();
    expect(first).toBe(second);
    useMediaStore.getState().cleanup();
    release();
    await first;
    expect(useMediaStore.getState().unlisteners).toHaveLength(0);
    expect(mocks.unlisten).toHaveBeenCalledTimes(4);
  });

  it("does not clear a new call's busy state when an old camera request resolves", async () => {
    await useMediaStore.getState().startCall("first");
    let finishCamera!: () => void;
    mocks.camera.mockImplementationOnce(() => new Promise<void>((resolve) => { finishCamera = resolve; }));
    const camera = useMediaStore.getState().toggleVideo();
    await useMediaStore.getState().endCall();
    let finishCapture!: () => void;
    mocks.start.mockImplementationOnce(() => new Promise<void>((resolve) => { finishCapture = resolve; }));
    const starting = useMediaStore.getState().startCall("second");
    await vi.waitFor(() => expect(mocks.start).toHaveBeenCalledTimes(2));
    finishCamera();
    await camera;
    expect(useMediaStore.getState().busy).toBe(true);
    finishCapture();
    await starting;
    expect(useMediaStore.getState().inCall).toBe(true);
  });

  it("never restarts the camera when a disable IPC fails", async () => {
    await useMediaStore.getState().startCall("channel");
    await useMediaStore.getState().toggleVideo();
    expect(useMediaStore.getState().isVideoEnabled).toBe(true);
    mocks.camera.mockClear();
    mocks.toggleVideo.mockRejectedValueOnce(new Error("IPC unavailable"));
    await expect(useMediaStore.getState().toggleVideo()).rejects.toThrow("IPC unavailable");
    expect(mocks.camera.mock.calls.every(([enabled]) => enabled === false)).toBe(true);
    expect(useMediaStore.getState().isVideoEnabled).toBe(false);
  });
});
