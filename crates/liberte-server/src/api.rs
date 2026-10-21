use std::{sync::Arc, time::Instant};

use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, Method},
    middleware,
    response::{IntoResponse, Response},
    routing::{get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{cors::{Any, CorsLayer}, trace::TraceLayer};
use tracing::info;
use uuid::Uuid;

use crate::{
    auth::{authenticate, AuthenticatedUser, ReplayGuard},
    blob_store::BlobStore,
    config::ServerConfig,
    error::ServerError,
    premium::PremiumVerifier,
    rate_limit::{rate_limit_middleware, RateLimiter},
};
use liberte_shared::premium::PremiumToken;

#[derive(Clone)]
pub struct AppState {
    pub blob_store: Arc<BlobStore>,
    pub premium_verifier: Arc<PremiumVerifier>,
    pub replay_guard: Arc<ReplayGuard>,
    pub rate_limiter: RateLimiter,
    pub config: Arc<ServerConfig>,
    pub started_at: Instant,
}

pub fn build_router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers(Any);
    let protected = Router::new()
        .route("/blob/upload", post(blob_upload))
        .route("/blob/:id", get(blob_download).delete(blob_delete))
        .route("/backup/sync", post(backup_sync_upload))
        .route("/backup/:pubkey_hex", get(backup_sync_download))
        .layer(DefaultBodyLimit::max(state.config.max_blob_size.saturating_add(8192)))
        .route_layer(middleware::from_fn_with_state(state.clone(), authenticate));

    Router::new()
        .route("/health", get(health_check))
        .route("/info", get(server_info))
        .route("/premium/verify", post(premium_verify))
        .route("/admin/status", get(admin_status))
        .route("/admin/grant-premium", post(admin_grant_premium))
        .route("/admin/revoke-premium", post(admin_revoke_premium))
        .layer(DefaultBodyLimit::max(16 * 1024))
        .merge(protected)
        .layer(ConcurrencyLimitLayer::new(state.config.max_http_concurrency))
        .layer(middleware::from_fn_with_state(state.rate_limiter.clone(), rate_limit_middleware))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

#[derive(Serialize)]
struct BlobUploadResponse { id: Uuid }

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status": "ok", "version": env!("CARGO_PKG_VERSION")}))
}

async fn server_info(State(state): State<AppState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": state.config.instance_name,
        "version": env!("CARGO_PKG_VERSION"),
        "premium_required": state.config.premium_required,
        "registration_open": state.config.registration_open,
        "max_peers": state.config.max_peers,
        "max_blob_size": state.config.max_blob_size,
        "payment_server_pubkey": (state.config.payment_server_pubkey != [0; 32])
            .then(|| hex::encode(state.config.payment_server_pubkey)),
        "http_auth": "ed25519-v1"
    }))
}

async fn premium_verify(State(state): State<AppState>, Json(token): Json<PremiumToken>) -> Json<serde_json::Value> {
    Json(serde_json::json!({"valid": state.premium_verifier.verify(&token).await}))
}

async fn blob_upload(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    data: Bytes,
) -> Result<Json<BlobUploadResponse>, ServerError> {
    let id = state.blob_store.store_blob(&owner, &data).await?;
    info!(id = %id, size = data.len(), "Encrypted blob stored");
    Ok(Json(BlobUploadResponse { id }))
}

async fn blob_download(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> Result<Response, ServerError> {
    let data = state.blob_store.get_blob(&owner, id).await?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], data).into_response())
}

async fn blob_delete(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, ServerError> {
    state.blob_store.delete_blob(&owner, id).await?;
    Ok(Json(serde_json::json!({"deleted": true})))
}

#[derive(Deserialize)]
struct BackupSyncRequest {
    user_pubkey_hex: String,
    encrypted_data: String,
}

async fn backup_sync_upload(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    Json(req): Json<BackupSyncRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    if parse_hex_32(&req.user_pubkey_hex)? != owner {
        return Err(ServerError::Forbidden("Backup belongs to another identity".into()));
    }
    let data = req.encrypted_data.as_bytes();
    state.blob_store.store_backup(&owner, data).await?;
    Ok(Json(serde_json::json!({"stored": true, "size_bytes": data.len()})))
}

async fn backup_sync_download(
    State(state): State<AppState>,
    Extension(AuthenticatedUser(owner)): Extension<AuthenticatedUser>,
    Path(pubkey_hex): Path<String>,
) -> Result<Response, ServerError> {
    if parse_hex_32(&pubkey_hex)? != owner {
        return Err(ServerError::Forbidden("Backup belongs to another identity".into()));
    }
    let data = state.blob_store.get_backup(&owner).await?;
    Ok(([(header::CONTENT_TYPE, "application/octet-stream"), (header::CACHE_CONTROL, "no-store")], data).into_response())
}

fn verify_admin_token(headers: &HeaderMap, config: &ServerConfig) -> Result<(), ServerError> {
    use subtle::ConstantTimeEq;
    let expected = config.admin_token.as_ref()
        .ok_or_else(|| ServerError::Forbidden("Admin API is disabled".into()))?;
    let auth = headers.get("authorization").and_then(|v| v.to_str().ok()).unwrap_or("");
    let token = auth.strip_prefix("Bearer ").unwrap_or("");
    if token.len() != expected.len() || token.as_bytes().ct_eq(expected.as_bytes()).unwrap_u8() != 1 {
        return Err(ServerError::Forbidden("Invalid admin token".into()));
    }
    Ok(())
}

async fn admin_status(headers: HeaderMap, State(state): State<AppState>) -> Result<Json<serde_json::Value>, ServerError> {
    verify_admin_token(&headers, &state.config)?;
    Ok(Json(serde_json::json!({
        "name": state.config.instance_name,
        "premium_required": state.config.premium_required,
        "registration_open": state.config.registration_open,
        "max_peers": state.config.max_peers,
        "uptime_secs": state.started_at.elapsed().as_secs()
    })))
}

#[derive(Deserialize)]
struct AdminPremiumRequest { user_pubkey_hex: String }

async fn admin_grant_premium(
    headers: HeaderMap, State(state): State<AppState>, Json(req): Json<AdminPremiumRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    verify_admin_token(&headers, &state.config)?;
    state.premium_verifier.admin_grant(&parse_hex_32(&req.user_pubkey_hex)?).await?;
    Ok(Json(serde_json::json!({"granted": true})))
}

async fn admin_revoke_premium(
    headers: HeaderMap, State(state): State<AppState>, Json(req): Json<AdminPremiumRequest>,
) -> Result<Json<serde_json::Value>, ServerError> {
    verify_admin_token(&headers, &state.config)?;
    state.premium_verifier.admin_revoke(&parse_hex_32(&req.user_pubkey_hex)?).await?;
    Ok(Json(serde_json::json!({"revoked": true})))
}

fn parse_hex_32(value: &str) -> Result<[u8; 32], ServerError> {
    let bytes = hex::decode(value).map_err(|_| ServerError::BadRequest("Invalid public key hex".into()))?;
    bytes.try_into().map_err(|_| ServerError::BadRequest("Expected a 32-byte public key".into()))
}

pub async fn serve(state: AppState, addr: std::net::SocketAddr) -> anyhow::Result<()> {
    let app = build_router(state);
    info!(addr = %addr, "Starting HTTP API server");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app.into_make_service_with_connect_info::<std::net::SocketAddr>()).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::{to_bytes, Body}, http::{Request, StatusCode}};
    use liberte_shared::{http_auth::SignedRequest, identity::Identity, premium::create_premium_token};
    use tower::ServiceExt;

    async fn fixture(premium_required: bool) -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let config = ServerConfig {
            blob_storage_path: dir.path().to_path_buf(),
            premium_required,
            max_blob_size: 1024,
            max_user_bytes: 4096,
            max_total_bytes: 8192,
            ..ServerConfig::default()
        };
        let state = AppState {
            blob_store: Arc::new(BlobStore::new(config.blob_storage_path.clone(), config.max_blob_size, config.max_user_bytes, config.max_total_bytes, config.max_objects).await.unwrap()),
            premium_verifier: Arc::new(PremiumVerifier::open([0; 32], dir.path().join("entitlements.json")).await.unwrap()),
            replay_guard: Arc::new(ReplayGuard::open(dir.path().join("replay.json"), 100).await.unwrap()),
            rate_limiter: RateLimiter::default(),
            config: Arc::new(config),
            started_at: Instant::now(),
        };
        (dir, state)
    }

    fn signed_request(id: &Identity, method: &str, path: &str, body: &[u8]) -> Request<Body> {
        let signature = SignedRequest::new(id, method, path, body).unwrap();
        request_with_signature(method, path, body, &signature)
    }

    fn request_with_signature(method: &str, path: &str, body: &[u8], signature: &SignedRequest) -> Request<Body> {
        let mut builder = Request::builder().method(method).uri(path).header("content-type", "application/json");
        for (name, value) in signature.headers() { builder = builder.header(name, value); }
        builder.body(Body::from(body.to_vec())).unwrap()
    }

    async fn bytes(response: Response) -> Vec<u8> {
        to_bytes(response.into_body(), 8192).await.unwrap().to_vec()
    }

    #[tokio::test]
    async fn signed_blob_api_rejects_missing_tampered_and_replayed_requests() {
        let (_dir, state) = fixture(false).await;
        let app = build_router(state);
        let owner = Identity::generate();
        let path = "/blob/upload";
        let response = app.clone().oneshot(Request::builder().method("POST").uri(path).body(Body::from("blob")).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let signature = SignedRequest::new(&owner, "POST", path, b"ciphertext").unwrap();
        let response = app.clone().oneshot(request_with_signature("POST", path, b"tampered", &signature)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let response = app.clone().oneshot(request_with_signature("POST", path, b"ciphertext", &signature)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app.oneshot(request_with_signature("POST", path, b"ciphertext", &signature)).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn owner_can_download_delete_and_other_identity_cannot() {
        let (_dir, state) = fixture(false).await;
        let app = build_router(state);
        let owner = Identity::generate();
        let other = Identity::generate();
        let response = app.clone().oneshot(signed_request(&owner, "POST", "/blob/upload", b"ciphertext")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response: serde_json::Value = serde_json::from_slice(&bytes(response).await).unwrap();
        let path = format!("/blob/{}", response["id"].as_str().unwrap());
        for method in ["GET", "DELETE"] {
            let response = app.clone().oneshot(signed_request(&other, method, &path, b"")).await.unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
        let response = app.clone().oneshot(signed_request(&owner, "GET", &path, b"")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(bytes(response).await, b"ciphertext");
        let response = app.clone().oneshot(signed_request(&owner, "DELETE", &path, b"")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app.oneshot(signed_request(&owner, "GET", &path, b"")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn backup_api_enforces_owner_and_stores_exact_ciphertext() {
        let (_dir, state) = fixture(false).await;
        let app = build_router(state);
        let owner = Identity::generate();
        let other = Identity::generate();
        let body = serde_json::to_vec(&serde_json::json!({"user_pubkey_hex": hex::encode(owner.public_key_bytes()), "encrypted_data": "encrypted-backup"})).unwrap();
        let response = app.clone().oneshot(signed_request(&other, "POST", "/backup/sync", &body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = app.clone().oneshot(signed_request(&owner, "POST", "/backup/sync", &body)).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let path = format!("/backup/{}", hex::encode(owner.public_key_bytes()));
        let response = app.clone().oneshot(signed_request(&other, "GET", &path, b"")).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let response = app.oneshot(signed_request(&owner, "GET", &path, b"")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(bytes(response).await, b"encrypted-backup");
    }

    #[tokio::test]
    async fn premium_is_enforced_and_revocation_blocks_writes() {
        let (_dir, state) = fixture(true).await;
        let owner = Identity::generate();
        let app = build_router(state.clone());
        let response = app.clone().oneshot(signed_request(&owner, "POST", "/blob/upload", b"ciphertext")).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        state.premium_verifier.admin_grant(&owner.public_key_bytes()).await.unwrap();
        let response = app.clone().oneshot(signed_request(&owner, "POST", "/blob/upload", b"ciphertext")).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        state.premium_verifier.admin_revoke(&owner.public_key_bytes()).await.unwrap();
        let response = app.oneshot(signed_request(&owner, "POST", "/blob/upload", b"ciphertext")).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn premium_token_cannot_be_used_by_another_identity() {
        use chrono::{Duration, Utc};
        use ed25519_dalek::SigningKey;
        let (dir, mut state) = fixture(true).await;
        let issuer = SigningKey::generate(&mut rand::rngs::OsRng);
        state.premium_verifier = Arc::new(PremiumVerifier::open(issuer.verifying_key().to_bytes(), dir.path().join("issued-entitlements.json")).await.unwrap());
        let owner = Identity::generate();
        let other = Identity::generate();
        let token = create_premium_token(&owner.public_key_bytes(), Utc::now() + Duration::days(1), &issuer);
        let token_header = serde_json::to_string(&token).unwrap();
        let app = build_router(state);
        for (identity, expected) in [(&other, StatusCode::FORBIDDEN), (&owner, StatusCode::OK)] {
            let mut request = signed_request(identity, "POST", "/blob/upload", b"ciphertext");
            request.headers_mut().insert("x-liberte-premium", token_header.parse().unwrap());
            let response = app.clone().oneshot(request).await.unwrap();
            assert_eq!(response.status(), expected);
        }
    }

    #[tokio::test]
    async fn quota_and_closed_registration_reject_new_writes() {
        let (_dir, mut state) = fixture(false).await;
        Arc::make_mut(&mut state.config).registration_open = false;
        let owner = Identity::generate();
        let app = build_router(state.clone());
        let response = app.clone().oneshot(signed_request(&owner, "POST", "/blob/upload", b"ciphertext")).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        state.premium_verifier.admin_grant(&owner.public_key_bytes()).await.unwrap();
        let response = app.oneshot(signed_request(&owner, "POST", "/blob/upload", &vec![1; 1025])).await.unwrap();
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }
}
