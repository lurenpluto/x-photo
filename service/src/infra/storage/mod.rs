use std::io::Read;
use std::path::PathBuf;
use std::time::SystemTime;

pub mod local_fs;

#[derive(Debug, Clone)]
pub struct StorageEntry {
    pub path: PathBuf,
    pub is_dir: bool,
    pub size: u64,
    pub created_at: Option<SystemTime>,
    pub modified_at: Option<SystemTime>,
}

#[derive(Debug)]
pub enum StorageError {
    Io(std::io::Error),
    InvalidPath(String),
    DeleteDisabled,
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "io error: {}", err),
            Self::InvalidPath(path) => write!(f, "invalid path: {}", path),
            Self::DeleteDisabled => write!(f, "delete operation disabled"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<std::io::Error> for StorageError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

pub trait StorageAdapter: Send + Sync {
    fn list_entries(&self, root: &str) -> Result<Vec<StorageEntry>, StorageError>;
    fn stat(&self, path: &str) -> Result<StorageEntry, StorageError>;
    fn open_read(&self, path: &str) -> Result<Box<dyn Read + Send>, StorageError>;
    fn remove_file(&self, path: &str) -> Result<(), StorageError>;
    fn canonical_id(&self, path: &str) -> Result<String, StorageError>;
    fn sha256(&self, path: &str) -> Result<String, StorageError>;
}
