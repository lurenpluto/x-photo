use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Source {
    pub id: String,
    pub name: String,
    pub root_path: String,
    pub source_type: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Photo {
    pub id: String,
    pub source_id: String,
    pub storage_file_id: String,
    pub file_path: String,
    pub file_name: String,
    pub file_ext: Option<String>,
    pub file_size: i64,
    pub mime_type: Option<String>,
    pub content_hash: Option<String>,
    pub shot_at: Option<String>,
    pub created_at_fs: Option<String>,
    pub modified_at_fs: Option<String>,
    pub sort_time: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub exif_json: Option<String>,
    pub gps_lat: Option<f64>,
    pub gps_lng: Option<f64>,
    pub remark: Option<String>,
    pub deleted_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Album {
    pub id: String,
    pub name: String,
    pub remark: Option<String>,
    pub cover_photo_id: Option<String>,
    pub auto_created: bool,
    pub album_date: Option<String>,
    pub rule_key: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PhotoAlbum {
    pub photo_id: String,
    pub album_id: String,
    pub seq_no: Option<i32>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScanJob {
    pub id: String,
    pub source_id: String,
    pub status: String,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub total_count: Option<i32>,
    pub new_count: Option<i32>,
    pub updated_count: Option<i32>,
    pub failed_count: Option<i32>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PhotoFeature {
    pub photo_id: String,
    pub version: i32,
    pub features_json: String,
    pub updated_at: String,
}

pub fn build_photo_id(source_id: &str, storage_file_id: &str) -> String {
    let input = format!("{}:{}", source_id, storage_file_id);
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)
}

pub fn build_album_id(
    normalized_name: &str,
    album_date: Option<&str>,
    random_salt: &str,
) -> String {
    let input = format!(
        "{}:{}:{}",
        normalized_name,
        album_date.unwrap_or_default(),
        random_salt
    );
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)
}
