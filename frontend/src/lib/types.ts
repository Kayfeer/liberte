/** User identity = hex-encoded Ed25519 public key */
export type UserId = string;

/** UUID string */
export type ChannelId = string;
export type ServerId = string;

/** Connection mode between peers */
export type ConnectionMode = "direct" | "relayed" | "disconnected";

/** User status (Discord-style) */
export type UserStatus = "online" | "dnd" | "idle" | "invisible";

/** A grouped emoji reaction */
export interface ReactionGroup {
  emoji: string;
  users: UserId[];
}

/** A chat message */
export interface Message {
  id: string;
  channelId: ChannelId;
  senderId: UserId;
  senderDisplayName?: string;
  content: string;
  timestamp: string;
  reactions: ReactionGroup[];
}

/** A channel */
export interface Channel {
  id: ChannelId;
  name: string;
  serverId?: ServerId;
  createdAt: string;
}

/** A server (guild) */
export interface Server {
  id: ServerId;
  name: string;
  ownerId: UserId;
  createdAt: string;
}

/** Peer info in a call */
export interface PeerInfo {
  userId: UserId;
  displayName: string;
  isMuted: boolean;
  isVideoEnabled: boolean;
  state: "connecting" | "connected" | "disconnected" | "failed";
}

/** Call state */
export interface CallState {
  inCall: boolean;
  channelId?: ChannelId;
  participants: PeerInfo[];
  isMuted: boolean;
  isVideoEnabled: boolean;
  mode: "mesh" | "sfu";
}

/** User settings */
export interface UserSettings {
  displayName: string;
  audioInputDevice?: string;
  audioOutputDevice?: string;
  videoDevice?: string;
  notificationsEnabled: boolean;
  serverUrl: string;
}

/** Public info returned by a Liberté server instance */
export interface ServerInfo {
  name: string;
  version: string;
  premiumRequired: boolean;
  registrationOpen: boolean;
  maxPeers: number;
}

/** Identity info for display */
export interface IdentityInfo {
  publicKey: string;
  shortId: string;
  createdAt: string;
  displayName?: string;
  bio?: string;
  status: UserStatus;
}

/** Premium subscription status */
export interface PremiumStatus {
  isPremium: boolean;
  validUntil: string | null;
}

/** Typing indicator event payload */
export interface TypingEvent {
  channelId: ChannelId;
  userId: UserId;
  displayName?: string;
}
