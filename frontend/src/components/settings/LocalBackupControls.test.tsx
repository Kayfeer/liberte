import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import LocalBackupControls from "./LocalBackupControls";
import { useBackupStore } from "@/stores/backupStore";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), loadChannels: vi.fn(), loadMessages: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@/stores/messageStore", () => ({ useMessageStore: { getState: () => ({ activeChannelId: "restored-channel", error: null, errors: {}, loadChannels: mocks.loadChannels, loadMessages: mocks.loadMessages }) } }));
const file = { fileName: "liberte_backup_2026.liberte", filePath: "/app/backups/liberte_backup_2026.liberte", sizeBytes: 1024, modified: "2026-10-26T10:00:00Z" };

describe("Local automatic backup restore", () => {
  beforeEach(() => {
    useBackupStore.setState({ backups: [], lastBackupPath: file.filePath, lastBackupTime: null, isRestoring: false, error: null });
    mocks.loadChannels.mockResolvedValue(undefined); mocks.loadMessages.mockResolvedValue(undefined);
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "list_backups") return [file];
      if (command === "restore_local_backup") return { channelsImported: 1, messagesImported: 4, keysImported: 1 };
      throw new Error(`Unexpected command ${command}`);
    });
  });

  it("lists saved paths and requires confirmation before restoring with the current identity", async () => {
    render(<LocalBackupControls />);
    fireEvent.click(await screen.findByRole("button", { name: "Restaurer cette sauvegarde" }));
    expect(mocks.invoke).not.toHaveBeenCalledWith("restore_local_backup", expect.anything());
    fireEvent.click(screen.getByRole("button", { name: "Confirmer la restauration" }));
    await waitFor(() => expect(screen.getByRole("status").textContent).toContain("1 canaux et 4 messages"));
    expect(mocks.invoke).toHaveBeenCalledWith("restore_local_backup", { fileName: file.fileName });
    expect(mocks.loadChannels).toHaveBeenCalledOnce();
    expect(mocks.loadMessages).toHaveBeenCalledWith("restored-channel");
  });

  it("reports an unreadable backup without claiming restoration", async () => {
    mocks.invoke.mockImplementation(async (command: string) => {
      if (command === "list_backups") return [file];
      if (command === "restore_local_backup") throw new Error("file missing");
      throw new Error(`Unexpected command ${command}`);
    });
    render(<LocalBackupControls />);
    fireEvent.click(await screen.findByRole("button", { name: "Restaurer cette sauvegarde" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirmer la restauration" }));
    await waitFor(() => expect(screen.getByRole("alert").textContent).toContain("file missing"));
    expect(screen.queryByRole("status")).toBeNull();
    expect(mocks.loadChannels).not.toHaveBeenCalled();
  });
});
