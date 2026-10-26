import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import ProfileModal from "./ProfileModal";
import { useIdentityStore } from "@/stores/identityStore";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), clipboard: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));

describe("Profile persistence", () => {
  beforeEach(() => {
    useIdentityStore.setState({ identity: { publicKey: "identity-key", shortId: "identity", createdAt: "2026-10-26", displayName: "Alice", bio: "Ancienne bio", status: "online" }, error: null, loading: false });
    Object.defineProperty(navigator, "clipboard", { value: { writeText: mocks.clipboard }, configurable: true });
  });

  it("keeps the name draft and shows the backend error until retry succeeds", async () => {
    mocks.invoke.mockRejectedValueOnce(new Error("database locked")).mockResolvedValueOnce(undefined);
    render(<ProfileModal isOpen onClose={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Modifier le pseudo" }));
    fireEvent.change(screen.getByPlaceholderText("Pseudo"), { target: { value: "Alice nouvelle" } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer le pseudo" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("database locked"));
    expect((screen.getByPlaceholderText("Pseudo") as HTMLInputElement).value).toBe("Alice nouvelle");
    expect(useIdentityStore.getState().identity?.displayName).toBe("Alice");
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer le pseudo" }));
    await waitFor(() => expect(screen.queryByPlaceholderText("Pseudo")).toBeNull());
    expect(useIdentityStore.getState().identity?.displayName).toBe("Alice nouvelle");
    expect(mocks.invoke).toHaveBeenCalledWith("set_display_name", { name: "Alice nouvelle" });
  });

  it("retains a bio draft when persistence fails", async () => {
    mocks.invoke.mockRejectedValue(new Error("disk full"));
    render(<ProfileModal isOpen onClose={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Modifier la bio" }));
    fireEvent.change(screen.getByPlaceholderText("Parlez de vous..."), { target: { value: "Nouvelle bio" } });
    fireEvent.click(screen.getByRole("button", { name: "Sauvegarder" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("disk full"));
    expect((screen.getByPlaceholderText("Parlez de vous...") as HTMLTextAreaElement).value).toBe("Nouvelle bio");
    expect(useIdentityStore.getState().identity?.bio).toBe("Ancienne bio");
    expect(mocks.invoke).toHaveBeenCalledWith("set_bio", { bio: "Nouvelle bio" });
  });

  it("does not claim that the public key was copied when the clipboard rejects", async () => {
    mocks.clipboard.mockRejectedValue(new Error("clipboard denied"));
    render(<ProfileModal isOpen onClose={() => undefined} />);
    fireEvent.click(screen.getByRole("button", { name: "Copier la clé publique" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("clipboard denied"));
    expect(screen.queryByRole("button", { name: "Clé publique copiée" })).toBeNull();
  });
});
