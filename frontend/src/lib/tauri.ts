import { invoke } from "@tauri-apps/api/core";
import type {
  Message,
  Channel,
  IdentityInfo,
  ConnectionMode,
  PremiumStatus,
  ServerInfo,
} from "./types";

// Identity commands
export const createIdentity = () =>
  invoke<IdentityInfo>("create_identity");

export const loadIdentity = () =>
  invoke<IdentityInfo | null>("load_identity");

export const exportPubkey = () =>
  invoke<string>("export_pubkey");

// Network commands
export const connectPeer = (multiaddr: string) =>
  invoke<void>("connect_peer", { multiaddr });

export const listPeers = () =>
  invoke<string[]>("list_peers");

export const getConnectionMode = () =>
  invoke<ConnectionMode>("get_connection_mode");

// Messaging commands
export const sendMessage = (channelId: string, content: string, channelKeyHex: string) =>
  invoke<void>("send_message", { channelId, content, channelKeyHex });

export const getMessages = (channelId: string, channelKeyHex: string, limit: number, offset: number) =>
  invoke<Message[]>("get_messages", { channelId, channelKeyHex, limit, offset });

export const listChannels = () =>
  invoke<Channel[]>("list_channels");

// Media commands
export const startCall = (channelId: string) =>
  invoke<void>("start_call", { channelId });

export const endCall = () =>
  invoke<void>("end_call");

export const toggleMute = () =>
  invoke<boolean>("toggle_mute");

export const toggleVideo = () =>
  invoke<boolean>("toggle_video");

// File commands
export const sendFile = (channelId: string, filePath: string) =>
  invoke<void>("send_file", { channelId, filePath });

export const uploadPremiumBlob = (filePath: string, channelKeyHex: string) =>
  invoke<string>("upload_premium_blob", { filePath, channelKeyHex });

// Premium commands
export const checkPremium = () =>
  invoke<PremiumStatus>("check_premium");

export const activatePremium = (tokenJson: string) =>
  invoke<PremiumStatus>("activate_premium", { tokenJson });

// Settings commands
export const getSettings = () =>
  invoke<Record<string, unknown>>("get_settings");

export const updateSettings = (settings: Record<string, unknown>) =>
  invoke<void>("update_settings", { settings });

// Server info (self-hosted)
export const getServerInfo = (serverUrl: string) =>
  invoke<ServerInfo>("get_server_info", { serverUrl });

// Channel management commands
export interface CreateChannelResult {
  id: string;
  name: string;
  channelKeyHex: string;
}

export const createChannel = (name: string) =>
  invoke<CreateChannelResult>("create_channel", { name });

export const generateInvite = (
  channelId: string,
  channelName: string,
  channelKeyHex: string,
) => invoke<string>("generate_invite", { channelId, channelName, channelKeyHex });

export const acceptInvite = (inviteCode: string) =>
  invoke<CreateChannelResult>("accept_invite", { inviteCode });

export const getAllChannelKeys = () =>
  invoke<Record<string, string>>("get_all_channel_keys");

// Backup commands
export interface BackupFileInfo {
  fileName: string;
  filePath: string;
  sizeBytes: number;
  modified: string;
}

export interface ImportStats {
  channelsImported: number;
  messagesImported: number;
  keysImported: number;
}

export const exportBackup = () =>
  invoke<string>("export_backup");

export const saveBackupToFile = (filePath: string) =>
  invoke<string>("save_backup_to_file", { filePath });

export const autoBackup = () =>
  invoke<string>("auto_backup");

export const importBackup = (json: string) =>
  invoke<ImportStats>("import_backup", { json });

export const listBackups = () =>
  invoke<BackupFileInfo[]>("list_backups");
