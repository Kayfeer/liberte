import { invoke } from "@tauri-apps/api/core";
import type {
  Message,
  Channel,
  IdentityInfo,
  ConnectionMode,
  CallState,
  UserSettings,
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
export const sendMessage = (channelId: string, content: string) =>
  invoke<void>("send_message", { channelId, content });

export const getMessages = (channelId: string, limit: number, offset: number) =>
  invoke<Message[]>("get_messages", { channelId, limit, offset });

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

export const uploadPremiumBlob = (channelId: string, filePath: string) =>
  invoke<void>("upload_premium_blob", { channelId, filePath });

// Premium commands
export const checkPremium = () =>
  invoke<boolean>("check_premium");

export const activatePremium = (token: string) =>
  invoke<void>("activate_premium", { token });

// Settings commands
export const getSettings = () =>
  invoke<UserSettings>("get_settings");

export const updateSettings = (settings: UserSettings) =>
  invoke<void>("update_settings", { settings });

// Server info (self-hosted)
export const getServerInfo = (serverUrl: string) =>
  invoke<ServerInfo>("get_server_info", { serverUrl });
