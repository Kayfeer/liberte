use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::fs;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::ServerError;

/// Object directories are named after their authenticated owner. Legacy unowned
/// objects are not served until an administrator explicitly migrates ownership.
#[derive(Clone)]
pub struct BlobStore {
    base_path: PathBuf,
    max_size: usize,
    max_user_bytes: u64,
    max_total_bytes: u64,
    max_objects: usize,
    writes: Arc<Mutex<()>>,
}

impl BlobStore {
    pub async fn new(
        base_path: PathBuf,
        max_size: usize,
        max_user_bytes: u64,
        max_total_bytes: u64,
        max_objects: usize,
    ) -> Result<Self, ServerError> {
        fs::create_dir_all(&base_path).await.map_err(storage_error)?;
        let base_path = fs::canonicalize(base_path).await.map_err(storage_error)?;
        let users = base_path.join("users");
        fs::create_dir_all(&users).await.map_err(storage_error)?;
        reject_symlink(&users).await?;
        Ok(Self {
            base_path, max_size, max_user_bytes, max_total_bytes, max_objects,
            writes: Arc::new(Mutex::new(())),
        })
    }

    fn owner_dir(&self, owner: &[u8; 32]) -> PathBuf {
        self.base_path.join("users").join(hex::encode(owner))
    }

    async fn object_path(&self, owner: &[u8; 32], name: &str) -> Result<PathBuf, ServerError> {
        if name.contains(['/', '\\']) || name.contains("..") {
            return Err(ServerError::BadRequest("Invalid object name".into()));
        }
        reject_symlink(&self.base_path.join("users")).await?;
        let dir = self.owner_dir(owner);
        if fs::try_exists(&dir).await.map_err(storage_error)? {
            reject_symlink(&dir).await?;
        }
        let path = dir.join(name);
        if fs::try_exists(&path).await.map_err(storage_error)? {
            reject_symlink(&path).await?;
        }
        Ok(path)
    }

    pub async fn store_blob(&self, owner: &[u8; 32], data: &[u8]) -> Result<Uuid, ServerError> {
        let id = Uuid::new_v4();
        self.write_object(owner, &format!("{id}.enc"), data).await?;
        Ok(id)
    }

    pub async fn get_blob(&self, owner: &[u8; 32], id: Uuid) -> Result<Vec<u8>, ServerError> {
        let path = self.object_path(owner, &format!("{id}.enc")).await?;
        fs::read(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ServerError::BlobNotFound(id),
            _ => storage_error(e),
        })
    }

    pub async fn delete_blob(&self, owner: &[u8; 32], id: Uuid) -> Result<(), ServerError> {
        let _write = self.writes.lock().await;
        let path = self.object_path(owner, &format!("{id}.enc")).await?;
        fs::remove_file(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ServerError::BlobNotFound(id),
            _ => storage_error(e),
        })?;
        // Prevent a churn of short-lived identities leaving unbounded empty directories.
        let _ = fs::remove_dir(self.owner_dir(owner)).await;
        Ok(())
    }

    pub async fn store_backup(&self, owner: &[u8; 32], data: &[u8]) -> Result<(), ServerError> {
        self.write_object(owner, "backup.enc", data).await
    }

    pub async fn get_backup(&self, owner: &[u8; 32]) -> Result<String, ServerError> {
        let path = self.object_path(owner, "backup.enc").await?;
        fs::read_to_string(path).await.map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => ServerError::NotFound("No backup found".into()),
            _ => storage_error(e),
        })
    }

    async fn write_object(&self, owner: &[u8; 32], name: &str, data: &[u8]) -> Result<(), ServerError> {
        if data.is_empty() {
            return Err(ServerError::BadRequest("Empty object".into()));
        }
        if data.len() > self.max_size {
            return Err(ServerError::BlobTooLarge { size: data.len(), max: self.max_size });
        }
        let _write = self.writes.lock().await;
        let path = self.object_path(owner, name).await?;
        let previous = match fs::metadata(&path).await {
            Ok(m) => m.len(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(storage_error(e)),
        };
        let (total, count, by_owner) = self.usage().await?;
        let user_bytes = by_owner.get(&hex::encode(owner)).copied().unwrap_or_default();
        let extra = data.len() as u64;
        if total.saturating_sub(previous).saturating_add(extra) > self.max_total_bytes
            || user_bytes.saturating_sub(previous).saturating_add(extra) > self.max_user_bytes
            || (previous == 0 && count >= self.max_objects)
        {
            return Err(ServerError::QuotaExceeded);
        }
        fs::create_dir_all(self.owner_dir(owner)).await.map_err(storage_error)?;
        reject_symlink(&self.owner_dir(owner)).await?;
        atomic_write(path, data.to_vec()).await
    }

    async fn usage(&self) -> Result<(u64, usize, HashMap<String, u64>), ServerError> {
        let mut total = 0u64;
        let mut count = 0usize;
        let mut by_owner = HashMap::new();
        let mut dirs = fs::read_dir(self.base_path.join("users")).await.map_err(storage_error)?;
        while let Some(dir) = dirs.next_entry().await.map_err(storage_error)? {
            let kind = dir.file_type().await.map_err(storage_error)?;
            if kind.is_symlink() || !kind.is_dir() {
                return Err(ServerError::BlobStorage("Invalid storage directory".into()));
            }
            let owner = dir.file_name().to_string_lossy().into_owned();
            let mut files = fs::read_dir(dir.path()).await.map_err(storage_error)?;
            let mut user_size = 0u64;
            while let Some(file) = files.next_entry().await.map_err(storage_error)? {
                if file.file_type().await.map_err(storage_error)?.is_symlink() {
                    return Err(ServerError::BlobStorage("Symlinks are not allowed".into()));
                }
                let metadata = file.metadata().await.map_err(storage_error)?;
                if metadata.is_file() {
                    count += 1;
                    user_size = user_size.saturating_add(metadata.len());
                }
            }
            total = total.saturating_add(user_size);
            by_owner.insert(owner, user_size);
        }
        Ok((total, count, by_owner))
    }
}

async fn reject_symlink(path: &Path) -> Result<(), ServerError> {
    if fs::symlink_metadata(path).await.map_err(storage_error)?.file_type().is_symlink() {
        return Err(ServerError::BlobStorage("Symlinks are not allowed".into()));
    }
    Ok(())
}

/// Atomic replacement on Windows and Unix; a failed write preserves the old file.
pub async fn atomic_write(path: PathBuf, bytes: Vec<u8>) -> Result<(), ServerError> {
    tokio::task::spawn_blocking(move || {
        let parent = path.parent().ok_or_else(|| ServerError::Internal("No parent directory".into()))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent).map_err(storage_error)?;
        temp.write_all(&bytes).map_err(storage_error)?;
        temp.as_file().sync_all().map_err(storage_error)?;
        temp.persist(path).map_err(|e| storage_error(e.error))?;
        Ok(())
    })
    .await
    .map_err(|e| ServerError::Internal(e.to_string()))?
}

fn storage_error(error: std::io::Error) -> ServerError {
    ServerError::BlobStorage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn store(path: &Path, quota: u64) -> BlobStore {
        BlobStore::new(path.to_owned(), 1024, quota, 4096, 10).await.unwrap()
    }

    #[tokio::test]
    async fn owner_isolation_survives_restart() {
        let temp = tempfile::tempdir().unwrap();
        let store1 = store(temp.path(), 2048).await;
        let id = store1.store_blob(&[1; 32], b"ciphertext").await.unwrap();
        drop(store1);
        let reopened = store(temp.path(), 2048).await;
        assert_eq!(reopened.get_blob(&[1; 32], id).await.unwrap(), b"ciphertext");
        assert!(reopened.get_blob(&[2; 32], id).await.is_err());
        assert!(reopened.delete_blob(&[2; 32], id).await.is_err());
        assert!(reopened.get_blob(&[1; 32], id).await.is_ok());
    }

    #[tokio::test]
    async fn quota_failure_keeps_previous_backup() {
        let temp = tempfile::tempdir().unwrap();
        let store = store(temp.path(), 5).await;
        store.store_backup(&[1; 32], b"first").await.unwrap();
        assert!(store.store_backup(&[1; 32], b"too long").await.is_err());
        assert_eq!(store.get_backup(&[1; 32]).await.unwrap(), "first");
        store.store_backup(&[1; 32], b"next").await.unwrap();
        assert_eq!(store.get_backup(&[1; 32]).await.unwrap(), "next");
    }

    #[tokio::test]
    async fn concurrent_writers_cannot_exceed_global_quota() {
        let temp = tempfile::tempdir().unwrap();
        let store = BlobStore::new(temp.path().to_owned(), 1024, 8, 8, 10).await.unwrap();
        let (first, second) = tokio::join!(
            store.store_blob(&[1; 32], b"first"),
            store.store_blob(&[2; 32], b"other"),
        );
        assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
        let (bytes, count, _) = store.usage().await.unwrap();
        assert_eq!(bytes, 5);
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn object_limit_is_restored_after_deletion() {
        let temp = tempfile::tempdir().unwrap();
        let store = BlobStore::new(temp.path().to_owned(), 1024, 100, 100, 1).await.unwrap();
        let id = store.store_blob(&[1; 32], b"first").await.unwrap();
        assert!(matches!(store.store_blob(&[2; 32], b"other").await, Err(ServerError::QuotaExceeded)));
        store.delete_blob(&[1; 32], id).await.unwrap();
        assert!(!store.owner_dir(&[1; 32]).exists());
        assert!(store.store_blob(&[2; 32], b"other").await.is_ok());
    }
}
