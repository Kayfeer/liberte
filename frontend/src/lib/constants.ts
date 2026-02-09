export const APP_NAME = "Liberté";
export const APP_VERSION = "0.1.1";

// Tauri event names
export const EVENTS = {
  NEW_MESSAGE: "new-message",
  PEER_CONNECTED: "peer-connected",
  PEER_DISCONNECTED: "peer-disconnected",
  CALL_STATE_CHANGED: "call-state-changed",
  CONNECTION_MODE_CHANGED: "connection-mode-changed",
} as const;

// Message limits
export const MESSAGE_PAGE_SIZE = 50;
export const MAX_MESSAGE_LENGTH = 4000;
export const MAX_FILE_SIZE = 50 * 1024 * 1024; // 50 MiB
