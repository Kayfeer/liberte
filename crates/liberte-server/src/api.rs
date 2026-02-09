use std::sync::Arc;

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{HeaderMap, Method},
    middleware,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;
use uuid::Uuid;

use crate::blob_store::BlobStore;
use crate::config::ServerConfig;
use crate::error::ServerError;
use crate::premium::PremiumVerifier;
use crate::rate_limit::{rate_limit_middleware, RateLimiter};

use liberte_shared::premium::PremiumToken;

#[derive(Clone)]
pub struct AppState {
    pub blob_store: Arc<BlobStore>,
    pub premium_verifier: Arc<PremiumVerifier>,
    pub rate_limiter: RateLimiter,
    pub config: Arc<ServerConfig>,
}

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);

    Router::new()
        .route("/health", get(health_check))
        .route("/info", get(server_info))
        .route("/premium/verify", post(premium_verify))
        .route("/blob/upload", post(blob_upload))
        .route("/blob/{id}", get(blob_download))
        .route("/blob/{id}", delete(blob_delete))
        .route("/backup/sync", post(backup_sync_upload))
        .route("/backup/{pubkey_hex}", get(backup_sync_download))
        .route("/admin/status", get(admin_status))
        .route("/admin/grant-premium", post(admin_grant_premium))
        .route("/admin/revoke-premium", post(admin_revoke_premium))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.rate_limiter.clone(),
            rate_limit_middleware,
        ))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct PremiumVerifyResponse {
    valid: bool,
}

#[derive(Serialize)]
struct BlobUploadResponse {
    id: Uuid,
}

#[derive(Serialize)]
#[allow(dead_code)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct ServerInfoResponse {
    name: String,
    version: &'static str,
    premium_required: bool,
    registration_open: bool,
    max_peers: usize,
}

#[derive(Serialize)]
struct AdminStatusResponse {
    name: String,
    premium_required: bool,
    registration_open: bool,
    max_peers: usize,
    uptime_secs: u64,
}

#[derive(Deserialize)]
struct AdminPremiumRequest {
    user_pubkey_hex: String,
}

async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn premium_verify(
    State(state): State<AppState>,
    Json(token): Json<PremiumToken>,
) -> Json<PremiumVerifyResponse> {
    if !state.config.premium_required {
        return Json(PremiumVerifyResponse { valid: true });
    }
    let valid = state.premium_verifier.verify(&token).await;
    Json(PremiumVerifyResponse { valid })
}

async fn blob_upload(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<BlobUploadResponse>, ServerError> {
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ServerError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|e| ServerError::BadRequest(format!("Failed to read field: {}", e)))?;

            let id = state.blob_store.store_blob(&data).await?;

            info!(id = %id, size = data.len(), "Blob uploaded via API");

            return Ok(Json(BlobUploadResponse { id }));
        }
    }

    Err(ServerError::BadRequest(
        "Missing 'file' field in multipart form".to_string(),
    ))
}

async fn blob_download(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Vec<u8>, ServerError> {
    let data = state.blob_store.get_blob(id).await?;
    Ok(data)
}

async fn blob_delete(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.blob_store.delete_blob(id).await?;
    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn server_info(State(state): State<AppState>) -> Json<ServerInfoResponse> {
    Json(ServerInfoResponse {
        name: state.config.instance_name.clone(),
        version: env!("CARGO_PKG_VERSION"),
        premium_required: state.config.premium_required,
        registration_open: state.config.registration_open,
        max_peers: state.config.max_peers,
    })
}

fn verify_admin_token(headers: &HeaderMap, config: &ServerConfig) -> Result<(), ServerError> {
    let Some(ref expected) = config.admin_token else {
        return Err(ServerError::Forbidden(
            "Admin API is disabled (no ADMIN_TOKEN configured)".into(),
        ));
    };

    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let token = auth.strip_prefix("Bearer ").unwrap_or(auth);

    // Constant-time comparison to prevent timing attacks on admin token.
    use subtle::ConstantTimeEq;
    let token_bytes = token.as_bytes();
    let expected_bytes = expected.as_bytes();
    if token_bytes.len() != expected_bytes.len()
        || token_bytes.ct_eq(expected_bytes).unwrap_u8() != 1
    {
        return Err(ServerError::Forbidden("Invalid admin token".into()));
    }

    Ok(())
}

async fn admin_status(
    headers: HeaderMap,
    State(state): State<AppState>,
) -> Result<Json<AdminStatusResponse>, ServerError> {
    verify_admin_token(&headers, &state.config)?;

    Ok(Json(AdminStatusResponse {
        name: state.config.instance_name.clone(),
        premium_required: state.config.premium_required,
        registration_open: state.config.registration_open,
        max_peers: state.config.max_peers,
        uptime_secs: 0, // TODO: track with start instant
    }))
}

async fn admin_grant_premium(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AdminPremiumRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    verify_admin_token(&headers, &state.config)?;

    let pubkey = parse_hex_32(&req.user_pubkey_hex)?;
    state.premium_verifier.admin_grant(&pubkey).await;

    info!(user = %req.user_pubkey_hex, "Admin granted premium");
    Ok(Json(serde_json::json!({ "granted": true })))
}

async fn admin_revoke_premium(
    headers: HeaderMap,
    State(state): State<AppState>,
    Json(req): Json<AdminPremiumRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    verify_admin_token(&headers, &state.config)?;

    let pubkey = parse_hex_32(&req.user_pubkey_hex)?;
    state.premium_verifier.admin_revoke(&pubkey).await;

    info!(user = %req.user_pubkey_hex, "Admin revoked premium");
    Ok(Json(serde_json::json!({ "revoked": true })))
}

fn parse_hex_32(hex: &str) -> Result<[u8; 32], ServerError> {
    let hex = hex.trim();
    if hex.len() != 64 {
        return Err(ServerError::BadRequest(format!(
            "Expected 64 hex chars, got {}",
            hex.len()
        )));
    }
    let bytes =
        hex::decode(hex).map_err(|e| ServerError::BadRequest(format!("Invalid hex: {e}")))?;
    let mut arr = [0u8; 32];
    arr.copy_from_slice(&bytes);
    Ok(arr)
}

// ─── Backup sync endpoints ───

#[derive(Deserialize)]
struct BackupSyncRequest {
    /// User's Ed25519 pubkey (hex, 64 chars)
    user_pubkey_hex: String,
    /// Encrypted backup data (the client encrypts before sending)
    encrypted_data: String,
}

#[derive(Serialize)]
struct BackupSyncResponse {
    stored: bool,
    size_bytes: usize,
}

/// Upload an encrypted backup blob, keyed by user pubkey.
/// Overwrites any previous backup for that user.
async fn backup_sync_upload(
    State(state): State<AppState>,
    Json(req): Json<BackupSyncRequest>,
) -> Result<Json<BackupSyncResponse>, ServerError> {
    // Validate pubkey is exactly 64 hex chars (rejects any path-traversal chars)
    let _pubkey = parse_hex_32(&req.user_pubkey_hex)?;
    let data = req.encrypted_data.as_bytes();

    // Store backup in a dedicated sub-path: backups/<pubkey_hex>.enc
    let backup_dir = state.blob_store.base_path().join("backups");
    tokio::fs::create_dir_all(&backup_dir)
        .await
        .map_err(|e| ServerError::Internal(format!("Failed to create backup dir: {e}")))?;

    let filename = format!("{}.enc", req.user_pubkey_hex);
    let file_path = state
        .blob_store
        .safe_subpath("backups", &filename)?;
    tokio::fs::write(&file_path, data)
        .await
        .map_err(|e| ServerError::Internal(format!("Failed to write backup: {e}")))?;

    info!(
        user = %req.user_pubkey_hex,
        size = data.len(),
        "Backup synced to server"
    );

    Ok(Json(BackupSyncResponse {
        stored: true,
        size_bytes: data.len(),
    }))
}

/// Download the encrypted backup for a given user pubkey.
async fn backup_sync_download(
    State(state): State<AppState>,
    Path(pubkey_hex): Path<String>,
) -> Result<String, ServerError> {
    // Validate pubkey is exactly 64 hex chars (rejects any path-traversal chars)
    let _pubkey = parse_hex_32(&pubkey_hex)?;

    let filename = format!("{pubkey_hex}.enc");
    let file_path = state
        .blob_store
        .safe_subpath("backups", &filename)?;

    if !file_path.exists() {
        return Err(ServerError::NotFound(
            "No backup found for this user".into(),
        ));
    }

    let data = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|e| ServerError::Internal(format!("Failed to read backup: {e}")))?;

    Ok(data)
}

pub async fn serve(state: AppState, addr: std::net::SocketAddr) -> anyhow::Result<()> {
    let app = build_router(state);

    info!(addr = %addr, "Starting HTTP API server");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;

    Ok(())
}
