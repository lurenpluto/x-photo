use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: String,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            code: 0,
            message: "ok".to_string(),
            data,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSourceRequest {
    pub name: String,
    pub root_path: String,
    pub source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PhotoSearchRequest {
    pub keyword: Option<String>,
    pub album_id: Option<String>,
    pub source_id: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub order: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PagedData<T> {
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub items: Vec<T>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAlbumRequest {
    pub name: String,
    pub remark: Option<String>,
    pub auto_created: Option<bool>,
    pub album_date: Option<String>,
    pub rule_key: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ScanTriggerResponse {
    pub job_id: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePhotoRemarkRequest {
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchDeletePhotosRequest {
    pub photo_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct BatchAddToAlbumRequest {
    pub album_id: String,
    pub photo_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct BatchOperationResult {
    pub affected: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAlbumRequest {
    pub name: Option<String>,
    pub remark: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetAlbumCoverRequest {
    pub cover_photo_id: String,
}

#[derive(Debug, Deserialize)]
pub struct AlbumPhotosRequest {
    pub photo_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PaginationQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AlbumSimple {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize)]
pub struct PhotoDetailData {
    pub photo: crate::domain::models::Photo,
    pub albums: Vec<AlbumSimple>,
}

#[derive(Debug, Serialize)]
pub struct AlbumDetailData {
    pub album: crate::domain::models::Album,
    pub photos: PagedData<crate::domain::models::Photo>,
}

#[derive(Debug, Deserialize)]
pub struct FsWatchScanTriggerRequest {
    pub changed_paths: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct TaskJobsQuery {
    pub job_type: Option<String>,
    pub status: Option<String>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TaskJobData {
    pub id: String,
    pub job_type: String,
    pub trigger_type: String,
    pub status: String,
    pub is_daemon: bool,
    pub heartbeat_at: Option<String>,
    pub scan_job_id: Option<String>,
    pub payload_json: Option<String>,
    pub checkpoint_json: Option<String>,
    pub progress_done: i64,
    pub progress_total: Option<i64>,
    pub retry_count: i64,
    pub max_retries: i64,
    pub error_message: Option<String>,
    pub run_after: Option<String>,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct ActiveTaskQuery {
    pub include_all_daemon: Option<bool>,
}
