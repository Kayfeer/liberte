//! Premium status verification and caching.
//!
//! Verifies `PremiumToken` tokens issued by the payment server using
//! ed25519-dalek, and caches successful verifications to avoid
//! re-checking signatures on every request.

use std::collections::HashMap;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use tokio::sync::RwLock;
use tracing::{debug, info};

use liberte_shared::premium::{check_premium_status_with_key, PremiumToken};

// ---------------------------------------------------------------------------
// Cached entry
// ---------------------------------------------------------------------------

/// A cached premium verification result.
#[derive(Debug, Clone)]
struct CachedStatus {
    /// Whether the token was valid at verification time.
    valid: bool,
    /// When the premium subscription expires (from the token).
    valid_until: DateTime<Utc>,
    /// When we cached this result.
    verified_at: DateTime<Utc>,
}

impl CachedStatus {
    /// Returns `true` if the cached entry is still usable.
    ///
    /// An entry is stale when the subscription has expired since we
    /// last checked.
    fn is_fresh(&self) -> bool {
        self.valid && Utc::now() < self.valid_until
    }
}

// ---------------------------------------------------------------------------
// Premium verifier
// ---------------------------------------------------------------------------

/// Verifies and caches premium status for user public keys.
#[derive(Clone)]
pub struct PremiumVerifier {
    /// The payment server's Ed25519 public key.
    server_pubkey: [u8; 32],
    /// Cache: user_pubkey -> last verified status.
    cache: Arc<RwLock<HashMap<[u8; 32], CachedStatus>>>,
}

impl PremiumVerifier {
    /// Create a new verifier with the given payment server public key.
    pub fn new(server_pubkey: [u8; 32]) -> Self {
        Self {
            server_pubkey,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Verify a premium token.
    ///
    /// Returns `true` if the token is valid and not expired.
    /// Successful verifications are cached so that subsequent calls
    /// for the same user skip the cryptographic check.
    pub async fn verify(&self, token: &PremiumToken) -> bool {
        // 1. Check the cache first.
        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(&token.user_pubkey) {
                if entry.is_fresh() {
                    debug!(
                        user = hex::encode(token.user_pubkey),
                        "Premium status served from cache"
                    );
                    return true;
                }
            }
        }

        // 2. Perform full cryptographic verification.
        let valid = check_premium_status_with_key(token, &self.server_pubkey);

        // 3. Update the cache.
        {
            let mut cache = self.cache.write().await;
            cache.insert(
                token.user_pubkey,
                CachedStatus {
                    valid,
                    valid_until: token.valid_until,
                    verified_at: Utc::now(),
                },
            );
        }

        if valid {
            info!(
                user = hex::encode(token.user_pubkey),
                until = %token.valid_until,
                "Premium status verified"
            );
        } else {
            debug!(
                user = hex::encode(token.user_pubkey),
                "Premium verification failed"
            );
        }

        valid
    }

    /// Check whether a user has a cached, still-valid premium status
    /// without presenting a new token.
    pub async fn is_premium_cached(&self, user_pubkey: &[u8; 32]) -> bool {
        let cache = self.cache.read().await;
        cache
            .get(user_pubkey)
            .map(|entry| entry.is_fresh())
            .unwrap_or(false)
    }

    /// Manually grant premium to a user (admin action for self-hosted).
    ///
    /// Inserts a cache entry valid for 100 years (effectively permanent
    /// until revoked).
    pub async fn admin_grant(&self, user_pubkey: &[u8; 32]) {
        let mut cache = self.cache.write().await;
        cache.insert(
            *user_pubkey,
            CachedStatus {
                valid: true,
                valid_until: Utc::now() + Duration::days(36500),
                verified_at: Utc::now(),
            },
        );
    }

    /// Manually revoke premium from a user (admin action for self-hosted).
    pub async fn admin_revoke(&self, user_pubkey: &[u8; 32]) {
        let mut cache = self.cache.write().await;
        cache.remove(user_pubkey);
    }

    /// Evict expired entries from the cache.
    pub async fn purge_expired(&self) {
        let mut cache = self.cache.write().await;
        let before = cache.len();
        cache.retain(|_, entry| entry.is_fresh());
        let removed = before - cache.len();
        if removed > 0 {
            debug!(removed, "Purged expired premium cache entries");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;
    use ed25519_dalek::SigningKey;
    use liberte_shared::premium::create_premium_token;
    use rand::rngs::OsRng;

    #[tokio::test]
    async fn test_verify_valid_token() {
        let server_key = SigningKey::generate(&mut OsRng);
        let server_pubkey = server_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() + Duration::days(30),
            &server_key,
        );

        let verifier = PremiumVerifier::new(server_pubkey);
        assert!(verifier.verify(&token).await);
        // Second call should be served from cache.
        assert!(verifier.is_premium_cached(&user_pubkey).await);
    }

    #[tokio::test]
    async fn test_verify_expired_token() {
        let server_key = SigningKey::generate(&mut OsRng);
        let server_pubkey = server_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() - Duration::days(1),
            &server_key,
        );

        let verifier = PremiumVerifier::new(server_pubkey);
        assert!(!verifier.verify(&token).await);
    }

    #[tokio::test]
    async fn test_verify_wrong_key() {
        let server_key = SigningKey::generate(&mut OsRng);
        let wrong_key = SigningKey::generate(&mut OsRng);
        let wrong_pubkey = wrong_key.verifying_key().to_bytes();
        let user_pubkey = [42u8; 32];

        let token = create_premium_token(
            &user_pubkey,
            Utc::now() + Duration::days(30),
            &server_key,
        );

        let verifier = PremiumVerifier::new(wrong_pubkey);
        assert!(!verifier.verify(&token).await);
    }
}
