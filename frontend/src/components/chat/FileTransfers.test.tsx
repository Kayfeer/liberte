import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import FileTransfers from "./FileTransfers";
import { useFileStore } from "@/stores/fileStore";
import { useIdentityStore } from "@/stores/identityStore";
import type { FileTransferDto } from "@/lib/tauri";

const mocks = vi.hoisted(() => ({ listChannelFiles: vi.fn(), acceptFile: vi.fn(), saveReceivedFile: vi.fn(), save: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);
vi.mock("@tauri-apps/plugin-dialog", () => ({ save: mocks.save }));

const offered: FileTransferDto = { fileId: "file", channelId: "room", fileName: "document.txt", fileSize: 1024, status: "offered", progress: 0, senderId: "alice" };
describe("file transfers", () => {
  beforeEach(() => {
    Object.values(mocks).forEach((mock) => mock.mockReset());
    useFileStore.setState({ transfers: {}, errors: {} });
    useIdentityStore.setState({ identity: { publicKey: "me", shortId: "me", createdAt: "", status: "online" } });
    mocks.listChannelFiles.mockResolvedValue([offered]);
  });
  it("requires explicit acceptance of an incoming file and shows failures", async () => {
    mocks.acceptFile.mockRejectedValueOnce(new Error("Peer disconnected"));
    render(<FileTransfers channelId="room" />);
    const button = await screen.findByRole("button", { name: "Recevoir" });
    expect(mocks.acceptFile).not.toHaveBeenCalled();
    fireEvent.click(button);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Peer disconnected"));
    expect(mocks.acceptFile).toHaveBeenCalledWith("file");
    expect(mocks.saveReceivedFile).not.toHaveBeenCalled();
  });
  it("saves a completed incoming file only to the chosen destination", async () => {
    mocks.listChannelFiles.mockResolvedValue([{ ...offered, status: "received" }]);
    mocks.save.mockResolvedValue("/chosen/document.txt");
    mocks.saveReceivedFile.mockResolvedValue(undefined);
    render(<FileTransfers channelId="room" />);
    fireEvent.click(await screen.findByRole("button", { name: "Enregistrer" }));
    await waitFor(() => expect(mocks.saveReceivedFile).toHaveBeenCalledWith("file", "/chosen/document.txt"));
  });
  it("does not offer to receive our own outgoing file", async () => {
    mocks.listChannelFiles.mockResolvedValue([{ ...offered, senderId: "me" }]);
    render(<FileTransfers channelId="room" />);
    await screen.findByText("document.txt");
    expect(screen.queryByRole("button", { name: "Recevoir" })).toBeNull();
  });
});

