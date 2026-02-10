export const APP_NAME = "Liberté";
export const APP_VERSION = "0.3.0";

// Tauri event names
export const EVENTS = {
  NEW_MESSAGE: "new-message",
  PEER_CONNECTED: "peer-connected",
  PEER_DISCONNECTED: "peer-disconnected",
  CALL_STATE_CHANGED: "call-state-changed",
  CONNECTION_MODE_CHANGED: "connection-mode-changed",
  TYPING_INDICATOR: "typing-indicator",
  STATUS_CHANGED: "status-changed",
  MESSAGE_REACTION: "message-reaction",
} as const;

// Message limits
export const MESSAGE_PAGE_SIZE = 50;
export const MAX_MESSAGE_LENGTH = 4000;
export const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50 MiB

// Typing indicator timeout (ms)
export const TYPING_TIMEOUT = 5000;
export const TYPING_THROTTLE = 2000;

// Common emoji reactions (Discord-style quick picker)
export const QUICK_REACTIONS = ["👍", "❤️", "😂", "😮", "😢", "🎉", "🔥", "👀"];
