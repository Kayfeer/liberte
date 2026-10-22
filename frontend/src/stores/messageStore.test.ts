import { beforeEach, describe, expect, it, vi } from "vitest";
import { useMessageStore } from "./messageStore";
import type { Message } from "@/lib/types";

const mocks = vi.hoisted(() => ({ getMessages: vi.fn(), listChannels: vi.fn(), getAllChannelKeys: vi.fn(), sendMessage: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);
function message(index: number): Message {
  return { id: String(index), channelId: "room", senderId: "alice", content: `Message ${index}`,
    timestamp: new Date(Date.UTC(2026, 0, 1, 0, 0, index)).toISOString(), reactions: [] };
}

describe("message history", () => {
  beforeEach(() => {
    Object.values(mocks).forEach((mock) => mock.mockReset());
    useMessageStore.setState({ channels: [], activeChannelId: null, messages: {}, channelKeys: { room: "key" },
      loadingByChannel: {}, loadingMore: {}, hasMore: {}, errors: {}, error: null });
  });
  it("loads older pages and keeps them after a real-time refresh", async () => {
    mocks.getMessages.mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(index + 50)))
      .mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(index)))
      .mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(index + 51)));
    await useMessageStore.getState().loadMessages("room");
    await useMessageStore.getState().loadMoreMessages("room");
    expect(mocks.getMessages).toHaveBeenNthCalledWith(2, "room", "key", 50, 50);
    await useMessageStore.getState().loadMessages("room");
    const messages = useMessageStore.getState().messages.room;
    expect(messages).toHaveLength(101);
    expect(messages[0].id).toBe("0");
    expect(messages[100].id).toBe("100");
    expect(new Set(messages.map((item) => item.id)).size).toBe(101);
  });
  it("does not overwrite a new refresh with an older response", async () => {
    let finishOlder!: (value: Message[]) => void;
    mocks.getMessages.mockReturnValueOnce(new Promise<Message[]>((resolve) => { finishOlder = resolve; }))
      .mockResolvedValueOnce([message(2)]);
    const older = useMessageStore.getState().loadMessages("room");
    await useMessageStore.getState().loadMessages("room");
    finishOlder([message(1)]);
    await older;
    expect(useMessageStore.getState().messages.room.map((item) => item.id)).toEqual(["2"]);
  });
  it("retains history and exposes failures for retry", async () => {
    useMessageStore.setState({ messages: { room: [message(0)] }, hasMore: { room: true } });
    mocks.getMessages.mockRejectedValueOnce(new Error("Database unavailable"));
    await useMessageStore.getState().loadMoreMessages("room");
    expect(useMessageStore.getState().messages.room).toEqual([message(0)]);
    expect(useMessageStore.getState().errors.room).toContain("Database unavailable");
    expect(useMessageStore.getState().loadingMore.room).toBe(false);
  });
  it("stops pagination when the database returns the final short page", async () => {
    mocks.getMessages.mockResolvedValueOnce([message(0)]);
    await useMessageStore.getState().loadMessages("room");
    await useMessageStore.getState().loadMoreMessages("room");
    expect(useMessageStore.getState().hasMore.room).toBe(false);
    expect(mocks.getMessages).toHaveBeenCalledOnce();
  });
});
