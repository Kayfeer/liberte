/// Protocol version string for libp2p identify
pub const PROTOCOL_VERSION: &str = "/liberte/1.0.0";

/// Application name
pub const APP_NAME: &str = "Liberté";

/// XChaCha20-Poly1305 nonce size in bytes
pub const NONCE_SIZE: usize = 24;

/// Ed25519 public key size in bytes
pub const PUBKEY_SIZE: usize = 32;

/// Ed25519 secret key size in bytes
pub const SECRET_KEY_SIZE: usize = 32;

/// Symmetric key size in bytes (for XChaCha20-Poly1305)
pub const SYMMETRIC_KEY_SIZE: usize = 32;

/// Maximum message size in bytes (256 KiB)
pub const MAX_MESSAGE_SIZE: usize = 262_144;

/// Maximum file transfer size in bytes (50 MiB)
pub const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

/// GossipSub heartbeat interval in seconds
pub const GOSSIPSUB_HEARTBEAT_SECS: u64 = 1;

/// Default QUIC listen port
pub const DEFAULT_QUIC_PORT: u16 = 4001;

/// Default HTTP API port (server)
pub const DEFAULT_HTTP_PORT: u16 = 8080;

/// DNS over HTTPS servers
pub const DOH_CLOUDFLARE: &str = "1.1.1.1";
pub const DOH_GOOGLE: &str = "8.8.8.8";

/// Key derivation contexts (BLAKE3)
pub const KDF_CONTEXT_CHANNEL_KEY: &str = "liberte-channel-key-v1";
pub const KDF_CONTEXT_DB_KEY: &str = "liberte-db-key-v1";

/// Premium price in euros
pub const PREMIUM_PRICE_EUR: f64 = 0.99;
