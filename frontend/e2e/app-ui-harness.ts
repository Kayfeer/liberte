// UI rendering fixture only: all native/backend IPC is explicitly simulated.
// This harness is outside src and is not an application entry point in production.
import { mockIPC } from "@tauri-apps/api/mocks";
const userId = "a".repeat(64);
const unexpected: string[] = [];
Object.assign(window, { uiUnexpectedCommands: unexpected });
mockIPC((command, args) => {
  switch (command) {
    case "load_identity":
      return { publicKey: userId, shortId: "aaaaaaaa", createdAt: "2026-01-01T00:00:00Z", displayName: "Camille", status: "online" };
    case "list_channels":
      return [{ id: "general", name: "Général", createdAt: "2026-01-01T00:00:00Z" }, { id: "projet", name: "Projet", createdAt: "2026-01-01T00:00:00Z" }];
    case "get_all_channel_keys": return { general: "b".repeat(64), projet: "c".repeat(64) };
    case "get_network_status":
      return { peers: [], connectionMode: "disconnected", listenAddresses: [], externalAddresses: [], localPeerId: null, error: null };
    case "get_messages":
      return [
        { id: "one", channelId: (args as { channelId: string }).channelId, senderId: "d".repeat(64), senderDisplayName: "Alex", content: "Bienvenue dans cet aperçu de l’interface. Les données de ce test visuel sont simulées.", timestamp: "2026-01-01T09:00:00Z", reactions: [] },
        { id: "two", channelId: (args as { channelId: string }).channelId, senderId: userId, senderDisplayName: "Camille", content: "Canaux, historique et paramètres sont accessibles depuis cette fenêtre.", timestamp: "2026-01-01T09:01:00Z", reactions: [] },
      ];
    case "list_channel_files": return [];
    case "list_backups": return [];
    case "get_settings": return { serverUrl: "", notificationsEnabled: false };
    case "check_premium": return { isPremium: false, validUntil: null };
    // Reconcile stale native presence after a WebView reload.
    case "end_call": return undefined;
    case "plugin:notification|is_permission_granted": return false;
    case "plugin:notification|request_permission": return "denied";
    case "plugin:updater|check": return null;
    default:
      unexpected.push(command);
      throw new Error("UI fixture does not implement IPC " + command);
  }
}, { shouldMockEvents: true });
await import("../src/main");

