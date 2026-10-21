use std::{collections::HashMap, path::PathBuf, sync::Arc};

use liberte_shared::premium::{check_premium_status_with_key, PremiumToken};
use tokio::sync::RwLock;

use crate::{blob_store::atomic_write, error::ServerError};

/// Explicit operator decisions survive restarts; a revocation overrides even a
/// still-valid issuer token. Tokens themselves are verified on every use.
#[derive(Clone)]
pub struct PremiumVerifier {
    server_pubkey: [u8; 32],
    overrides: Arc<RwLock<HashMap<String, bool>>>,
    state_path: PathBuf,
}

impl PremiumVerifier {
    pub async fn open(server_pubkey: [u8; 32], state_path: PathBuf) -> Result<Self, ServerError> {
        let overrides: HashMap<String, bool> = match tokio::fs::read(&state_path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|e| ServerError::Internal(format!("Invalid entitlement state: {e}")))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(e) => return Err(ServerError::Internal(e.to_string())),
        };
        if overrides.len() > 10_000 || overrides.keys().any(|key| hex::decode(key).map_or(true, |key| key.len() != 32)) {
            return Err(ServerError::Internal("Invalid entitlement state".into()));
        }
        Ok(Self { server_pubkey, overrides: Arc::new(RwLock::new(overrides)), state_path })
    }

    pub async fn verify(&self, token: &PremiumToken) -> bool {
        if self.overrides.read().await.get(&hex::encode(token.user_pubkey)) == Some(&false) {
            return false;
        }
        self.server_pubkey != [0; 32] && check_premium_status_with_key(token, &self.server_pubkey)
    }

    pub async fn authorized(&self, user: &[u8; 32], token: Option<&PremiumToken>) -> bool {
        if let Some(granted) = self.overrides.read().await.get(&hex::encode(user)) {
            return *granted;
        }
        match token {
            Some(token) if &token.user_pubkey == user => self.verify(token).await,
            _ => false,
        }
    }

    pub async fn is_admin_granted(&self, user: &[u8; 32]) -> bool {
        self.overrides.read().await.get(&hex::encode(user)) == Some(&true)
    }

    pub async fn admin_grant(&self, user: &[u8; 32]) -> Result<(), ServerError> {
        self.set_override(user, true).await
    }

    pub async fn admin_revoke(&self, user: &[u8; 32]) -> Result<(), ServerError> {
        self.set_override(user, false).await
    }

    async fn set_override(&self, user: &[u8; 32], granted: bool) -> Result<(), ServerError> {
        let mut overrides = self.overrides.write().await;
        let mut updated = overrides.clone();
        let key = hex::encode(user);
        if updated.len() >= 10_000 && !updated.contains_key(&key) {
            return Err(ServerError::ServiceUnavailable("Entitlement capacity reached".into()));
        }
        updated.insert(key, granted);
        let bytes = serde_json::to_vec(&updated).map_err(|e| ServerError::Internal(e.to_string()))?;
        atomic_write(self.state_path.clone(), bytes).await?;
        *overrides = updated;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Duration, Utc};
    use ed25519_dalek::SigningKey;
    use liberte_shared::premium::create_premium_token;
    use rand::rngs::OsRng;

    #[tokio::test]
    async fn every_token_is_verified_and_bound_to_its_user() {
        let dir = tempfile::tempdir().unwrap();
        let issuer = SigningKey::generate(&mut OsRng);
        let verifier = PremiumVerifier::open(issuer.verifying_key().to_bytes(), dir.path().join("entitlements.json")).await.unwrap();
        let user = [42; 32];
        let token = create_premium_token(&user, Utc::now() + Duration::days(1), &issuer);
        assert!(verifier.authorized(&user, Some(&token)).await);
        assert!(!verifier.authorized(&[43; 32], Some(&token)).await);
        let mut forged = token.clone();
        forged.signature[0] ^= 1;
        assert!(!verifier.verify(&forged).await);
        let expired = create_premium_token(&user, Utc::now() - Duration::days(1), &issuer);
        assert!(!verifier.verify(&expired).await);
    }

    #[tokio::test]
    async fn grants_and_revocations_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("entitlements.json");
        let issuer = SigningKey::generate(&mut OsRng);
        let pubkey = issuer.verifying_key().to_bytes();
        let user = [42; 32];
        let token = create_premium_token(&user, Utc::now() + Duration::days(1), &issuer);
        let verifier = PremiumVerifier::open(pubkey, path.clone()).await.unwrap();
        verifier.admin_grant(&user).await.unwrap();
        let verifier = PremiumVerifier::open(pubkey, path.clone()).await.unwrap();
        assert!(verifier.authorized(&user, None).await);
        verifier.admin_revoke(&user).await.unwrap();
        let verifier = PremiumVerifier::open(pubkey, path).await.unwrap();
        assert!(!verifier.authorized(&user, Some(&token)).await);
    }
}
