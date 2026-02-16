use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{StorageAdapter, StorageEntry, StorageError};

pub struct LocalFsAdapter {
    allow_delete: bool,
}

impl LocalFsAdapter {
    pub fn new(allow_delete: bool) -> Self {
        Self { allow_delete }
    }

    fn normalize_path(&self, path: &str) -> Result<PathBuf, StorageError> {
        let raw = path.trim();
        if raw.is_empty() {
            return Err(StorageError::InvalidPath(path.to_string()));
        }

        if raw == "~" {
            if let Ok(home) = std::env::var("HOME") {
                return Ok(PathBuf::from(home));
            }
        }

        if let Some(stripped) = raw.strip_prefix("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return Ok(PathBuf::from(home).join(stripped));
            }
        }

        Ok(PathBuf::from(raw))
    }

    fn to_entry(path: PathBuf, metadata: fs::Metadata) -> StorageEntry {
        StorageEntry {
            path,
            is_dir: metadata.is_dir(),
            size: metadata.len(),
            created_at: metadata.created().ok(),
            modified_at: metadata.modified().ok(),
        }
    }

    fn walk_recursive(root: &Path, out: &mut Vec<StorageEntry>) -> Result<(), StorageError> {
        let entries = fs::read_dir(root)?;
        for item in entries {
            let item = item?;
            let path = item.path();
            let metadata = item.metadata()?;

            let entry = Self::to_entry(path.clone(), metadata.clone());
            out.push(entry);

            if metadata.is_dir() {
                Self::walk_recursive(&path, out)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_path_should_expand_tilde_prefix() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let adapter = LocalFsAdapter::new(false);
        let path = adapter
            .normalize_path("~/.xphoto/tests")
            .expect("normalize path should succeed");
        assert!(path.starts_with(home));
        assert!(path.to_string_lossy().contains(".xphoto/tests"));
    }
}

impl StorageAdapter for LocalFsAdapter {
    fn list_entries(&self, root: &str) -> Result<Vec<StorageEntry>, StorageError> {
        let root = self.normalize_path(root)?;
        let metadata = fs::metadata(&root)?;
        if !metadata.is_dir() {
            return Err(StorageError::InvalidPath(root.display().to_string()));
        }

        let mut out = Vec::new();
        Self::walk_recursive(&root, &mut out)?;
        Ok(out)
    }

    fn stat(&self, path: &str) -> Result<StorageEntry, StorageError> {
        let path = self.normalize_path(path)?;
        let metadata = fs::metadata(&path)?;
        Ok(Self::to_entry(path, metadata))
    }

    fn open_read(&self, path: &str) -> Result<Box<dyn Read + Send>, StorageError> {
        let path = self.normalize_path(path)?;
        let file = File::open(path)?;
        Ok(Box::new(file))
    }

    fn remove_file(&self, path: &str) -> Result<(), StorageError> {
        if !self.allow_delete {
            return Err(StorageError::DeleteDisabled);
        }
        let path = self.normalize_path(path)?;
        fs::remove_file(path)?;
        Ok(())
    }

    fn canonical_id(&self, path: &str) -> Result<String, StorageError> {
        let path = self.normalize_path(path)?;
        let canonical = fs::canonicalize(path)?;
        Ok(canonical.to_string_lossy().to_string())
    }

    fn sha256(&self, path: &str) -> Result<String, StorageError> {
        let path = self.normalize_path(path)?;
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buf = [0_u8; 8 * 1024];

        loop {
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }

        Ok(hex::encode(hasher.finalize()))
    }
}
