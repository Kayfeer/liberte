import { act, fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MessageInput from "./MessageInput";
import { useMessageStore } from "@/stores/messageStore";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("@/lib/tauri", () => ({ sendTyping: vi.fn().mockResolvedValue(undefined), sendFile: vi.fn() }));
const sendMessage = vi.fn();
beforeEach(() => { sendMessage.mockReset(); useMessageStore.setState({ sendMessage }); });

describe("channel drafts", () => {
  it("keeps drafts separate and sends only the active channel's text", async () => {
    sendMessage.mockResolvedValue(undefined);
    const view = render(<MessageInput channelId="a" />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Brouillon A" } });
    view.rerender(<MessageInput channelId="b" />);
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("");
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Message B" } });
    await act(async () => { fireEvent.click(screen.getByLabelText("Envoyer le message")); });
    expect(sendMessage).toHaveBeenCalledWith("b", "Message B");
    view.rerender(<MessageInput channelId="a" />);
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Brouillon A");
  });
  it("does not erase a newer draft when an earlier send completes", async () => {
    let finish!: () => void;
    sendMessage.mockReturnValue(new Promise<void>((resolve) => { finish = resolve; }));
    render(<MessageInput channelId="a" />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Premier" } });
    fireEvent.click(screen.getByLabelText("Envoyer le message"));
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Suivant" } });
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });
    expect(sendMessage).toHaveBeenCalledOnce();
    await act(async () => { finish(); });
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("Suivant");
  });
  it("keeps a failed send's draft and error in its original channel", async () => {
    let fail!: (reason: Error) => void;
    sendMessage.mockReturnValue(new Promise<void>((_, reject) => { fail = reject; }));
    const view = render(<MessageInput channelId="a" />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "À garder" } });
    fireEvent.click(screen.getByLabelText("Envoyer le message"));
    view.rerender(<MessageInput channelId="b" />);
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "B" } });
    await act(async () => { fail(new Error("Pair indisponible")); });
    expect(screen.queryByRole("alert")).toBeNull();
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("B");
    view.rerender(<MessageInput channelId="a" />);
    expect(screen.getByRole("alert").textContent).toContain("Pair indisponible");
    expect((screen.getByRole("textbox") as HTMLTextAreaElement).value).toBe("À garder");
  });
});
