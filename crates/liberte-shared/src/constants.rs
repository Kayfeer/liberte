pub const PROTOCOL_VERSION: &str = "/liberte/1.0.0";
pub const APP_NAME: &str = "Liberté";

pub const NONCE_SIZE: usize = 24;
pub const PUBKEY_SIZE: usize = 32;
pub const SECRET_KEY_SIZE: usize = 32;
pub const SYMMETRIC_KEY_SIZE: usize = 32;

pub const MAX_MESSAGE_SIZE: usize = 262_144; // 256 KiB
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024; // 50 MiB

pub const GOSSIPSUB_HEARTBEAT_SECS: u64 = 1;
pub const DEFAULT_QUIC_PORT: u16 = 4001;
pub const DEFAULT_HTTP_PORT: u16 = 8080;

pub const DOH_CLOUDFLARE: &str = "1.1.1.1";
pub const DOH_GOOGLE: &str = "8.8.8.8";

// BLAKE3 KDF contexts
pub const KDF_CONTEXT_CHANNEL_KEY: &str = "liberte-channel-key-v1";
pub const KDF_CONTEXT_DB_KEY: &str = "liberte-db-key-v1";

pub const PREMIUM_PRICE_EUR: f64 = 0.99;
