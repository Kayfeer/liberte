use std::path::PathBuf;

use tokio::fs;
use tracing::{debug, info};
use uuid::Uuid;

use crate::error::ServerError;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct BlobStore {
    base_path: PathBuf,
    max_size: usize,
}

impl BlobStore {
    pub async fn new(base_path: PathBuf, max_size: usize) -> Result<Self, ServerError> {
        fs::create_dir_all(&base_path).await.map_err(|e| {
            ServerError::BlobStorage(format!(
                "Failed to create blob directory '{}': {}",
                base_path.display(),
                e
            ))
        })?;

        info!(path = %base_path.display(), "Blob store initialized");

        Ok(Self {
            base_path,
            max_size,
        })
    }

    pub async fn store_blob(&self, data: &[u8]) -> Result<Uuid, ServerError> {
        if data.is_empty() {
            return Err(ServerError::BlobStorage("Empty blob".to_string()));
        }
        if data.len() > self.max_size {
            return Err(ServerError::BlobTooLarge {
                size: data.len(),
                max: self.max_size,
            });
        }

        let id = Uuid::new_v4();
        let path = self.blob_path(&id);

        fs::write(&path, data).await.map_err(|e| {
            ServerError::BlobStorage(format!("Failed to write blob {}: {}", id, e))
        })?;

        debug!(id = %id, size = data.len(), "Stored blob");
        Ok(id)
    }

    pub async fn get_blob(&self, id: Uuid) -> Result<Vec<u8>, ServerError> {
        let path = self.blob_path(&id);

        if !path.exists() {
            return Err(ServerError::BlobNotFound(id));
        }

        let data = fs::read(&path).await.map_err(|e| {
            ServerError::BlobStorage(format!("Failed to read blob {}: {}", id, e))
        })?;

        debug!(id = %id, size = data.len(), "Retrieved blob");
        Ok(data)
    }

    pub async fn delete_blob(&self, id: Uuid) -> Result<(), ServerError> {
        let path = self.blob_path(&id);

        if !path.exists() {
            return Err(ServerError::BlobNotFound(id));
        }

        fs::remove_file(&path).await.map_err(|e| {
            ServerError::BlobStorage(format!("Failed to delete blob {}: {}", id, e))
        })?;

        debug!(id = %id, "Deleted blob");
        Ok(())
    }

    #[allow(dead_code)]
    pub async fn list_blobs(&self) -> Result<Vec<Uuid>, ServerError> {
        let mut ids = Vec::new();
        let mut entries = fs::read_dir(&self.base_path).await.map_err(|e| {
            ServerError::BlobStorage(format!("Failed to list blobs: {}", e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            ServerError::BlobStorage(format!("Failed to read directory entry: {}", e))
        })? {
            if let Some(name) = entry.file_name().to_str() {
                if let Ok(id) = Uuid::parse_str(name) {
                    ids.push(id);
                }
            }
        }

        Ok(ids)
    }

    fn blob_path(&self, id: &Uuid) -> PathBuf {
        self.base_path.join(id.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    async fn test_store() -> (BlobStore, TempDir) {
        let dir = TempDir::new().unwrap();
        let store = BlobStore::new(dir.path().to_path_buf(), 1024 * 1024).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn test_store_and_get() {
        let (store, _dir) = test_store().await;
        let data = b"encrypted-blob-data";

        let id = store.store_blob(data).await.unwrap();
        let retrieved = store.get_blob(id).await.unwrap();
        assert_eq!(retrieved, data);
    }

    #[tokio::test]
    async fn test_delete() {
        let (store, _dir) = test_store().await;
        let id = store.store_blob(b"delete-me").await.unwrap();

        store.delete_blob(id).await.unwrap();
        assert!(store.get_blob(id).await.is_err());
    }

    #[tokio::test]
    async fn test_list() {
        let (store, _dir) = test_store().await;

        let id1 = store.store_blob(b"blob-1").await.unwrap();
        let id2 = store.store_blob(b"blob-2").await.unwrap();

        let ids = store.list_blobs().await.unwrap();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));
    }

    #[tokio::test]
    async fn test_not_found() {
        let (store, _dir) = test_store().await;
        let missing = Uuid::new_v4();
        assert!(store.get_blob(missing).await.is_err());
    }

    #[tokio::test]
    async fn test_empty_blob_rejected() {
        let (store, _dir) = test_store().await;
        assert!(store.store_blob(b"").await.is_err());
    }
}
