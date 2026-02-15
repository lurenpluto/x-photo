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
