use std::{collections::HashMap, path::PathBuf};

use axum::{
    body::{to_bytes, Body},
    extract::{Request, State},
    http::HeaderMap,
    middleware::Next,
    response::Response,
};
use chrono::Utc;
use liberte_shared::{
    http_auth::{SignedRequest, MAX_CLOCK_SKEW_SECONDS},
    premium::PremiumToken,
};
use tokio::sync::Mutex;

use crate::{api::AppState, blob_store::atomic_write, error::ServerError};

#[derive(Clone, Copy)]
pub struct AuthenticatedUser(pub [u8; 32]);

/// Shared by every route and persisted before accepting a request, so a restart
/// within the signature validity window cannot make captured requests reusable.
pub struct ReplayGuard {
    path: PathBuf,
    nonces: Mutex<HashMap<String, i64>>,
    capacity: usize,
}

impl ReplayGuard {
    pub async fn open(path: PathBuf, capacity: usize) -> Result<Self, ServerError> {
        let mut nonces: HashMap<String, i64> = match tokio::fs::read(&path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| ServerError::Internal(format!("Invalid replay state: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(ServerError::Internal(e.to_string())),
        };
        nonces.retain(|_, expiry| *expiry >= Utc::now().timestamp());
        if nonces.len() > capacity {
            return Err(ServerError::Internal("Replay state exceeds configured capacity".into()));
        }
        Ok(Self { path, nonces: Mutex::new(nonces), capacity })
    }

    async fn consume(&self, request: &SignedRequest, now: i64) -> Result<(), ServerError> {
        let mut nonces = self.nonces.lock().await;
        let mut updated = nonces.clone();
        updated.retain(|_, expiry| *expiry >= now);
        let key = format!("{}:{}", request.public_key.to_ascii_lowercase(), request.nonce);
        if updated.contains_key(&key) {
            return Err(ServerError::Unauthorized("Request already used".into()));
        }
        if updated.len() >= self.capacity {
            return Err(ServerError::ServiceUnavailable("Authentication capacity reached".into()));
        }
        updated.insert(key, request.timestamp.saturating_add(MAX_CLOCK_SKEW_SECONDS));
        let bytes = serde_json::to_vec(&updated).map_err(|e| ServerError::Internal(e.to_string()))?;
        atomic_write(self.path.clone(), bytes).await?;
        *nonces = updated;
        Ok(())
    }
}

fn header(headers: &HeaderMap, name: &'static str) -> Result<String, ServerError> {
    if headers.get_all(name).iter().count() != 1 {
        return Err(ServerError::Unauthorized(format!("Missing or repeated {name}")));
    }
    headers.get(name)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 256)
        .map(str::to_owned)
        .ok_or_else(|| ServerError::Unauthorized(format!("Invalid {name}")))
}

pub async fn authenticate(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response, ServerError> {
    let signed = SignedRequest {
        public_key: header(request.headers(), "x-liberte-key")?,
        timestamp: header(request.headers(), "x-liberte-timestamp")?
            .parse().map_err(|_| ServerError::Unauthorized("Invalid timestamp".into()))?,
        nonce: header(request.headers(), "x-liberte-nonce")?,
        signature: header(request.headers(), "x-liberte-signature")?,
    };
    let (mut parts, body) = request.into_parts();
    let bytes = tokio::time::timeout(std::time::Duration::from_secs(60),
        to_bytes(body, state.config.max_blob_size.saturating_add(8192))).await
        .map_err(|_| ServerError::BadRequest("Request body timed out".into()))?
        .map_err(|_| ServerError::BlobTooLarge { size: state.config.max_blob_size.saturating_add(1), max: state.config.max_blob_size })?;
    let path = parts.uri.path_and_query().map(|path| path.as_str()).unwrap_or("/");
    let now = Utc::now().timestamp();
    let owner = signed.verify(parts.method.as_str(), path, &bytes, now)
        .map_err(ServerError::Unauthorized)?;
    state.replay_guard.consume(&signed, now).await?;

    // Expiry never prevents an owner downloading or deleting their own ciphertext.
    // Closed registration restricts new writes to explicitly admitted identities.
    if parts.method == axum::http::Method::POST {
        if !state.config.registration_open && !state.premium_verifier.is_admin_granted(&owner).await {
            return Err(ServerError::Forbidden("Registration is closed".into()));
        }
        if state.config.premium_required {
            let token: Option<PremiumToken> = match parts.headers.get("x-liberte-premium") {
                Some(value) => {
                    if parts.headers.get_all("x-liberte-premium").iter().count() != 1 || value.len() > 8192 {
                        return Err(ServerError::Forbidden("Invalid premium token".into()));
                    }
                    Some(serde_json::from_slice(value.as_bytes())
                        .map_err(|_| ServerError::Forbidden("Invalid premium token".into()))?)
                }
                None => None,
            };
            if !state.premium_verifier.authorized(&owner, token.as_ref()).await {
                return Err(ServerError::Forbidden("Premium entitlement required".into()));
            }
        }
    }
    parts.extensions.insert(AuthenticatedUser(owner));
    Ok(next.run(Request::from_parts(parts, Body::from(bytes))).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use liberte_shared::identity::Identity;

    #[tokio::test]
    async fn replay_is_rejected_after_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("replay.json");
        let signed = SignedRequest::new(&Identity::generate(), "GET", "/blob/test", b"").unwrap();
        let guard = ReplayGuard::open(path.clone(), 100).await.unwrap();
        guard.consume(&signed, Utc::now().timestamp()).await.unwrap();
        let reopened = ReplayGuard::open(path, 100).await.unwrap();
        assert!(reopened.consume(&signed, Utc::now().timestamp()).await.is_err());
    }

    #[tokio::test]
    async fn replay_cache_fails_closed_when_full() {
        let dir = tempfile::tempdir().unwrap();
        let guard = ReplayGuard::open(dir.path().join("replay.json"), 1).await.unwrap();
        let owner = Identity::generate();
        let first = SignedRequest::new(&owner, "GET", "/blob/first", b"").unwrap();
        let second = SignedRequest::new(&owner, "GET", "/blob/second", b"").unwrap();
        guard.consume(&first, Utc::now().timestamp()).await.unwrap();
        assert!(matches!(guard.consume(&second, Utc::now().timestamp()).await, Err(ServerError::ServiceUnavailable(_))));
    }
}
