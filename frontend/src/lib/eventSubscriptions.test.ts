import { beforeEach, describe, expect, it, vi } from "vitest";
import { createEventSubscriptions } from "./eventSubscriptions";

const mocks = vi.hoisted(() => ({ listen: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocks.listen }));

describe("Tauri event subscriptions", () => {
  beforeEach(() => mocks.listen.mockReset());
  it("releases a registration which completes after unmount", async () => {
    let resolve!: (value: () => void) => void;
    mocks.listen.mockReturnValue(new Promise<() => void>((done) => { resolve = done; }));
    const callback = vi.fn();
    const scope = createEventSubscriptions(vi.fn());
    scope.add("event", callback);
    scope.dispose();
    const unlisten = vi.fn();
    resolve(unlisten);
    await vi.waitFor(() => expect(unlisten).toHaveBeenCalledOnce());
    mocks.listen.mock.calls[0][1]({ event: "event", id: 1, payload: {} });
    expect(callback).not.toHaveBeenCalled();
  });
  it("cleans every listener exactly once and ignores later events", async () => {
    const unlisten = vi.fn();
    mocks.listen.mockResolvedValue(unlisten);
    const callback = vi.fn();
    const scope = createEventSubscriptions(vi.fn());
    scope.add("event", callback);
    await Promise.resolve();
    mocks.listen.mock.calls[0][1]({ event: "event", id: 1, payload: "first" });
    expect(callback).toHaveBeenCalledOnce();
    scope.dispose(); scope.dispose();
    expect(unlisten).toHaveBeenCalledOnce();
    mocks.listen.mock.calls[0][1]({ event: "event", id: 1, payload: "late" });
    expect(callback).toHaveBeenCalledOnce();
  });
  it("reports a registration failure while mounted", async () => {
    mocks.listen.mockRejectedValue(new Error("permission denied"));
    const onError = vi.fn();
    const scope = createEventSubscriptions(onError);
    scope.add("event", vi.fn());
    await vi.waitFor(() => expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: "permission denied" })));
    scope.dispose();
  });
});
