import { act, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import TypingIndicator from "./TypingIndicator";
const mocks = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => mocks);
type Callback = (event: { payload: { channelId: string; userId: string; displayName: string } }) => void;
beforeEach(() => mocks.listen.mockReset());
describe("typing subscription", () => {
  it("ignores a late event from the previous channel and disposes its late registration", async () => {
    const callbacks: Callback[] = [];
    let finish!: (unlisten: () => void) => void;
    const unlisten = vi.fn();
    mocks.listen.mockImplementationOnce((_: string, callback: Callback) => {
      callbacks.push(callback); return new Promise<() => void>((resolve) => { finish = resolve; });
    }).mockImplementationOnce((_: string, callback: Callback) => { callbacks.push(callback); return Promise.resolve(vi.fn()); });
    const view = render(<TypingIndicator channelId="a" />);
    view.rerender(<TypingIndicator channelId="b" />);
    await act(async () => {
      callbacks[0]({ payload: { channelId: "a", userId: "alice", displayName: "Alice" } });
      finish(unlisten);
    });
    expect(screen.queryByText(/Alice/)).toBeNull();
    expect(unlisten).toHaveBeenCalledOnce();
    act(() => callbacks[1]({ payload: { channelId: "b", userId: "bob", displayName: "Bob" } }));
    expect(screen.getByText(/Bob/)).toBeTruthy();
  });
  it("reports a listener registration failure", async () => {
    mocks.listen.mockRejectedValue(new Error("Permission denied"));
    render(<TypingIndicator channelId="a" />);
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Permission denied"));
  });
});
