import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Home from "./Home";
import { useMessageStore } from "../stores/messageStore";
import { useNetworkStore } from "../stores/networkStore";
import { useMediaStore } from "../stores/mediaStore";
import { useNavigationStore } from "../stores/navigationStore";

const mocks = vi.hoisted(() => ({
  listChannels: vi.fn(), getAllChannelKeys: vi.fn(), getNetworkStatus: vi.fn(),
  listen: vi.fn(), setupListeners: vi.fn(), cleanup: vi.fn(),
}));
vi.mock("@/lib/tauri", () => ({
  listChannels: mocks.listChannels, getAllChannelKeys: mocks.getAllChannelKeys, getNetworkStatus: mocks.getNetworkStatus,
}));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));
vi.mock("@tauri-apps/plugin-notification", () => ({
  isPermissionGranted: vi.fn().mockResolvedValue(false),
  requestPermission: vi.fn().mockResolvedValue("denied"), sendNotification: vi.fn(),
}));
vi.mock("../stores/mediaStore", async () => {
  const { create } = await import("zustand");
  return { useMediaStore: create(() => ({
    inCall: false, error: null, setupListeners: mocks.setupListeners, cleanup: mocks.cleanup,
  })) };
});
vi.mock("../components/voice/VoicePanel", () => ({ default: () => <div data-testid="voice-panel" /> }));
vi.mock("../components/layout/Sidebar", () => ({ default: () => null }));
vi.mock("../components/layout/Header", () => ({ default: () => null }));
vi.mock("../components/layout/MainPanel", () => ({ default: () => null }));
vi.mock("../components/common/UpdateBanner", () => ({ default: () => null }));
vi.mock("./Settings", () => ({ default: () => null }));

describe("home lifecycle", () => {
  beforeEach(() => {
    mocks.listChannels.mockResolvedValue([{ id: "one", name: "General", createdAt: "" }]);
    mocks.getAllChannelKeys.mockResolvedValue({ one: "key" });
    mocks.getNetworkStatus.mockResolvedValue({ peers: [], connectionMode: "disconnected", listenAddresses: [], externalAddresses: [], localPeerId: null, error: null });
    mocks.listen.mockResolvedValue(vi.fn());
    mocks.setupListeners.mockResolvedValue(undefined);
    useMessageStore.setState({ channels: [], error: null });
    useNetworkStore.setState({ error: null });
    useMediaStore.setState({ inCall: false, error: null });
    useNavigationStore.setState({ currentPage: "home" });
  });
  it("loads channels once and does not re-register when channel data changes", async () => {
    const view = render(<Home />);
    await waitFor(() => expect(useMessageStore.getState().channels).toHaveLength(1));
    const registrations = mocks.listen.mock.calls.length;
    act(() => useMessageStore.setState({ channels: [{ id: "two", name: "Other", createdAt: "" }] }));
    await act(async () => { await Promise.resolve(); });
    expect(mocks.listChannels).toHaveBeenCalledOnce();
    expect(mocks.getAllChannelKeys).toHaveBeenCalledOnce();
    expect(mocks.listen).toHaveBeenCalledTimes(registrations);
    expect(registrations).toBe(8);
    view.unmount();
    expect(mocks.cleanup).toHaveBeenCalledOnce();
  });
  it("keeps call playback mounted while settings are open and shows a fatal call error", async () => {
    useMediaStore.setState({ inCall: true });
    render(<Home />);
    const playback = screen.getByTestId("voice-panel");
    act(() => useNavigationStore.setState({ currentPage: "settings" }));
    expect(screen.getByTestId("voice-panel")).toBe(playback);
    act(() => useMediaStore.setState({ inCall: false, error: "Chiffrement interrompu" }));
    expect(screen.queryByTestId("voice-panel")).toBeNull();
    expect(screen.getByRole("alert").textContent).toContain("Chiffrement interrompu");
    await waitFor(() => expect(useMessageStore.getState().channels).toHaveLength(1));
  });
});
