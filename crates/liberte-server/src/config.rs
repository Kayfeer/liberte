use std::{fmt, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub public_relay_addr: Option<String>,
    pub http_addr: SocketAddr,
    pub blob_storage_path: PathBuf,
    pub payment_server_pubkey: [u8; 32],
    pub max_blob_size: usize,
    pub max_user_bytes: u64,
    pub max_total_bytes: u64,
    pub max_objects: usize,
    pub max_replay_entries: usize,
    pub max_http_concurrency: usize,
    pub instance_name: String,
    pub premium_required: bool,
    pub admin_token: Option<String>,
    pub registration_open: bool,
    pub max_peers: usize,
}

impl fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerConfig")
            .field("listen_addr", &self.listen_addr)
            .field("public_relay_addr", &self.public_relay_addr)
            .field("http_addr", &self.http_addr)
            .field("blob_storage_path", &self.blob_storage_path)
            .field("instance_name", &self.instance_name)
            .field("premium_required", &self.premium_required)
            .field("registration_open", &self.registration_open)
            .field("max_peers", &self.max_peers)
            .field("max_blob_size", &self.max_blob_size)
            .field("max_user_bytes", &self.max_user_bytes)
            .field("max_total_bytes", &self.max_total_bytes)
            .field("max_objects", &self.max_objects)
            .field("max_http_concurrency", &self.max_http_concurrency)
            .field("admin_token", &self.admin_token.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "/ip4/0.0.0.0/udp/4001/quic-v1".to_string(),
            public_relay_addr: None,
            http_addr: ([0, 0, 0, 0], 8080).into(),
            blob_storage_path: PathBuf::from("./blobs"),
            payment_server_pubkey: [0; 32],
            max_blob_size: 50 * 1024 * 1024,
            max_user_bytes: 250 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_objects: 10_000,
            max_replay_entries: 10_000,
            max_http_concurrency: 8,
            instance_name: "Liberte Node".to_string(),
            premium_required: true,
            admin_token: None,
            registration_open: true,
            max_peers: 128,
        }
    }
}

impl ServerConfig {
    /// Malformed security limits fail startup instead of silently disabling limits.
    pub fn from_env() -> anyhow::Result<Self> {
        let mut config = Self::default();
        if let Ok(value) = std::env::var("LISTEN_ADDR") { config.listen_addr = value; }
        if let Ok(value) = std::env::var("PUBLIC_RELAY_ADDR") { config.public_relay_addr = Some(value); }
        if let Ok(value) = std::env::var("HTTP_ADDR") { config.http_addr = value.parse()?; }
        if let Ok(value) = std::env::var("BLOB_STORAGE_PATH") { config.blob_storage_path = value.into(); }
        if let Ok(value) = std::env::var("PAYMENT_SERVER_PUBKEY") {
            config.payment_server_pubkey = parse_hex_pubkey(&value).map_err(anyhow::Error::msg)?;
        }
        if let Ok(value) = std::env::var("INSTANCE_NAME") { config.instance_name = value; }
        if let Ok(value) = std::env::var("ADMIN_TOKEN") {
            anyhow::ensure!(value.len() >= 32, "ADMIN_TOKEN must contain at least 32 characters");
            config.admin_token = Some(value);
        }
        config.premium_required = env_bool("PREMIUM_REQUIRED", config.premium_required)?;
        config.registration_open = env_bool("REGISTRATION_OPEN", config.registration_open)?;
        config.max_blob_size = env_limit("MAX_BLOB_SIZE", config.max_blob_size, 256 * 1024 * 1024)?;
        config.max_user_bytes = env_limit("MAX_USER_BYTES", config.max_user_bytes as usize, usize::MAX)? as u64;
        config.max_total_bytes = env_limit("MAX_TOTAL_BYTES", config.max_total_bytes as usize, usize::MAX)? as u64;
        config.max_objects = env_limit("MAX_OBJECTS", config.max_objects, 100_000)?;
        config.max_replay_entries = env_limit("MAX_REPLAY_ENTRIES", config.max_replay_entries, 100_000)?;
        config.max_http_concurrency = env_limit("MAX_HTTP_CONCURRENCY", config.max_http_concurrency, 64)?;
        config.max_peers = env_limit("MAX_PEERS", config.max_peers, 4096)?;
        anyhow::ensure!(config.max_user_bytes >= config.max_blob_size as u64, "MAX_USER_BYTES must be >= MAX_BLOB_SIZE");
        anyhow::ensure!(config.max_total_bytes >= config.max_user_bytes, "MAX_TOTAL_BYTES must be >= MAX_USER_BYTES");
        Ok(config)
    }
}

fn env_limit(name: &str, default: usize, maximum: usize) -> anyhow::Result<usize> {
    let Some(value) = std::env::var(name).ok() else { return Ok(default); };
    let value: usize = value.parse().map_err(|_| anyhow::anyhow!("{name} must be an integer"))?;
    anyhow::ensure!(value > 0 && value <= maximum, "{name} must be between 1 and {maximum}");
    Ok(value)
}

fn env_bool(name: &str, default: bool) -> anyhow::Result<bool> {
    match std::env::var(name).as_deref() {
        Ok("true" | "1") => Ok(true),
        Ok("false" | "0") => Ok(false),
        Err(_) => Ok(default),
        _ => anyhow::bail!("{name} must be true, false, 1 or 0"),
    }
}

fn parse_hex_pubkey(value: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(value.trim()).map_err(|_| "invalid public key hex".to_string())?;
    bytes.try_into().map_err(|_| "public key must contain 32 bytes".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_bounded() {
        let config = ServerConfig::default();
        assert!(config.max_peers > 0);
        assert!(config.max_total_bytes >= config.max_user_bytes);
        assert_eq!(config.http_addr, ([0, 0, 0, 0], 8080).into());
    }

    #[test]
    fn debug_never_contains_admin_secret() {
        let config = ServerConfig { admin_token: Some("do-not-log-this-secret".into()), ..ServerConfig::default() };
        let output = format!("{config:?}");
        assert!(!output.contains("do-not-log-this-secret"));
        assert!(output.contains("[REDACTED]"));
    }

    #[test]
    fn parses_public_key() {
        assert_eq!(parse_hex_pubkey(&"ab".repeat(32)).unwrap(), [0xab; 32]);
        assert!(parse_hex_pubkey("abcd").is_err());
    }
}
