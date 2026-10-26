import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import RemoteBackupControls from "./RemoteBackupControls";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), loadChannels: vi.fn(), loadMessages: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@/stores/messageStore", () => ({ useMessageStore: { getState: () => ({ activeChannelId: "channel", error: null, errors: {}, loadChannels: mocks.loadChannels, loadMessages: mocks.loadMessages }) } }));

describe("Remote backup controls", () => {
  beforeEach(() => { mocks.loadChannels.mockResolvedValue(undefined); mocks.loadMessages.mockResolvedValue(undefined); });
  it("reports an offline failure without claiming success", async () => {
    mocks.invoke.mockRejectedValue(new Error("Serveur inaccessible"));
    render(<RemoteBackupControls />);
    fireEvent.click(screen.getByRole("button", { name: "Synchroniser maintenant" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Serveur inaccessible"));
    expect(screen.queryByRole("status")).toBeNull();
    expect(mocks.invoke).toHaveBeenCalledWith("upload_remote_backup");
  });
  it("refreshes the local history after a successful restore", async () => {
    mocks.invoke.mockResolvedValue({ channelsImported: 2, messagesImported: 5, keysImported: 2 });
    render(<RemoteBackupControls />);
    fireEvent.click(screen.getByRole("button", { name: "Restaurer depuis le serveur" }));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("2 canaux et 5 messages"));
    expect(mocks.loadChannels).toHaveBeenCalledOnce();
    expect(mocks.loadMessages).toHaveBeenCalledWith("channel");
  });
  it("shows success only after the encrypted upload is acknowledged", async () => {
    mocks.invoke.mockResolvedValue({ sizeBytes: 2048, syncedAt: "2026-10-21T12:00:00Z" });
    render(<RemoteBackupControls />);
    fireEvent.click(screen.getByRole("button", { name: "Synchroniser maintenant" }));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("2 Ko"));
  });
});
