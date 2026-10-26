import { beforeEach, describe, expect, it, vi } from "vitest";
import { useFileStore } from "./fileStore";
import type { FileTransferDto } from "@/lib/tauri";
const mocks = vi.hoisted(() => ({ listChannelFiles: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);
const file: FileTransferDto = { fileId: "offer", channelId: "room", fileName: "hello.txt", fileSize: 4, senderId: "alice", progress: 0, status: "offered" };
beforeEach(() => { mocks.listChannelFiles.mockReset(); useFileStore.setState({ transfers: {}, errors: {} }); });
describe("transfer list reconciliation", () => {
  it("does not replace a completed live transfer with an older list snapshot", async () => {
    let finish!: (value: FileTransferDto[]) => void;
    mocks.listChannelFiles.mockReturnValue(new Promise<FileTransferDto[]>((resolve) => { finish = resolve; }));
    const load = useFileStore.getState().load("room");
    useFileStore.getState().upsert({ ...file, status: "received", progress: 100 });
    finish([file]);
    await load;
    expect(useFileStore.getState().transfers.offer.status).toBe("received");
    expect(useFileStore.getState().transfers.offer.progress).toBe(100);
  });
  it("keeps a dismissed transfer removed when a pending list returns", async () => {
    let finish!: (value: FileTransferDto[]) => void;
    mocks.listChannelFiles.mockReturnValue(new Promise<FileTransferDto[]>((resolve) => { finish = resolve; }));
    const load = useFileStore.getState().load("room");
    useFileStore.getState().remove("dismissed");
    finish([{ ...file, fileId: "dismissed" }]);
    await load;
    useFileStore.getState().upsert({ ...file, fileId: "dismissed" });
    expect(useFileStore.getState().transfers.dismissed).toBeUndefined();
  });
});
