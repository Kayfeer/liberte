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
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,liberte_server=debug")),
        )
        .init();

    info!(
        "Starting Liberte relay server v{}",
        env!("CARGO_PKG_VERSION")
    );

    let config = ServerConfig::from_env();
    info!(?config, "Loaded configuration");
    info!(
        instance = %config.instance_name,
        premium_required = config.premium_required,
        registration_open = config.registration_open,
        admin_enabled = config.admin_token.is_some(),
        "Self-hosted instance settings"
    );

    let blob_store =
        Arc::new(BlobStore::new(config.blob_storage_path.clone(), config.max_blob_size).await?);

    let premium_verifier = Arc::new(PremiumVerifier::new(config.payment_server_pubkey));

    // 10 req/s sustained, burst of 30
    let rate_limiter = RateLimiter::default();

    let _sfu_manager = sfu::SfuManager::new();

    let app_state = AppState {
        blob_store,
        premium_verifier,
        rate_limiter: rate_limiter.clone(),
        config: Arc::new(config.clone()),
    };

    // Rate limiter cleanup every 5 min, evict buckets idle >10 min
    let rl = rate_limiter.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        loop {
            interval.tick().await;
            rl.purge_stale(600.0).await;
        }
    });

    // Premium cache cleanup every 10 min
    let pv = app_state.premium_verifier.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
        loop {
            interval.tick().await;
            pv.purge_expired().await;
        }
    });

    let listen_addr = config.listen_addr.clone();
    let http_addr = config.http_addr;

    let relay_peer_id = relay::spawn_relay(&listen_addr).await?;
    info!(
        peer_id = %relay_peer_id,
        addr = %listen_addr,
        "Relay server running in background"
    );

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
