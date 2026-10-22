use std::{fmt, net::SocketAddr, path::PathBuf};

#[derive(Clone)]
pub struct ServerConfig {
    pub listen_addr: String,
    pub public_relay_addr: Option<String>,
    pub livekit: Option<LiveKitConfig>,
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
            .field("livekit", &self.livekit)
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
            livekit: None,
            http_addr: ([0, 0, 0, 0], 8080).into(),
            blob_storage_path: PathBuf::from("./blobs"),
            payment_server_pubkey: [0; 32],
            max_blob_size: 50 * 1024 * 1024,
            max_user_bytes: 250 * 1024 * 1024,
            max_total_bytes: 1024 * 1024 * 1024,
            max_objects: 10_000,
            max_replay_entries: 10_000,
            max_http_concurrency: 2,
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
        if let Ok(value) = std::env::var("PUBLIC_RELAY_ADDR") {
            if !value.is_empty() { config.public_relay_addr = Some(value); }
        }
        if let Ok(value) = std::env::var("HTTP_ADDR") { config.http_addr = value.parse()?; }
        if let Ok(value) = std::env::var("BLOB_STORAGE_PATH") { config.blob_storage_path = value.into(); }
        if let Ok(value) = std::env::var("PAYMENT_SERVER_PUBKEY") {
            config.payment_server_pubkey = parse_hex_pubkey(&value).map_err(anyhow::Error::msg)?;
        }
        if let Ok(value) = std::env::var("INSTANCE_NAME") { config.instance_name = value; }
        if let Ok(value) = std::env::var("ADMIN_TOKEN") {
            if !value.is_empty() {
                anyhow::ensure!(value.len() >= 32, "ADMIN_TOKEN must contain at least 32 characters");
                config.admin_token = Some(value);
            }
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
        config.livekit = LiveKitConfig::from_env()?;
        anyhow::ensure!(config.max_user_bytes >= config.max_blob_size as u64, "MAX_USER_BYTES must be >= MAX_BLOB_SIZE");
        anyhow::ensure!(config.max_total_bytes >= config.max_user_bytes, "MAX_TOTAL_BYTES must be >= MAX_USER_BYTES");
        Ok(config)
    }
}

#[derive(Clone)]
pub struct LiveKitConfig {
    pub url: String,
    pub api_key: String,
    pub api_secret: String,
}

impl fmt::Debug for LiveKitConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LiveKitConfig")
            .field("url", &self.url)
            .field("api_key", &"[REDACTED]")
            .field("api_secret", &"[REDACTED]")
            .finish()
    }
}

impl LiveKitConfig {
    fn from_env() -> anyhow::Result<Option<Self>> {
        let url = std::env::var("LIVEKIT_URL").ok().filter(|value| !value.is_empty());
        let key = std::env::var("LIVEKIT_API_KEY").ok().filter(|value| !value.is_empty());
        let secret = std::env::var("LIVEKIT_API_SECRET").ok().filter(|value| !value.is_empty());
        match (url, key, secret) {
            (None, None, None) => Ok(None),
            (Some(url), Some(api_key), Some(api_secret)) => {
                let config = Self { url, api_key, api_secret };
                config.validate()?;
                Ok(Some(config))
            }
            _ => anyhow::bail!("LIVEKIT_URL, LIVEKIT_API_KEY and LIVEKIT_API_SECRET must be configured together"),
        }
    }

    fn validate(&self) -> anyhow::Result<()> {
        let url = url::Url::parse(&self.url).map_err(|_| anyhow::anyhow!("Invalid LIVEKIT_URL"))?;
        let local = url.host_str().is_some_and(|host| host.eq_ignore_ascii_case("localhost")
            || host.trim_matches(['[', ']']).parse::<std::net::IpAddr>().is_ok_and(|ip| ip.is_loopback()));
        anyhow::ensure!(url.scheme() == "wss" || (url.scheme() == "ws" && local), "LIVEKIT_URL requires wss outside loopback");
        anyhow::ensure!(url.username().is_empty() && url.password().is_none() && url.query().is_none() && url.fragment().is_none(), "LIVEKIT_URL cannot contain credentials, query or fragment");
        anyhow::ensure!(self.api_key.len() <= 128 && self.api_key.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_')), "Invalid LIVEKIT_API_KEY");
        anyhow::ensure!(self.api_secret.len() >= 32 && self.api_secret.len() <= 1024, "LIVEKIT_API_SECRET must contain 32 to 1024 characters");
        Ok(())
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

    #[test]
    fn media_configuration_masks_secrets_and_requires_secure_transport() {
        let mut config = LiveKitConfig { url: "wss://media.example.test".into(), api_key: "test-key".into(), api_secret: "test-secret-longer-than-32-characters".into() };
        assert!(config.validate().is_ok());
        let debug = format!("{config:?}");
        assert!(!debug.contains(&config.api_key));
        assert!(!debug.contains(&config.api_secret));
        config.url = "ws://media.example.test".into();
        assert!(config.validate().is_err());
        config.url = "ws://127.0.0.1:7880".into();
        assert!(config.validate().is_ok());
        config.url = "wss://name:password@media.example.test".into();
        assert!(config.validate().is_err());
    }
}
