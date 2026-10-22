//! Signed HTTP requests. Authentication does not replace HTTPS transport security.
use chrono::Utc;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::identity::Identity;

pub const MAX_CLOCK_SKEW_SECONDS: i64 = 120;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignedRequest {
    pub public_key: String,
    pub timestamp: i64,
    pub nonce: String,
    pub signature: String,
}

impl SignedRequest {
    pub fn new(identity: &Identity, method: &str, path: &str, body: &[u8]) -> Result<Self, String> {
        let mut request = Self {
            public_key: hex::encode(identity.public_key_bytes()),
            timestamp: Utc::now().timestamp(),
            nonce: Uuid::new_v4().to_string(),
            signature: String::new(),
        };
        request.signature = hex::encode(
            identity
                .sign(&request.signing_bytes(method, path, body)?)
                .to_bytes(),
        );
        Ok(request)
    }

    pub fn headers(&self) -> [(&'static str, String); 4] {
        [
            ("x-liberte-key", self.public_key.clone()),
            ("x-liberte-timestamp", self.timestamp.to_string()),
            ("x-liberte-nonce", self.nonce.clone()),
            ("x-liberte-signature", self.signature.clone()),
        ]
    }

    /// Verify cryptographic identity. The server must also consume this nonce once.
    pub fn verify(
        &self,
        method: &str,
        path: &str,
        body: &[u8],
        now: i64,
    ) -> Result<[u8; 32], String> {
        if self.public_key.len() != 64 || self.signature.len() != 128 || self.nonce.len() != 36 {
            return Err("Invalid signed request field lengths".into());
        }
        if self.timestamp.abs_diff(now) > MAX_CLOCK_SKEW_SECONDS as u64 {
            return Err("Request timestamp is outside the allowed clock window".into());
        }
        let key_bytes: [u8; 32] = hex::decode(&self.public_key)
            .map_err(|_| "Invalid public key")?
            .try_into()
            .map_err(|_| "Invalid public key length")?;
        let signature_bytes: [u8; 64] = hex::decode(&self.signature)
            .map_err(|_| "Invalid signature")?
            .try_into()
            .map_err(|_| "Invalid signature length")?;
        VerifyingKey::from_bytes(&key_bytes)
            .map_err(|_| "Invalid Ed25519 key")?
            .verify_strict(
                &self.signing_bytes(method, path, body)?,
                &Signature::from_bytes(&signature_bytes),
            )
            .map_err(|_| "Invalid request signature")?;
        Ok(key_bytes)
    }

    fn signing_bytes(&self, method: &str, path: &str, body: &[u8]) -> Result<Vec<u8>, String> {
        if !matches!(method, "GET" | "POST" | "DELETE" | "PUT")
            || !path.starts_with('/')
            || path.len() > 2048
            || path.chars().any(char::is_control)
            || Uuid::parse_str(&self.nonce).is_err()
            || self.nonce.len() != 36
            || self.public_key.len() != 64
        {
            return Err("Invalid signed request fields".into());
        }
        // JSON tuple gives unambiguous framing; the domain prevents cross-protocol reuse.
        serde_json::to_vec(&(
            "liberte-http-auth-v1",
            &self.public_key,
            self.timestamp,
            &self.nonce,
            method,
            path,
            blake3::hash(body).to_hex().as_str(),
        ))
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_signature_binds_identity_method_path_and_body() {
        let identity = Identity::generate();
        let request = SignedRequest::new(&identity, "POST", "/backup/sync", b"backup").unwrap();
        let now = Utc::now().timestamp();
        assert_eq!(
            request
                .verify("POST", "/backup/sync", b"backup", now)
                .unwrap(),
            identity.public_key_bytes()
        );
        assert!(request
            .verify("POST", "/backup/sync", b"changed", now)
            .is_err());
        assert!(request
            .verify("POST", "/blob/upload", b"backup", now)
            .is_err());
        assert!(request
            .verify("DELETE", "/backup/sync", b"backup", now)
            .is_err());
        let mut changed = request.clone();
        changed.public_key = hex::encode(Identity::generate().public_key_bytes());
        assert!(changed
            .verify("POST", "/backup/sync", b"backup", now)
            .is_err());
        assert!(request
            .verify("POST", "/backup/sync", b"backup", now + 121)
            .is_err());
    }

    #[test]
    fn malformed_fields_and_extreme_timestamps_are_rejected_without_panicking() {
        let identity = Identity::generate();
        let valid = SignedRequest::new(&identity, "GET", "/backup/test", b"").unwrap();
        let mut invalid = valid.clone();
        invalid.timestamp = i64::MIN;
        assert!(invalid
            .verify("GET", "/backup/test", b"", i64::MAX)
            .is_err());
        invalid = valid.clone();
        invalid.signature = "00".repeat(64);
        assert!(invalid
            .verify("GET", "/backup/test", b"", Utc::now().timestamp())
            .is_err());
        invalid = valid;
        invalid.public_key = "z".repeat(64);
        assert!(invalid
            .verify("GET", "/backup/test", b"", Utc::now().timestamp())
            .is_err());
        assert!(SignedRequest::new(&identity, "GET", "/invalid\npath", b"").is_err());
    }
}
