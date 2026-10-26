import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import InviteModal from "./InviteModal";
const mocks = vi.hoisted(() => ({ generateInvite: vi.fn(), copy: vi.fn() }));
vi.mock("@/lib/tauri", () => ({ generateInvite: mocks.generateInvite }));
const props = { isOpen: true, onClose: vi.fn(), channelId: "a", channelName: "A", channelKeyHex: "key-a" };
beforeEach(() => {
  mocks.generateInvite.mockReset(); mocks.copy.mockReset();
  Object.defineProperty(navigator, "clipboard", { value: { writeText: mocks.copy }, configurable: true });
});
describe("invitations", () => {
  it("never displays an earlier channel's invitation under a different channel", async () => {
    let finish!: (code: string) => void;
    mocks.generateInvite.mockReturnValueOnce(new Promise<string>((resolve) => { finish = resolve; }))
      .mockResolvedValueOnce("code-b");
    const view = render(<InviteModal {...props} />);
    view.rerender(<InviteModal {...props} channelId="b" channelName="B" channelKeyHex="key-b" />);
    await waitFor(() => expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("code-b"));
    await act(async () => { finish("code-a"); });
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("code-b");
  });
  it("reports denied clipboard access without a false success", async () => {
    mocks.generateInvite.mockResolvedValue("invite");
    mocks.copy.mockRejectedValue(new Error("Permission denied"));
    render(<InviteModal {...props} />);
    await screen.findByRole("textbox");
    fireEvent.click(screen.getByText("Copier le code"));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("Permission denied"));
    expect(screen.queryByText("Copié")).toBeNull();
  });
  it("lets the user retry after generation fails", async () => {
    mocks.generateInvite.mockRejectedValueOnce(new Error("Réseau indisponible")).mockResolvedValueOnce("invite");
    render(<InviteModal {...props} />);
    fireEvent.click(await screen.findByText("Réessayer"));
    await waitFor(() => expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("invite"));
  });
});
