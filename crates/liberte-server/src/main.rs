//! # liberte-server
//!
//! VPS relay server for the Liberte network.
//!
//! This binary provides:
//! - **libp2p circuit relay v2** so that peers behind NAT can reach each other
//! - **SFU** (Selective Forwarding Unit) that routes encrypted media frames
//!   without ever decrypting them
//! - **Encrypted blob storage** for premium users (files stored as opaque
//!   ciphertext on disk)
//! - **REST API** (axum) for health checks, premium verification, and blob
//!   upload/download
//! - **Per-IP rate limiting** to protect against abuse

mod api;
mod blob_store;
mod config;
mod error;
mod premium;
mod rate_limit;
mod relay;
mod sfu;

use std::sync::Arc;

use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::api::AppState;
use crate::blob_store::BlobStore;
use crate::config::ServerConfig;
use crate::premium::PremiumVerifier;
use crate::rate_limit::RateLimiter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // -----------------------------------------------------------------------
    // 1. Initialize tracing (respects RUST_LOG env var)
    // -----------------------------------------------------------------------
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,liberte_server=debug")),
        )
        .init();

    info!("Starting Liberte relay server v{}", env!("CARGO_PKG_VERSION"));

    // -----------------------------------------------------------------------
    // 2. Load configuration
    // -----------------------------------------------------------------------
    let config = ServerConfig::from_env();
    info!(?config, "Loaded configuration");
    info!(
        instance = %config.instance_name,
        premium_required = config.premium_required,
        registration_open = config.registration_open,
        admin_enabled = config.admin_token.is_some(),
        "Self-hosted instance settings"
    );

    // -----------------------------------------------------------------------
    // 3. Initialize subsystems
    // -----------------------------------------------------------------------

    // Blob store (creates directory if missing)
    let blob_store = Arc::new(
        BlobStore::new(config.blob_storage_path.clone(), config.max_blob_size).await?,
    );

    // Premium verifier with payment server public key
    let premium_verifier = Arc::new(PremiumVerifier::new(config.payment_server_pubkey));

    // Rate limiter: 10 req/s sustained, burst of 30
    let rate_limiter = RateLimiter::default();

    // SFU manager (available for future integration into the p2p layer)
    let _sfu_manager = sfu::SfuManager::new();

    // Application state for the HTTP API
    let app_state = AppState {
        blob_store,
        premium_verifier,
        rate_limiter: rate_limiter.clone(),
        config: Arc::new(config.clone()),
    };

    // -----------------------------------------------------------------------
    // 4. Spawn background tasks
    // -----------------------------------------------------------------------

    // Periodic rate limiter cleanup (every 5 minutes, evict buckets idle >10 min)
    let rl = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            rl.purge_stale(600.0).await;
        }
    });

    // Periodic premium cache cleanup (every 10 minutes)
    let pv = app_state.premium_verifier.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
        loop {
            interval.tick().await;
            pv.purge_expired().await;
        }
    });

    // -----------------------------------------------------------------------
    // 5. Spawn the libp2p relay (runs in background tokio task)
    // -----------------------------------------------------------------------
    let listen_addr = config.listen_addr.clone();
    let http_addr = config.http_addr;

    let relay_peer_id = relay::spawn_relay(&listen_addr).await?;
    info!(
        peer_id = %relay_peer_id,
        addr = %listen_addr,
        "Relay server running in background"
    );

    // -----------------------------------------------------------------------
    // 6. Run the HTTP API server (blocks until shutdown)
    // -----------------------------------------------------------------------
    // tokio::select! ensures that if either the HTTP server or a shutdown
    // signal arrives, we exit cleanly.
    tokio::select! {
        result = api::serve(app_state, http_addr) => {
            if let Err(e) = result {
                tracing::error!(error = %e, "HTTP server failed");
                return Err(e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            info!("Received Ctrl+C, shutting down");
        }
    }

    Ok(())
}
