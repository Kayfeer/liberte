import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import Settings from "./Settings";
import { useIdentityStore } from "@/stores/identityStore";

const mocks = vi.hoisted(() => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: mocks.invoke }));
vi.mock("@/components/settings/ArchiveControls", () => ({ ArchiveExport: () => null, BackupSettings: () => null }));
vi.mock("@/hooks/useMediaDevices", () => ({ useMediaDevices: () => ({ audioInputs: [], audioOutputs: [], videoInputs: [], selectedAudioInput: "", selectedAudioOutput: "", selectedVideoInput: "", inputVolume: 100, outputVolume: 100, noiseSuppression: true, echoCancellation: true, autoGainControl: false, voiceIsolation: false, iceServers: [], loading: false, error: null, update: vi.fn(), refresh: vi.fn() }) }));

describe("Settings server contract", () => {
  beforeEach(() => { useIdentityStore.setState({ identity: null, error: null }); });

  it("reloads access status for a newly persisted server without leaving Settings", async () => {
    let savedServer = "https://old.example";
    mocks.invoke.mockImplementation(async (command: string, args?: { settings?: { serverUrl: string } }) => {
      if (command === "get_settings") return { serverUrl: savedServer };
      if (command === "get_network_status") return { peers: [], connectionMode: "disconnected", listenAddresses: [], externalAddresses: [], localPeerId: null, error: null };
      if (command === "check_premium") return { isPremium: savedServer === "https://old.example", validUntil: null };
      if (command === "get_server_info") return { name: "Nouveau", version: "2.4.0" };
      if (command === "update_settings") { savedServer = args!.settings!.serverUrl; return; }
      throw new Error(`Unexpected command ${command}`);
    });
    render(<Settings />);
    await screen.findByText("Accès actif.");
    const serverInput = screen.getByLabelText("URL du serveur auto-hébergé");
    await waitFor(() => expect((serverInput as HTMLInputElement).value).toBe("https://old.example"));
    fireEvent.change(serverInput, { target: { value: "https://new.example/" } });
    fireEvent.click(screen.getByRole("button", { name: "Vérifier et enregistrer le serveur" }));
    await screen.findByText("Aucun abonnement actif.");
    expect(screen.queryByText("Accès actif.")).toBeNull();
    expect(mocks.invoke).toHaveBeenCalledWith("update_settings", { settings: { serverUrl: "https://new.example" } });
  });
});
