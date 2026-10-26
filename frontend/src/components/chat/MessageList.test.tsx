import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MessageList from "./MessageList";
import { useMessageStore } from "../../stores/messageStore";

vi.mock("./MessageBubble", () => ({ default: ({ message }: { message: { content: string } }) => <p>{message.content}</p> }));
const mocks = vi.hoisted(() => ({ getMessages: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);

describe("history navigation", () => {
  beforeEach(() => {
    useMessageStore.setState({ messages: { room: [] }, hasMore: { room: true }, loadingByChannel: {}, loadingMore: {}, errors: {}, channelKeys: { room: "key" } });
  });
  it("offers older messages and displays them after loading", async () => {
    mocks.getMessages.mockResolvedValueOnce([{ id: "old", channelId: "room", senderId: "a", timestamp: "2026-01-01T00:00:00Z", content: "Ancien message", reactions: [] }]);
    render(<MessageList channelId="room" />);
    fireEvent.click(screen.getByRole("button", { name: "Charger les messages précédents" }));
    await waitFor(() => expect(screen.getByText("Ancien message")).toBeTruthy());
    expect(screen.queryByRole("button", { name: "Charger les messages précédents" })).toBeNull();
  });
});
