import { invoke } from "@tauri-apps/api/core";
import type {
  Message,
  Channel,
  IdentityInfo,
  ConnectionMode,
  PremiumStatus,
  ServerInfo,
  ReactionGroup,
} from "./types";

// Identity commands
export const createIdentity = (displayName?: string) =>
  invoke<IdentityInfo>("create_identity", { displayName: displayName || null });

export const loadIdentity = () =>
  invoke<IdentityInfo | null>("load_identity");

export const exportPubkey = () =>
  invoke<string>("export_pubkey");

export const setDisplayName = (name: string) =>
  invoke<void>("set_display_name", { name });

export const setBio = (bio: string) =>
  invoke<void>("set_bio", { bio });

export const setStatus = (status: string) =>
  invoke<void>("set_status", { status });

// Network commands
export const connectPeer = (multiaddr: string) =>
  invoke<void>("connect_peer", { multiaddr });

export const listPeers = () =>
  invoke<string[]>("list_peers");

export const getConnectionMode = () =>
  invoke<ConnectionMode>("get_connection_mode");

export interface NetworkStatus {
  peers: string[];
  connectionMode: ConnectionMode;
  listenAddresses: string[];
  externalAddresses: string[];
  localPeerId: string | null;
  error: string | null;
}

export const getNetworkStatus = () => invoke<NetworkStatus>("get_network_status");

// Messaging commands
export const sendMessage = (channelId: string, content: string, channelKeyHex: string) =>
  invoke<void>("send_message", { channelId, content, channelKeyHex });

export const sendTyping = (channelId: string) => invoke<void>("send_typing", { channelId });

export const getMessages = (channelId: string, channelKeyHex: string, limit: number, offset: number) =>
  invoke<Message[]>("get_messages", { channelId, channelKeyHex, limit, offset });

export const listChannels = () =>
  invoke<Channel[]>("list_channels");

export const searchMessages = (query: string, channelId?: string) =>
  invoke<Message[]>("search_messages", { query, channelId });

// Media commands
export const startCall = (channelId: string) =>
  invoke<void>("start_call", { channelId });

export const endCall = () =>
  invoke<void>("end_call");

export const toggleMute = () =>
  invoke<boolean>("toggle_mute");

export const toggleVideo = () =>
  invoke<boolean>("toggle_video");

export const setCallMode = (mode: "mesh" | "sfu") =>
  invoke<string>("set_call_mode", { mode });

export interface CallStateInfo {
  inCall: boolean;
  isMuted: boolean;
  isVideoEnabled: boolean;
  mode: string;
}

export const getCallState = () =>
  invoke<CallStateInfo>("get_call_state");

// File commands
export const sendFile = (channelId: string, filePath: string) =>
  invoke<void>("send_file", { channelId, filePath });

export interface FileTransferDto {
  fileId: string;
  channelId: string;
  fileName: string;
  fileSize: number;
  status: "offered" | "receiving" | "received" | "sending" | "completed" | "failed";
  progress: number;
  senderId: string;
  error?: string | null;
}

export const listChannelFiles = (channelId: string) =>
  invoke<FileTransferDto[]>("list_channel_files", { channelId });
export const acceptFile = (fileId: string) => invoke<void>("accept_file", { fileId });
export const saveReceivedFile = (fileId: string, filePath: string) =>
  invoke<void>("save_received_file", { fileId, filePath });

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

export const exportBackup = (password: string) =>
  invoke<string>("export_backup", { password });

export const saveBackupToFile = (filePath: string, password: string) =>
  invoke<string>("save_backup_to_file", { filePath, password });

export const autoBackup = () =>
  invoke<string>("auto_backup");

export const importBackup = (json: string, password?: string, allowLegacy = false) =>
  invoke<ImportStats>("import_backup", { json, password, allowLegacy });

export const listBackups = () =>
  invoke<BackupFileInfo[]>("list_backups");

// Profile commands
export interface ProfileImportResult {
  publicKey: string;
  channelsImported: number;
}

export const exportProfile = (password: string) =>
  invoke<string>("export_profile", { password });

export const saveProfileToFile = (filePath: string, password: string) =>
  invoke<string>("save_profile_to_file", { filePath, password });

export const importProfile = (json: string, password?: string, allowLegacy = false) =>
  invoke<ProfileImportResult>("import_profile", { json, password, allowLegacy });

// Reaction commands
export const addReaction = (channelId: string, messageId: string, emoji: string) =>
  invoke<void>("add_reaction", { channelId, messageId, emoji });

export const removeReaction = (messageId: string, emoji: string) =>
  invoke<void>("remove_reaction", { messageId, emoji });

export const getReactions = (messageId: string) =>
  invoke<ReactionGroup[]>("get_reactions", { messageId });
