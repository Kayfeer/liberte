import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  start: vi.fn(), stop: vi.fn(), announce: vi.fn(), mute: vi.fn(), camera: vi.fn(),
  startCall: vi.fn(), endCall: vi.fn(), toggleMute: vi.fn(), toggleVideo: vi.fn(),
  setCallMode: vi.fn(), listen: vi.fn(), unlisten: vi.fn(),
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
} }));
vi.mock("@/lib/sfuSession", () => ({ SfuSession: class {
  start = mocks.start; stop = mocks.stop; announce = mocks.announce;
  setMuted = mocks.mute; setVideoEnabled = mocks.camera;
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
  mocks.listen.mockResolvedValue(mocks.unlisten);
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
