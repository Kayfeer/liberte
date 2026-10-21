//! Versioned, authenticated backups. Portable exports use Argon2id; unattended
//! local snapshots use a domain-separated key derived from the local identity.
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD, Engine};
use chacha20poly1305::{aead::{Aead, Payload}, KeyInit, XChaCha20Poly1305, XNonce};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use crate::identity::Identity;

pub const MAX_CONTAINER_BYTES: usize = 96 * 1024 * 1024;
pub const MAX_PLAINTEXT_BYTES: usize = 64 * 1024 * 1024;
const FORMAT: &str = "liberte-encrypted-backup";
const MEMORY_KIB: u32 = 65_536;
const ITERATIONS: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupKind { Backup, Profile }

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
enum KeySource {
    Password { salt: String, memory_kib: u32, iterations: u32, parallelism: u32 },
    Identity { public_key: String },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header { format: String, version: u32, kind: BackupKind, key_source: KeySource }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Container { header: Header, nonce: String, ciphertext: String }

fn password_key(password: &str, salt: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    if password.len() > 1024 { return Err("Password exceeds 1024 bytes".into()); }
    let params = Params::new(MEMORY_KIB, ITERATIONS, 1, Some(32)).map_err(|e| e.to_string())?;
    let mut key = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| format!("Password derivation failed: {e}"))?;
    Ok(key)
}

fn identity_key(identity: &Identity) -> Zeroizing<[u8; 32]> {
    Zeroizing::new(blake3::derive_key("liberte-local-backup-v1", identity.secret_bytes()))
}

fn seal(plaintext: &[u8], header: Header, key: &[u8; 32]) -> Result<String, String> {
    if plaintext.len() > MAX_PLAINTEXT_BYTES { return Err("Backup exceeds the 64 MiB export limit".into()); }
    let aad = serde_json::to_vec(&header).map_err(|e| e.to_string())?;
    let mut nonce = [0u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = XChaCha20Poly1305::new(key.into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: plaintext, aad: &aad })
        .map_err(|_| "Backup encryption failed".to_string())?;
    serde_json::to_string(&Container { header, nonce: STANDARD.encode(nonce), ciphertext: STANDARD.encode(ciphertext) })
        .map_err(|e| e.to_string())
}

pub fn encrypt_password(plaintext: &[u8], kind: BackupKind, password: &str) -> Result<String, String> {
    if password.chars().count() < 12 { return Err("Choose a backup password of at least 12 characters".into()); }
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let key = password_key(password, &salt)?;
    seal(plaintext, Header { format: FORMAT.into(), version: 1, kind, key_source: KeySource::Password {
        salt: STANDARD.encode(salt), memory_kib: MEMORY_KIB, iterations: ITERATIONS, parallelism: 1,
    } }, &key)
}

pub fn encrypt_identity(plaintext: &[u8], kind: BackupKind, identity: &Identity) -> Result<String, String> {
    seal(plaintext, Header { format: FORMAT.into(), version: 1, kind, key_source: KeySource::Identity {
        public_key: hex::encode(identity.public_key_bytes()),
    } }, &identity_key(identity))
}

/// Decrypt or explicitly opt into an old plaintext export. Never fall back from
/// a malformed encrypted envelope to plaintext, even when legacy import is allowed.
pub fn decode_import(input: &str, password: Option<&str>, identity: Option<&Identity>,
    kind: BackupKind, allow_legacy: bool) -> Result<Zeroizing<Vec<u8>>, String> {
    if input.len() > MAX_CONTAINER_BYTES { return Err("Backup file is too large".into()); }
    let value: serde_json::Value = serde_json::from_str(input).map_err(|e| format!("Invalid backup JSON: {e}"))?;
    if !value.is_object() { return Err("Backup must be a JSON object".into()); }
    if value.get("header").is_none() && value.get("ciphertext").is_none() && value.get("nonce").is_none() {
        if !allow_legacy { return Err("This is an unencrypted legacy export containing secret keys. Confirm legacy import explicitly before continuing.".into()); }
        if input.len() > MAX_PLAINTEXT_BYTES { return Err("Legacy backup is too large".into()); }
        return Ok(Zeroizing::new(input.as_bytes().to_vec()));
    }
    let container: Container = serde_json::from_value(value).map_err(|e| format!("Invalid encrypted backup: {e}"))?;
    if container.header.format != FORMAT || container.header.version != 1 { return Err("Unsupported encrypted backup format or version".into()); }
    if container.header.kind != kind { return Err("This export has a different type (profile or backup)".into()); }
    let key = match &container.header.key_source {
        KeySource::Password { salt, memory_kib, iterations, parallelism } => {
            // Fixed, versioned parameters prevent attacker-controlled expensive KDF work.
            if *memory_kib != MEMORY_KIB || *iterations != ITERATIONS || *parallelism != 1 { return Err("Unsupported backup key derivation parameters".into()); }
            let salt = STANDARD.decode(salt).map_err(|_| "Invalid backup salt".to_string())?;
            if salt.len() != 16 { return Err("Invalid backup salt length".into()); }
            password_key(password.ok_or("Enter the password used to export this backup")?, &salt)?
        }
        KeySource::Identity { public_key } => {
            let identity = identity.ok_or("This automatic backup requires the original identity. Restore your portable profile first.")?;
            if *public_key != hex::encode(identity.public_key_bytes()) { return Err("This automatic backup belongs to another identity".into()); }
            identity_key(identity)
        }
    };
    let nonce = STANDARD.decode(&container.nonce).map_err(|_| "Invalid backup nonce".to_string())?;
    if nonce.len() != 24 { return Err("Invalid backup nonce length".into()); }
    let ciphertext = STANDARD.decode(&container.ciphertext).map_err(|_| "Invalid backup ciphertext".to_string())?;
    if ciphertext.len() > MAX_PLAINTEXT_BYTES + 16 { return Err("Backup contents are too large".into()); }
    let aad = serde_json::to_vec(&container.header).map_err(|e| e.to_string())?;
    XChaCha20Poly1305::new((&*key).into())
        .decrypt(XNonce::from_slice(&nonce), Payload { msg: &ciphertext, aad: &aad })
        .map(Zeroizing::new)
        .map_err(|_| "Incorrect password or damaged/tampered backup".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    const PASSWORD: &str = "portable backup password";

    #[test]
    fn password_export_is_portable_and_randomized() {
        let data = br#"{"secret":"private identity and channel keys"}"#;
        let first = encrypt_password(data, BackupKind::Profile, PASSWORD).unwrap();
        let second = encrypt_password(data, BackupKind::Profile, PASSWORD).unwrap();
        assert_ne!(first, second);
        assert!(!first.contains("private identity"));
        let restored = decode_import(&first, Some(PASSWORD), None, BackupKind::Profile, false).unwrap();
        assert_eq!(restored.as_slice(), data);
        assert!(decode_import(&first, Some("wrong password"), None, BackupKind::Profile, false).is_err());
        assert!(decode_import(&first, Some(PASSWORD), None, BackupKind::Backup, false).is_err());
    }

    #[test]
    fn modified_ciphertext_and_header_are_rejected() {
        let id = Identity::generate();
        let export = encrypt_identity(b"secret", BackupKind::Backup, &id).unwrap();
        let mut container: Container = serde_json::from_str(&export).unwrap();
        let mut ciphertext = STANDARD.decode(&container.ciphertext).unwrap();
        ciphertext[0] ^= 1;
        container.ciphertext = STANDARD.encode(ciphertext);
        assert!(decode_import(&serde_json::to_string(&container).unwrap(), None, Some(&id), BackupKind::Backup, true).is_err());
        let mut value: serde_json::Value = serde_json::from_str(&export).unwrap();
        value["header"]["kind"] = "profile".into();
        assert!(decode_import(&value.to_string(), None, Some(&id), BackupKind::Profile, false).is_err());
        assert!(decode_import(&export, None, Some(&Identity::generate()), BackupKind::Backup, false).is_err());
        assert_eq!(decode_import(&export, None, Some(&id), BackupKind::Backup, false).unwrap().as_slice(), b"secret");
    }

    #[test]
    fn legacy_requires_explicit_opt_in() {
        assert!(decode_import("{}", None, None, BackupKind::Backup, false).is_err());
        assert!(decode_import("{}", None, None, BackupKind::Backup, true).is_ok());
        assert!(decode_import(r#"{"header":{}}"#, None, None, BackupKind::Backup, true).is_err());
        assert!(encrypt_password(b"data", BackupKind::Backup, "short").is_err());
    }
}
