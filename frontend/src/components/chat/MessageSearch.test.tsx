import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import MessageSearch from "./MessageSearch";
import { useMessageStore } from "@/stores/messageStore";
import type { Message } from "@/lib/types";
const mocks = vi.hoisted(() => ({ searchMessages: vi.fn() }));
vi.mock("@/lib/tauri", () => mocks);
const result = (id: string): Message => ({ id, channelId: "a", senderId: "alice", content: id, timestamp: "2026-01-01T00:00:00Z", reactions: [] });
beforeEach(() => { vi.useFakeTimers(); mocks.searchMessages.mockReset(); });
afterEach(() => vi.useRealTimers());
async function query(text: string) {
  fireEvent.change(screen.getByRole("textbox"), { target: { value: text } });
  await act(async () => { await vi.advanceTimersByTimeAsync(300); });
}
describe("message search", () => {
  it("ignores stale requests after changing channels", async () => {
    let finish!: (value: Message[]) => void;
    mocks.searchMessages.mockReturnValueOnce(new Promise<Message[]>((resolve) => { finish = resolve; }))
      .mockResolvedValueOnce([result("Nouveau")]);
    const view = render(<MessageSearch channelId="a" onClose={vi.fn()} />);
    await query("bonjour");
    view.rerender(<MessageSearch channelId="b" onClose={vi.fn()} />);
    await act(async () => { await vi.advanceTimersByTimeAsync(300); });
    await act(async () => { finish([result("Ancien")]); });
    expect(screen.getByText("Nouveau")).toBeTruthy();
    expect(screen.queryByText("Ancien")).toBeNull();
    expect(mocks.searchMessages).toHaveBeenLastCalledWith("bonjour", "b");
  });
  it("shows IPC errors instead of reporting no results", async () => {
    mocks.searchMessages.mockRejectedValue(new Error("Database locked"));
    render(<MessageSearch channelId="a" onClose={vi.fn()} />);
    await query("bonjour");
    expect(screen.getByRole("alert").textContent).toContain("Database locked");
    expect(screen.queryByText("Aucun résultat")).toBeNull();
  });
  it("waits for the actual message to load before closing", async () => {
    mocks.searchMessages.mockResolvedValue([result("Ancien message")]);
    let finish!: (value: boolean) => void;
    const reveal = vi.fn().mockReturnValue(new Promise<boolean>((resolve) => { finish = resolve; }));
    const original = useMessageStore.getState().revealMessage;
    useMessageStore.setState({ revealMessage: reveal });
    const close = vi.fn();
    try {
      render(<MessageSearch channelId="a" onClose={close} />);
      await query("ancien");
      fireEvent.click(screen.getByText("Ancien message"));
      expect(reveal).toHaveBeenCalledWith("a", "Ancien message");
      expect(close).not.toHaveBeenCalled();
      await act(async () => { finish(true); });
      expect(close).toHaveBeenCalledOnce();
    } finally { useMessageStore.setState({ revealMessage: original }); }
  });
  it("cancels a debounced search when closed", async () => {
    const view = render(<MessageSearch channelId="a" onClose={vi.fn()} />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "bonjour" } });
    view.unmount();
    await vi.advanceTimersByTimeAsync(500);
    expect(mocks.searchMessages).not.toHaveBeenCalled();
  });
});
