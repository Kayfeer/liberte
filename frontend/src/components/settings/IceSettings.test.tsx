import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import IceSettings from "./IceSettings";

describe("ICE settings acknowledgement", () => {
  it("waits for persistence and active call preferences before showing success", async () => {
    let finish!: (saved: boolean) => void;
    const onSave = vi.fn(() => new Promise<boolean>((resolve) => { finish = resolve; }));
    render(<IceSettings servers={[]} onSave={onSave} />);
    fireEvent.change(screen.getByLabelText("Serveurs ICE (JSON)"), { target: { value: '[{"urls":"stun:relay.example:3478"}]' } });
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer la configuration" }));
    expect(screen.queryByRole("status")).toBeNull();
    expect((screen.getByRole("button", { name: "Enregistrement..." }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => finish(true));
    expect(screen.getByRole("status").textContent).toBe("Configuration enregistrée.");
    expect(onSave).toHaveBeenCalledWith([{ urls: "stun:relay.example:3478" }]);
  });
  it("does not claim success when persistence or application reports failure", async () => {
    render(<IceSettings servers={[]} onSave={vi.fn().mockResolvedValue(false)} />);
    fireEvent.click(screen.getByRole("button", { name: "Enregistrer la configuration" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("n'a pas pu être enregistrée ou appliquée"));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
