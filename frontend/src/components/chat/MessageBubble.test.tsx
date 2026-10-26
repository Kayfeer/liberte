import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MessageBubble from "./MessageBubble";
import { useProfileStore } from "@/stores/profileStore";
import { useIdentityStore } from "@/stores/identityStore";
vi.mock("./ReactionBar", () => ({ default: () => null }));
vi.mock("../identity/ProfileModal", () => ({ default: () => null }));
const message = { id: "one", channelId: "room", senderId: "a".repeat(64), senderDisplayName: "Ancien pseudo", content: "Bonjour", timestamp: "2026-01-01T00:00:00Z", reactions: [] };
beforeEach(() => { useProfileStore.setState({ profiles: {} }); useIdentityStore.setState({ identity: null }); });
describe("message sender profile", () => {
  it("distinguishes an unknown name from an explicitly cleared name", () => {
    render(<MessageBubble message={message} />);
    act(() => useProfileStore.getState().setStatus(message.senderId, "online"));
    expect(screen.getByText("Ancien pseudo")).toBeTruthy();
    act(() => useProfileStore.getState().upsert({ userId: message.senderId, displayName: null, bio: null, status: "online" }));
    expect(screen.queryByText("Ancien pseudo")).toBeNull();
    expect(screen.getByText("aaaaaaaa")).toBeTruthy();
  });
  it("updates an already loaded own message immediately after editing the profile", () => {
    useIdentityStore.setState({ identity: { publicKey: message.senderId, shortId: "aaaaaaaa", createdAt: "", status: "online", displayName: "Actuel" } });
    render(<MessageBubble message={message} />);
    expect(screen.getByText("Actuel")).toBeTruthy();
    act(() => useIdentityStore.setState((state) => ({ identity: { ...state.identity!, displayName: "Nouveau" } })));
    expect(screen.getByText("Nouveau")).toBeTruthy();
    expect(screen.queryByText("Actuel")).toBeNull();
  });
});
