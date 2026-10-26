import { beforeEach, describe, expect, it, vi } from "vitest";
import { useMessageStore } from "./messageStore";
import type { Message } from "@/lib/types";

const mocks = vi.hoisted(() => ({ getMessages: vi.fn(), listChannels: vi.fn(), getAllChannelKeys: vi.fn(), sendMessage: vi.fn(), getReactions: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);
function message(index: number): Message {
  return { id: String(index), channelId: "room", senderId: "alice", content: `Message ${index}`,
    timestamp: new Date(Date.UTC(2026, 0, 1, 0, 0, index)).toISOString(), reactions: [] };
}

describe("message history", () => {
  beforeEach(() => {
    Object.values(mocks).forEach((mock) => mock.mockReset());
    useMessageStore.setState({ channels: [], activeChannelId: null, messages: {}, channelKeys: { room: "key" },
      loadingByChannel: {}, loadingMore: {}, hasMore: {}, errors: {}, error: null, focusedMessageId: null });
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
  it("refreshes reactions on an old loaded message without refetching only the newest page", async () => {
    useMessageStore.setState({ messages: { room: Array.from({ length: 100 }, (_, index) => message(index)) } });
    mocks.getReactions.mockResolvedValue([{ emoji: "👍", users: ["bob"] }]);
    await useMessageStore.getState().refreshReactions("room", "0");
    expect(useMessageStore.getState().messages.room[0].reactions).toEqual([{ emoji: "👍", users: ["bob"] }]);
    expect(mocks.getMessages).not.toHaveBeenCalled();
    expect(useMessageStore.getState().messages.room).toHaveLength(100);
  });
  it("reveals a search result older than the first history page", async () => {
    mocks.getMessages.mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(index + 50)))
      .mockResolvedValueOnce(Array.from({ length: 50 }, (_, index) => message(index)));
    expect(await useMessageStore.getState().revealMessage("room", "3")).toBe(true);
    expect(useMessageStore.getState().focusedMessageId).toBe("3");
    expect(useMessageStore.getState().activeChannelId).toBe("room");
    expect(useMessageStore.getState().messages.room).toHaveLength(100);
    expect(mocks.getMessages).toHaveBeenLastCalledWith("room", "key", 50, 50);
  });
  it("does not redirect focus when the user changes channel while history loads", async () => {
    let finish!: (value: Message[]) => void;
    mocks.getMessages.mockReturnValueOnce(new Promise<Message[]>((resolve) => { finish = resolve; }))
      .mockResolvedValueOnce([]);
    const reveal = useMessageStore.getState().revealMessage("room", "0");
    useMessageStore.getState().setActiveChannel("other");
    finish([message(0)]);
    expect(await reveal).toBe(false);
    expect(useMessageStore.getState().activeChannelId).toBe("other");
    expect(useMessageStore.getState().focusedMessageId).toBeNull();
  });
  it("reports a missing search result instead of pretending to open it", async () => {
    mocks.getMessages.mockResolvedValue([]);
    await expect(useMessageStore.getState().revealMessage("room", "gone")).rejects.toThrow("n'est plus disponible");
    expect(useMessageStore.getState().focusedMessageId).toBeNull();
  });
});
