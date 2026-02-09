//! Server configuration loaded from environment variables.
//!
//! All settings have sensible defaults so the server can start with zero
//! configuration for local development.

use std::net::SocketAddr;
use std::path::PathBuf;

/// Server configuration.
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// libp2p multiaddr to listen on (QUIC).
    /// Env: `LISTEN_ADDR`
    /// Default: `/ip4/0.0.0.0/udp/4001/quic-v1`
    pub listen_addr: String,

    /// Socket address for the HTTP (axum) API server.
    /// Env: `HTTP_ADDR`
    /// Default: `0.0.0.0:8080`
    pub http_addr: SocketAddr,

    /// Filesystem path where encrypted blobs are stored.
    /// Env: `BLOB_STORAGE_PATH`
    /// Default: `./blobs`
    pub blob_storage_path: PathBuf,

    /// Ed25519 public key of the payment server (hex-encoded, 64 chars).
    /// Env: `PAYMENT_SERVER_PUBKEY`
    /// Default: all-zeros (development only).
    pub payment_server_pubkey: [u8; 32],

    /// Maximum blob size in bytes (50 MiB).
    pub max_blob_size: usize,

    // -- Self-hosted instance settings --

    /// Human-readable name for this server instance.
    /// Env: `INSTANCE_NAME`
    /// Default: `"Liberté Node"`
    pub instance_name: String,

    /// Whether premium verification is required for SFU/blob access.
    /// Self-hosted admins can disable this to grant full access to all users.
    /// Env: `PREMIUM_REQUIRED` (true/false)
    /// Default: `true`
    pub premium_required: bool,

    /// Admin API bearer token. Required to access /admin/* endpoints.
    /// Env: `ADMIN_TOKEN`
    /// Default: empty (admin API disabled).
    pub admin_token: Option<String>,

    /// Whether new users can connect freely.
    /// Env: `REGISTRATION_OPEN` (true/false)
    /// Default: `true`
    pub registration_open: bool,

    /// Maximum number of concurrent connected peers (0 = unlimited).
    /// Env: `MAX_PEERS`
    /// Default: `0`
    pub max_peers: usize,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".to_string(),
            http_addr: ([0, 0, 0, 0], 8080).into(),
            blob_storage_path: PathBuf::from("./blobs"),
            payment_server_pubkey: [0u8; 32],
            max_blob_size: 50 * 1024 * 1024, // 50 MiB
            instance_name: "Liberté Node".to_string(),
            premium_required: true,
            admin_token: None,
            registration_open: true,
            max_peers: 0,
        }
    }
}

impl ServerConfig {
    /// Load configuration from environment variables, falling back to defaults.
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(addr) = std::env::var("LISTEN_ADDR") {
            config.listen_addr = addr;
        }

        if let Ok(addr) = std::env::var("HTTP_ADDR") {
            if let Ok(parsed) = addr.parse::<SocketAddr>() {
                config.http_addr = parsed;
            } else {
                tracing::warn!(
                    value = %addr,
                    "Invalid HTTP_ADDR, using default"
                );
            }
        }

        if let Ok(path) = std::env::var("BLOB_STORAGE_PATH") {
            config.blob_storage_path = PathBuf::from(path);
        }

        if let Ok(hex_key) = std::env::var("PAYMENT_SERVER_PUBKEY") {
            match parse_hex_pubkey(&hex_key) {
                Ok(key) => config.payment_server_pubkey = key,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "Invalid PAYMENT_SERVER_PUBKEY, using default (dev-only)"
                    );
                }
            }
        }

        // -- Self-hosted settings --

        if let Ok(name) = std::env::var("INSTANCE_NAME") {
            config.instance_name = name;
        }

        if let Ok(val) = std::env::var("PREMIUM_REQUIRED") {
            config.premium_required = val != "false" && val != "0";
        }

        if let Ok(token) = std::env::var("ADMIN_TOKEN") {
            if !token.is_empty() {
                config.admin_token = Some(token);
            }
        }

        if let Ok(val) = std::env::var("REGISTRATION_OPEN") {
            config.registration_open = val != "false" && val != "0";
        }

        if let Ok(val) = std::env::var("MAX_PEERS") {
            if let Ok(n) = val.parse::<usize>() {
                config.max_peers = n;
            }
        }

        // RUST_LOG is handled directly by tracing-subscriber's EnvFilter,
        // so we do not store it here.

        config
    }
}

/// Parse a 64-character hex string into a 32-byte array.
fn parse_hex_pubkey(hex: &str) -> Result<[u8; 32], String> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err(format!("expected 64 hex chars, got {}", hex.len()));
    }

    let mut bytes = [0u8; 32];
    for (i, chunk) in hex.as_bytes().chunks(2).enumerate() {
        let hi = hex_digit(chunk[0])?;
        let lo = hex_digit(chunk[1])?;
        bytes[i] = (hi << 4) | lo;
    }
    Ok(bytes)
}

fn hex_digit(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit: {}", c as char)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ServerConfig::default();
        assert_eq!(config.http_addr, ([0, 0, 0, 0], 8080).into());
        assert_eq!(config.payment_server_pubkey, [0u8; 32]);
    }

    #[test]
    fn test_parse_hex_pubkey() {
        let hex = "ab".repeat(32);
        let key = parse_hex_pubkey(&hex).unwrap();
        assert_eq!(key, [0xab; 32]);
    }

    #[test]
    fn test_parse_hex_pubkey_wrong_length() {
        assert!(parse_hex_pubkey("abcd").is_err());
    }
}
