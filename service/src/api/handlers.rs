use std::collections::HashMap;
use std::path::Path as StdPath;
use std::sync::Arc;

use axum::{extract::Path, extract::Query, extract::State, http::StatusCode, Json};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tracing::{error, info, warn};

use crate::api::types::{
    AlbumDetailData, AlbumPhotosRequest, AlbumSimple, ApiResponse, BatchAddToAlbumRequest,
    BatchDeletePhotosRequest, BatchOperationResult, CreateAlbumRequest, CreateSourceRequest,
    PagedData, PaginationQuery, PhotoDetailData, PhotoSearchRequest, ScanTriggerResponse,
    SetAlbumCoverRequest, UpdateAlbumRequest, UpdatePhotoRemarkRequest,
};
use crate::api::AppState;
use crate::domain::album_rules::{
    patterns_from_delimiters, parse_album_from_dir_name_with_patterns, AlbumRulePattern,
    RegexRulePattern,
};
use crate::domain::models::{build_album_id, build_photo_id, Album, Photo, Source};
use crate::infra::storage::local_fs::LocalFsAdapter;
use crate::infra::storage::StorageAdapter;

pub async fn health() -> Json<ApiResponse<Value>> {
    info!("health check requested");
    Json(ApiResponse::ok(json!({"status": "ok"})))
}

pub async fn list_sources(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Source>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!("list_sources requested");
    let rows = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|err| internal_db_error("list_sources.fetch_all", json!({}), err))?;

    info!(count = rows.len(), "list_sources completed");

    Ok(Json(ApiResponse::ok(rows)))
}

pub async fn create_source(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateSourceRequest>,
) -> Result<Json<ApiResponse<Source>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.name.trim().is_empty() || req.root_path.trim().is_empty() {
        return Err(bad_request("name/root_path 不能为空"));
    }

    info!(
        name = req.name.trim(),
        root_path = req.root_path.trim(),
        source_type = req.source_type.as_deref().unwrap_or("local_fs"),
        "create_source requested"
    );

    let id = sha256_hex(&req.root_path);
    let now = Utc::now().to_rfc3339();
    let source_type = req
        .source_type
        .unwrap_or_else(|| "local_fs".to_string());

    sqlx::query(
        "INSERT INTO sources (id, name, root_path, source_type, enabled, created_at, updated_at)
         VALUES (?, ?, ?, ?, 1, ?, ?)",
    )
    .bind(&id)
    .bind(req.name.trim())
    .bind(req.root_path.trim())
    .bind(&source_type)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "create_source.insert",
            json!({
                "source_id": id,
                "root_path": req.root_path.trim(),
            }),
            err,
        )
    })?;

    let item = Source {
        id,
        name: req.name.trim().to_string(),
        root_path: req.root_path.trim().to_string(),
        source_type,
        enabled: true,
        created_at: now.clone(),
        updated_at: now,
    };

    info!(source_id = item.id, "create_source completed");

    Ok(Json(ApiResponse::ok(item)))
}

pub async fn trigger_source_scan(
    Path(source_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<ScanTriggerResponse>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(source_id, "trigger_source_scan requested");

    let source = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         WHERE id = ?",
    )
    .bind(&source_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "trigger_source_scan.find_source",
            json!({"source_id": source_id}),
            err,
        )
    })?
    .ok_or_else(|| bad_request("source 不存在"))?;

    if !source.enabled {
        return Err(bad_request("source 已禁用，无法扫描"));
    }

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM scan_jobs WHERE source_id = ? AND status IN ('pending', 'running')",
    )
    .bind(&source.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "trigger_source_scan.active_count",
            json!({"source_id": source.id}),
            err,
        )
    })?;

    if active_count > 0 {
        return Err(bad_request("当前 source 已有扫描任务在执行或排队"));
    }

    let job_id = enqueue_scan_job(state.clone(), source)
        .await
        .map_err(|msg| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    code: 500,
                    message: msg,
                    data: json!({}),
                }),
            )
        })?;

    Ok(Json(ApiResponse::ok(ScanTriggerResponse { job_id })))
}

pub async fn get_scan_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Value>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "get_scan_job requested");

    let row = sqlx::query(
        "SELECT id, source_id, status, cancel_requested, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message
         FROM scan_jobs
         WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| internal_db_error("get_scan_job.fetch", json!({"job_id": job_id}), err))?
    .ok_or_else(|| bad_request("scan job 不存在"))?;

    let data = json!({
        "id": row.get::<String, _>("id"),
        "source_id": row.get::<String, _>("source_id"),
        "status": row.get::<String, _>("status"),
        "cancel_requested": row.get::<i64, _>("cancel_requested") == 1,
        "started_at": row.get::<Option<String>, _>("started_at"),
        "finished_at": row.get::<Option<String>, _>("finished_at"),
        "total_count": row.get::<Option<i64>, _>("total_count"),
        "new_count": row.get::<Option<i64>, _>("new_count"),
        "updated_count": row.get::<Option<i64>, _>("updated_count"),
        "failed_count": row.get::<Option<i64>, _>("failed_count"),
        "error_message": row.get::<Option<String>, _>("error_message"),
    });

    Ok(Json(ApiResponse::ok(data)))
}

pub async fn cancel_scan_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "cancel_scan_job requested");

    let row = sqlx::query("SELECT source_id, status FROM scan_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| internal_db_error("cancel_scan_job.fetch", json!({"job_id": job_id}), err))?
        .ok_or_else(|| bad_request("scan job 不存在"))?;

    let source_id: String = row.get("source_id");
    let status: String = row.get("status");
    let now = Utc::now().to_rfc3339();

    if status == "pending" {
        let result = sqlx::query(
            "UPDATE scan_jobs
             SET status = 'cancelled', cancel_requested = 1, finished_at = ?, error_message = ?
             WHERE id = ?",
        )
        .bind(&now)
        .bind("cancel requested before start")
        .bind(&job_id)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "cancel_scan_job.cancel_pending",
                json!({"job_id": job_id}),
                err,
            )
        })?;

        upsert_source_scan_state(
            &state.pool,
            &source_id,
            "cancelled",
            None,
            Some(&now),
            None,
            None,
            None,
            None,
            Some("cancel requested before start"),
        )
        .await
        .map_err(|msg| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    code: 500,
                    message: msg,
                    data: json!({}),
                }),
            )
        })?;

        return Ok(Json(ApiResponse::ok(BatchOperationResult {
            affected: result.rows_affected() as i64,
        })));
    }

    if status == "running" {
        let result = sqlx::query("UPDATE scan_jobs SET cancel_requested = 1 WHERE id = ?")
            .bind(&job_id)
            .execute(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error(
                    "cancel_scan_job.request_running",
                    json!({"job_id": job_id}),
                    err,
                )
            })?;
        return Ok(Json(ApiResponse::ok(BatchOperationResult {
            affected: result.rows_affected() as i64,
        })));
    }

    Ok(Json(ApiResponse::ok(BatchOperationResult { affected: 0 })))
}

pub async fn retry_scan_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<ScanTriggerResponse>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "retry_scan_job requested");

    let row = sqlx::query("SELECT source_id, status FROM scan_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| internal_db_error("retry_scan_job.fetch_job", json!({"job_id": job_id}), err))?
        .ok_or_else(|| bad_request("scan job 不存在"))?;

    let source_id: String = row.get("source_id");
    let status: String = row.get("status");
    if status != "failed" && status != "cancelled" {
        return Err(bad_request("仅支持对 failed/cancelled 任务重试"));
    }

    let source = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         WHERE id = ?",
    )
    .bind(&source_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| internal_db_error("retry_scan_job.fetch_source", json!({"source_id": source_id}), err))?
    .ok_or_else(|| bad_request("source 不存在"))?;

    let active_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM scan_jobs WHERE source_id = ? AND status IN ('pending', 'running')",
    )
    .bind(&source.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| internal_db_error("retry_scan_job.active_count", json!({"source_id": source.id}), err))?;
    if active_count > 0 {
        return Err(bad_request("当前 source 已有扫描任务在执行或排队"));
    }

    let new_job_id = enqueue_scan_job(state.clone(), source)
        .await
        .map_err(|msg| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ApiResponse {
                    code: 500,
                    message: msg,
                    data: json!({}),
                }),
            )
        })?;

    Ok(Json(ApiResponse::ok(ScanTriggerResponse { job_id: new_job_id })))
}

pub async fn search_photos(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PhotoSearchRequest>,
) -> Result<Json<ApiResponse<PagedData<Photo>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(100).clamp(1, 500);
    let offset = (page - 1) * page_size;
    let keyword = req.keyword.unwrap_or_default().trim().to_string();

    info!(
        page,
        page_size,
        offset,
        keyword = %keyword,
        "search_photos requested"
    );

    let (count_sql, list_sql) = if keyword.is_empty() {
        (
            "SELECT COUNT(1) FROM photos WHERE deleted_at IS NULL",
            "SELECT id, source_id, storage_file_id, file_path, file_name, file_ext, file_size, mime_type,
                    content_hash, shot_at, created_at_fs, modified_at_fs, sort_time, width, height,
                    exif_json, gps_lat, gps_lng, remark, deleted_at, created_at, updated_at
             FROM photos
             WHERE deleted_at IS NULL
             ORDER BY sort_time DESC
             LIMIT ? OFFSET ?",
        )
    } else {
        (
            "SELECT COUNT(1) FROM photos WHERE deleted_at IS NULL AND (file_name LIKE ? OR file_path LIKE ? OR IFNULL(remark, '') LIKE ?)",
            "SELECT id, source_id, storage_file_id, file_path, file_name, file_ext, file_size, mime_type,
                    content_hash, shot_at, created_at_fs, modified_at_fs, sort_time, width, height,
                    exif_json, gps_lat, gps_lng, remark, deleted_at, created_at, updated_at
             FROM photos
             WHERE deleted_at IS NULL
               AND (file_name LIKE ? OR file_path LIKE ? OR IFNULL(remark, '') LIKE ?)
             ORDER BY sort_time DESC
             LIMIT ? OFFSET ?",
        )
    };

    let total: i64 = if keyword.is_empty() {
        sqlx::query_scalar(count_sql)
            .fetch_one(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error("search_photos.count", json!({"keyword": keyword.clone()}), err)
            })?
    } else {
        let like = format!("%{}%", keyword);
        sqlx::query_scalar(count_sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .fetch_one(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error("search_photos.count_like", json!({"like": like.clone()}), err)
            })?
    };

    let items: Vec<Photo> = if keyword.is_empty() {
        sqlx::query_as(list_sql)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error(
                    "search_photos.list",
                    json!({"page": page, "page_size": page_size, "offset": offset}),
                    err,
                )
            })?
    } else {
        let like = format!("%{}%", keyword);
        sqlx::query_as(list_sql)
            .bind(&like)
            .bind(&like)
            .bind(&like)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error(
                    "search_photos.list_like",
                    json!({"like": like.clone(), "page": page, "page_size": page_size, "offset": offset}),
                    err,
                )
            })?
    };

    info!(total, returned = items.len(), page, page_size, "search_photos completed");

    Ok(Json(ApiResponse::ok(PagedData {
        total,
        page,
        page_size,
        items,
    })))
}

pub async fn get_photo_detail(
    Path(photo_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<PhotoDetailData>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(photo_id, "get_photo_detail requested");

    let photo = sqlx::query_as::<_, Photo>(
        "SELECT id, source_id, storage_file_id, file_path, file_name, file_ext, file_size, mime_type,
                content_hash, shot_at, created_at_fs, modified_at_fs, sort_time, width, height,
                exif_json, gps_lat, gps_lng, remark, deleted_at, created_at, updated_at
         FROM photos
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(&photo_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| internal_db_error("get_photo_detail.photo", json!({"photo_id": photo_id}), err))?
    .ok_or_else(|| bad_request("photo 不存在"))?;

    let album_rows = sqlx::query(
        "SELECT a.id, a.name
         FROM albums a
         INNER JOIN photo_albums pa ON pa.album_id = a.id
         WHERE pa.photo_id = ?
         ORDER BY a.created_at DESC",
    )
    .bind(&photo.id)
    .fetch_all(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "get_photo_detail.albums",
            json!({"photo_id": photo.id}),
            err,
        )
    })?;

    let albums = album_rows
        .into_iter()
        .map(|row| AlbumSimple {
            id: row.get::<String, _>("id"),
            name: row.get::<String, _>("name"),
        })
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::ok(PhotoDetailData { photo, albums })))
}

pub async fn get_album_detail(
    Path(album_id): Path<String>,
    Query(query): Query<PaginationQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<AlbumDetailData>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(100).clamp(1, 500);
    let offset = (page - 1) * page_size;

    info!(album_id, page, page_size, "get_album_detail requested");

    let album = sqlx::query_as::<_, Album>(
        "SELECT id, name, remark, cover_photo_id, auto_created, album_date, rule_key, created_at, updated_at
         FROM albums
         WHERE id = ?",
    )
    .bind(&album_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| internal_db_error("get_album_detail.album", json!({"album_id": album_id}), err))?
    .ok_or_else(|| bad_request("album 不存在"))?;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(1)
         FROM photo_albums pa
         INNER JOIN photos p ON p.id = pa.photo_id
         WHERE pa.album_id = ? AND p.deleted_at IS NULL",
    )
    .bind(&album.id)
    .fetch_one(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "get_album_detail.count",
            json!({"album_id": album.id}),
            err,
        )
    })?;

    let photos = sqlx::query_as::<_, Photo>(
        "SELECT p.id, p.source_id, p.storage_file_id, p.file_path, p.file_name, p.file_ext, p.file_size, p.mime_type,
                p.content_hash, p.shot_at, p.created_at_fs, p.modified_at_fs, p.sort_time, p.width, p.height,
                p.exif_json, p.gps_lat, p.gps_lng, p.remark, p.deleted_at, p.created_at, p.updated_at
         FROM photos p
         INNER JOIN photo_albums pa ON pa.photo_id = p.id
         WHERE pa.album_id = ? AND p.deleted_at IS NULL
         ORDER BY p.sort_time DESC
         LIMIT ? OFFSET ?",
    )
    .bind(&album.id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "get_album_detail.photos",
            json!({"album_id": album.id, "page": page, "page_size": page_size}),
            err,
        )
    })?;

    let data = AlbumDetailData {
        album,
        photos: PagedData {
            total,
            page,
            page_size,
            items: photos,
        },
    };

    Ok(Json(ApiResponse::ok(data)))
}

pub async fn list_albums(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<Album>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!("list_albums requested");
    let rows = sqlx::query_as::<_, Album>(
        "SELECT id, name, remark, cover_photo_id, auto_created, album_date, rule_key, created_at, updated_at
         FROM albums
         ORDER BY created_at DESC
         LIMIT 200",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|err| internal_db_error("list_albums.fetch_all", json!({}), err))?;

    info!(count = rows.len(), "list_albums completed");

    Ok(Json(ApiResponse::ok(rows)))
}

pub async fn create_album(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateAlbumRequest>,
) -> Result<Json<ApiResponse<Album>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.name.trim().is_empty() {
        return Err(bad_request("album name 不能为空"));
    }

    info!(
        name = req.name.trim(),
        auto_created = req.auto_created.unwrap_or(false),
        album_date = req.album_date.as_deref().unwrap_or(""),
        rule_key = req.rule_key.as_deref().unwrap_or(""),
        "create_album requested"
    );

    let now = Utc::now().to_rfc3339();
    let salt = format!("{}:{}", now, req.name.trim());
    let id = build_album_id(req.name.trim(), req.album_date.as_deref(), &salt);
    let auto_created = req.auto_created.unwrap_or(false);

    sqlx::query(
        "INSERT INTO albums (id, name, remark, cover_photo_id, auto_created, album_date, rule_key, created_at, updated_at)
         VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(req.name.trim())
    .bind(req.remark.as_deref())
    .bind(if auto_created { 1 } else { 0 })
    .bind(req.album_date.as_deref())
    .bind(req.rule_key.as_deref())
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "create_album.insert",
            json!({
                "album_id": id,
                "name": req.name.trim(),
            }),
            err,
        )
    })?;

    let item = Album {
        id,
        name: req.name.trim().to_string(),
        remark: req.remark,
        cover_photo_id: None,
        auto_created,
        album_date: req.album_date,
        rule_key: req.rule_key,
        created_at: now.clone(),
        updated_at: now,
    };

    info!(album_id = item.id, "create_album completed");

    Ok(Json(ApiResponse::ok(item)))
}

pub async fn update_photo_remark(
    Path(photo_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdatePhotoRemarkRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(photo_id, "update_photo_remark requested");
    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE photos
         SET remark = ?, updated_at = ?
         WHERE id = ? AND deleted_at IS NULL",
    )
    .bind(req.remark.as_deref())
    .bind(&now)
    .bind(&photo_id)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "update_photo_remark.update",
            json!({"photo_id": photo_id}),
            err,
        )
    })?;

    Ok(Json(ApiResponse::ok(BatchOperationResult {
        affected: result.rows_affected() as i64,
    })))
}

pub async fn batch_delete_photos(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchDeletePhotosRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.photo_ids.is_empty() {
        return Err(bad_request("photo_ids 不能为空"));
    }

    info!(count = req.photo_ids.len(), "batch_delete_photos requested");
    let now = Utc::now().to_rfc3339();
    let mut affected: i64 = 0;

    for photo_id in &req.photo_ids {
        let result = sqlx::query(
            "UPDATE photos
             SET deleted_at = ?, updated_at = ?
             WHERE id = ? AND deleted_at IS NULL",
        )
        .bind(&now)
        .bind(&now)
        .bind(photo_id)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "batch_delete_photos.update",
                json!({"photo_id": photo_id}),
                err,
            )
        })?;
        affected += result.rows_affected() as i64;
    }

    Ok(Json(ApiResponse::ok(BatchOperationResult { affected })))
}

pub async fn batch_add_to_album(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchAddToAlbumRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.photo_ids.is_empty() {
        return Err(bad_request("photo_ids 不能为空"));
    }
    if req.album_id.trim().is_empty() {
        return Err(bad_request("album_id 不能为空"));
    }

    let album_exists: Option<String> = sqlx::query_scalar("SELECT id FROM albums WHERE id = ?")
        .bind(req.album_id.trim())
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "batch_add_to_album.check_album",
                json!({"album_id": req.album_id}),
                err,
            )
        })?;
    if album_exists.is_none() {
        return Err(bad_request("album 不存在"));
    }

    let now = Utc::now().to_rfc3339();
    let mut affected: i64 = 0;
    for photo_id in &req.photo_ids {
        let result = sqlx::query(
            "INSERT INTO photo_albums (photo_id, album_id, seq_no, created_at)
             VALUES (?, ?, NULL, ?)
             ON CONFLICT(photo_id, album_id) DO NOTHING",
        )
        .bind(photo_id)
        .bind(req.album_id.trim())
        .bind(&now)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "batch_add_to_album.insert",
                json!({"photo_id": photo_id, "album_id": req.album_id}),
                err,
            )
        })?;
        affected += result.rows_affected() as i64;
    }

    Ok(Json(ApiResponse::ok(BatchOperationResult { affected })))
}

pub async fn update_album(
    Path(album_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<UpdateAlbumRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.name.is_none() && req.remark.is_none() {
        return Err(bad_request("name/remark 至少一个字段需要更新"));
    }

    let now = Utc::now().to_rfc3339();
    let mut current_name: Option<String> = None;
    if req.name.is_none() {
        current_name = sqlx::query_scalar("SELECT name FROM albums WHERE id = ?")
            .bind(&album_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error(
                    "update_album.get_name",
                    json!({"album_id": album_id}),
                    err,
                )
            })?;
        if current_name.is_none() {
            return Err(bad_request("album 不存在"));
        }
    }

    let final_name = req.name.clone().or(current_name).unwrap_or_default();
    if final_name.trim().is_empty() {
        return Err(bad_request("album name 不能为空"));
    }

    let result = sqlx::query(
        "UPDATE albums
         SET name = ?, remark = COALESCE(?, remark), updated_at = ?
         WHERE id = ?",
    )
    .bind(final_name.trim())
    .bind(req.remark.as_deref())
    .bind(&now)
    .bind(&album_id)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "update_album.update",
            json!({"album_id": album_id}),
            err,
        )
    })?;

    Ok(Json(ApiResponse::ok(BatchOperationResult {
        affected: result.rows_affected() as i64,
    })))
}

pub async fn set_album_cover(
    Path(album_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetAlbumCoverRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.cover_photo_id.trim().is_empty() {
        return Err(bad_request("cover_photo_id 不能为空"));
    }

    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE albums
         SET cover_photo_id = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(req.cover_photo_id.trim())
    .bind(&now)
    .bind(&album_id)
    .execute(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "set_album_cover.update",
            json!({"album_id": album_id, "cover_photo_id": req.cover_photo_id}),
            err,
        )
    })?;

    Ok(Json(ApiResponse::ok(BatchOperationResult {
        affected: result.rows_affected() as i64,
    })))
}

pub async fn album_add_photos(
    Path(album_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<AlbumPhotosRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.photo_ids.is_empty() {
        return Err(bad_request("photo_ids 不能为空"));
    }

    let now = Utc::now().to_rfc3339();
    let mut affected: i64 = 0;
    for photo_id in &req.photo_ids {
        let result = sqlx::query(
            "INSERT INTO photo_albums (photo_id, album_id, seq_no, created_at)
             VALUES (?, ?, NULL, ?)
             ON CONFLICT(photo_id, album_id) DO NOTHING",
        )
        .bind(photo_id)
        .bind(&album_id)
        .bind(&now)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "album_add_photos.insert",
                json!({"album_id": album_id, "photo_id": photo_id}),
                err,
            )
        })?;
        affected += result.rows_affected() as i64;
    }

    Ok(Json(ApiResponse::ok(BatchOperationResult { affected })))
}

pub async fn album_remove_photos(
    Path(album_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<AlbumPhotosRequest>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    if req.photo_ids.is_empty() {
        return Err(bad_request("photo_ids 不能为空"));
    }

    let mut affected: i64 = 0;
    for photo_id in &req.photo_ids {
        let result = sqlx::query(
            "DELETE FROM photo_albums
             WHERE album_id = ? AND photo_id = ?",
        )
        .bind(&album_id)
        .bind(photo_id)
        .execute(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "album_remove_photos.delete",
                json!({"album_id": album_id, "photo_id": photo_id}),
                err,
            )
        })?;
        affected += result.rows_affected() as i64;
    }

    Ok(Json(ApiResponse::ok(BatchOperationResult { affected })))
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)
}

fn internal_db_error(
    operation: &str,
    context: Value,
    err: sqlx::Error,
) -> (StatusCode, Json<ApiResponse<Value>>) {
    error!(operation, context = %context, error = %err, "database operation failed");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ApiResponse {
            code: 500,
            message: format!("db error at {}: {}", operation, err),
            data: json!({}),
        }),
    )
}

fn bad_request(message: &str) -> (StatusCode, Json<ApiResponse<Value>>) {
    warn!(message, "bad request");
    (
        StatusCode::BAD_REQUEST,
        Json(ApiResponse {
            code: 400,
            message: message.to_string(),
            data: json!({}),
        }),
    )
}

async fn enqueue_scan_job(state: Arc<AppState>, source: Source) -> Result<String, String> {
    let now = Utc::now().to_rfc3339();
    let job_id = sha256_hex(&format!("{}:{}", source.id, now));

    sqlx::query(
        "INSERT INTO scan_jobs (id, source_id, status, cancel_requested, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message)
         VALUES (?, ?, 'pending', 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    )
    .bind(&job_id)
    .bind(&source.id)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        format!(
            "failed to create scan job (job_id={}, source_id={}): {}",
            job_id, source.id, e
        )
    })?;

    info!(job_id, source_id = source.id, from = "none", to = "pending", "scan job status transition");

    let pool_for_task = state.pool.clone();
    let limiter = state.scan_limiter.clone();
    let job_id_for_task = job_id.clone();
    let album_rule_patterns = if state.config.album_rules.enabled {
        build_album_rule_patterns(&state.config)
    } else {
        Vec::new()
    };

    tokio::spawn(async move {
        let permit = limiter.acquire_owned().await;
        if permit.is_err() {
            error!(job_id = job_id_for_task, "failed to acquire scan limiter permit");
            return;
        }
        let _permit_guard = permit.ok();

        if let Err(e) = run_scan_job(
            pool_for_task,
            job_id_for_task.clone(),
            source,
            album_rule_patterns,
        )
        .await
        {
            error!(job_id = job_id_for_task, error = %e, "scan job execution failed at task level");
        }
    });

    Ok(job_id)
}

async fn is_cancel_requested(pool: &SqlitePool, job_id: &str) -> Result<bool, String> {
    let flag: Option<i64> = sqlx::query_scalar("SELECT cancel_requested FROM scan_jobs WHERE id = ?")
        .bind(job_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("failed to check cancel flag for job {}: {}", job_id, e))?;
    Ok(flag.unwrap_or(0) == 1)
}

async fn run_scan_job(
    pool: SqlitePool,
    job_id: String,
    source: Source,
    album_rule_patterns: Vec<AlbumRulePattern>,
) -> Result<(), String> {
    let current_status: Option<String> = sqlx::query_scalar("SELECT status FROM scan_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("failed to read scan job status before run (job_id={}): {}", job_id, e))?;
    if current_status.as_deref() == Some("cancelled") {
        info!(job_id, "scan job already cancelled before start");
        return Ok(());
    }

    let running_at = Utc::now().to_rfc3339();
    let transitioned = sqlx::query(
        "UPDATE scan_jobs
         SET status = 'running', started_at = ?
         WHERE id = ? AND status = 'pending'",
    )
        .bind(&running_at)
        .bind(&job_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            let msg = format!(
                "failed to switch scan job {} status pending->running for source {}: {}",
                job_id, source.id, e
            );
            error!("{}", msg);
            msg
        })?;
    if transitioned.rows_affected() == 0 {
        info!(job_id, "scan job does not transition to running, skip execution");
        return Ok(());
    }

    upsert_source_scan_state(
        &pool,
        &source.id,
        "running",
        Some(&running_at),
        None,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;

    info!(job_id, source_id = source.id, from = "pending", to = "running", "scan job status transition");

    let source_root = source.root_path.clone();
    let collect_result = tokio::task::spawn_blocking(move || {
        let adapter = LocalFsAdapter::new(false);
        let entries = adapter.list_entries(&source_root).map_err(|e| {
            format!(
                "failed to list source entries at {}: {}",
                source_root,
                e
            )
        })?;

        let mut candidates: Vec<ScannedPhotoCandidate> = Vec::new();
        let mut failed_count: i64 = 0;
        let mut first_error: Option<String> = None;

        for entry in entries {
            if entry.is_dir || !is_photo_path(&entry.path) {
                continue;
            }

            let file_path = entry.path.to_string_lossy().to_string();
            let file_name = entry
                .path
                .file_name()
                .map(|v| v.to_string_lossy().to_string())
                .unwrap_or_else(|| file_path.clone());
            let file_ext = file_ext_lowercase(&entry.path);

            let storage_file_id = match adapter.canonical_id(&file_path) {
                Ok(v) => v,
                Err(e) => {
                    failed_count += 1;
                    let msg = format!("failed to get canonical_id for {}: {}", file_path, e);
                    warn!("{}", msg);
                    if first_error.is_none() {
                        first_error = Some(msg);
                    }
                    continue;
                }
            };

            let content_hash = match adapter.sha256(&file_path) {
                Ok(v) => v,
                Err(e) => {
                    failed_count += 1;
                    let msg = format!("failed to calculate sha256 for {}: {}", file_path, e);
                    warn!("{}", msg);
                    if first_error.is_none() {
                        first_error = Some(msg);
                    }
                    continue;
                }
            };

            let (shot_at, exif_json) = match parse_exif_for_scan(&file_path) {
                Ok(v) => v,
                Err(e) => {
                    warn!(file_path = file_path, error = %e, "failed to parse exif, fallback to fs time");
                    (None, None)
                }
            };

            let created_at_fs = entry
                .created_at
                .map(|t| DateTime::<Utc>::from(t).to_rfc3339());

            let modified_at_fs = entry
                .modified_at
                .map(|t| DateTime::<Utc>::from(t).to_rfc3339());

            let sort_time = shot_at
                .clone()
                .or_else(|| created_at_fs.clone())
                .or_else(|| modified_at_fs.clone())
                .unwrap_or_else(|| Utc::now().to_rfc3339());

            candidates.push(ScannedPhotoCandidate {
                storage_file_id,
                file_path,
                file_name,
                file_ext: file_ext.clone(),
                file_size: std::cmp::min(entry.size, i64::MAX as u64) as i64,
                mime_type: mime_from_ext(file_ext.as_deref()),
                content_hash,
                shot_at,
                created_at_fs,
                modified_at_fs,
                sort_time,
                exif_json,
            });
        }

        Ok::<ScanCollectResult, String>(ScanCollectResult {
            candidates,
            failed_count,
            first_error,
        })
    })
    .await
    .map_err(|e| {
        let msg = format!("scan task join failed for job {}: {}", job_id, e);
        error!("{}", msg);
        msg
    })?;

    match collect_result {
        Ok(result) => {
            let mut new_count: i64 = 0;
            let mut updated_count: i64 = 0;
            let mut skipped_count: i64 = 0;
            let mut failed_count: i64 = result.failed_count;
            let now = Utc::now().to_rfc3339();
            let mut auto_album_link_count: i64 = 0;
            let mut album_cache: HashMap<String, String> = HashMap::new();
            let mut last_scanned_path: Option<String> = None;
            let mut last_scanned_storage_file_id: Option<String> = None;
            let mut last_scanned_modified_at: Option<String> = None;
            let mut last_scanned_content_hash: Option<String> = None;

            if is_cancel_requested(&pool, &job_id).await? {
                let finished_at = Utc::now().to_rfc3339();
                sqlx::query(
                    "UPDATE scan_jobs
                     SET status = 'cancelled', finished_at = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                     WHERE id = ?",
                )
                .bind(&finished_at)
                .bind(result.candidates.len() as i64)
                .bind(new_count)
                .bind(updated_count)
                .bind(failed_count)
                .bind("cancel requested before photo upsert")
                .bind(&job_id)
                .execute(&pool)
                .await
                .map_err(|e| {
                    format!(
                        "failed to switch scan job {} status running->cancelled for source {}: {}",
                        job_id, source.id, e
                    )
                })?;

                upsert_source_scan_state(
                    &pool,
                    &source.id,
                    "cancelled",
                    Some(&running_at),
                    Some(&finished_at),
                    None,
                    None,
                    None,
                    None,
                    Some("cancel requested before photo upsert"),
                )
                .await?;

                info!(job_id, source_id = source.id, from = "running", to = "cancelled", "scan job status transition");
                return Ok(());
            }

            for candidate in &result.candidates {
                if is_cancel_requested(&pool, &job_id).await? {
                    let finished_at = Utc::now().to_rfc3339();
                    sqlx::query(
                        "UPDATE scan_jobs
                         SET status = 'cancelled', finished_at = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                         WHERE id = ?",
                    )
                    .bind(&finished_at)
                    .bind(result.candidates.len() as i64)
                    .bind(new_count)
                    .bind(updated_count)
                    .bind(failed_count)
                    .bind("cancel requested during running")
                    .bind(&job_id)
                    .execute(&pool)
                    .await
                    .map_err(|e| {
                        format!(
                            "failed to switch scan job {} status running->cancelled for source {}: {}",
                            job_id, source.id, e
                        )
                    })?;

                    upsert_source_scan_state(
                        &pool,
                        &source.id,
                        "cancelled",
                        Some(&running_at),
                        Some(&finished_at),
                        last_scanned_path.as_deref(),
                        last_scanned_storage_file_id.as_deref(),
                        last_scanned_modified_at.as_deref(),
                        last_scanned_content_hash.as_deref(),
                        Some("cancel requested during running"),
                    )
                    .await?;

                    info!(job_id, source_id = source.id, from = "running", to = "cancelled", "scan job status transition");
                    return Ok(());
                }

                last_scanned_path = Some(candidate.file_path.clone());
                last_scanned_storage_file_id = Some(candidate.storage_file_id.clone());
                last_scanned_modified_at = candidate.modified_at_fs.clone();
                last_scanned_content_hash = Some(candidate.content_hash.clone());

                let existing = sqlx::query(
                    "SELECT id, modified_at_fs, content_hash FROM photos WHERE source_id = ? AND storage_file_id = ?",
                )
                .bind(&source.id)
                .bind(&candidate.storage_file_id)
                .fetch_optional(&pool)
                .await
                .map_err(|e| {
                    let msg = format!(
                        "failed to check existing photo (source_id={}, storage_file_id={}, job_id={}): {}",
                        source.id, candidate.storage_file_id, job_id, e
                    );
                    error!("{}", msg);
                    msg
                })?;

                if let Some(row) = existing {
                    let existing_modified_at: Option<String> = row.get("modified_at_fs");
                    let existing_content_hash: Option<String> = row.get("content_hash");
                    if existing_modified_at == candidate.modified_at_fs
                        && existing_content_hash.as_deref() == Some(candidate.content_hash.as_str())
                    {
                        skipped_count += 1;
                        continue;
                    }
                    updated_count += 1;
                } else {
                    new_count += 1;
                }

                let photo_id = build_photo_id(&source.id, &candidate.storage_file_id);
                let upsert_result = sqlx::query(
                    "INSERT INTO photos (
                        id, source_id, storage_file_id, file_path, file_name, file_ext, file_size, mime_type,
                        content_hash, shot_at, created_at_fs, modified_at_fs, sort_time, width, height,
                        exif_json, gps_lat, gps_lng, remark, deleted_at, created_at, updated_at
                     ) VALUES (
                        ?, ?, ?, ?, ?, ?, ?, ?,
                        ?, ?, ?, ?, ?, NULL, NULL,
                        ?, NULL, NULL, NULL, NULL, ?, ?
                     )
                     ON CONFLICT(source_id, storage_file_id) DO UPDATE SET
                        file_path = excluded.file_path,
                        file_name = excluded.file_name,
                        file_ext = excluded.file_ext,
                        file_size = excluded.file_size,
                        mime_type = excluded.mime_type,
                        content_hash = excluded.content_hash,
                        shot_at = excluded.shot_at,
                        created_at_fs = excluded.created_at_fs,
                        modified_at_fs = excluded.modified_at_fs,
                        sort_time = excluded.sort_time,
                        exif_json = excluded.exif_json,
                        deleted_at = NULL,
                        updated_at = excluded.updated_at",
                )
                .bind(&photo_id)
                .bind(&source.id)
                .bind(&candidate.storage_file_id)
                .bind(&candidate.file_path)
                .bind(&candidate.file_name)
                .bind(candidate.file_ext.as_deref())
                .bind(candidate.file_size)
                .bind(candidate.mime_type.as_deref())
                .bind(&candidate.content_hash)
                .bind(candidate.shot_at.as_deref())
                .bind(candidate.created_at_fs.as_deref())
                .bind(candidate.modified_at_fs.as_deref())
                .bind(&candidate.sort_time)
                .bind(candidate.exif_json.as_deref())
                .bind(&now)
                .bind(&now)
                .execute(&pool)
                .await;

                if let Err(e) = upsert_result {
                    failed_count += 1;
                    error!(
                        job_id,
                        source_id = source.id,
                        file_path = candidate.file_path,
                        storage_file_id = candidate.storage_file_id,
                        error = %e,
                        "photo upsert failed"
                    );
                    continue;
                }

                match ensure_album_by_dir_rule(
                    &pool,
                    &source.id,
                    &candidate.file_path,
                    &photo_id,
                    &album_rule_patterns,
                    &mut album_cache,
                )
                .await
                {
                    Ok(linked) => {
                        if linked {
                            auto_album_link_count += 1;
                        }
                    }
                    Err(e) => {
                        failed_count += 1;
                        error!(
                            job_id,
                            source_id = source.id,
                            file_path = candidate.file_path,
                            error = %e,
                            "auto album linking failed"
                        );
                    }
                }
            }

            let total_count = (result.candidates.len() as i64) + result.failed_count;
            let finished_at = Utc::now().to_rfc3339();

            upsert_source_scan_state(
                &pool,
                &source.id,
                "success",
                Some(&running_at),
                Some(&finished_at),
                last_scanned_path.as_deref(),
                last_scanned_storage_file_id.as_deref(),
                last_scanned_modified_at.as_deref(),
                last_scanned_content_hash.as_deref(),
                result.first_error.as_deref(),
            )
            .await?;

            sqlx::query(
                "UPDATE scan_jobs
                 SET status = 'success', finished_at = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                 WHERE id = ?",
            )
            .bind(&finished_at)
            .bind(total_count)
            .bind(new_count)
            .bind(updated_count)
            .bind(failed_count)
            .bind(result.first_error.as_deref())
            .bind(&job_id)
            .execute(&pool)
            .await
            .map_err(|e| {
                let msg = format!(
                    "failed to switch scan job {} status running->success for source {}: {}",
                    job_id, source.id, e
                );
                error!("{}", msg);
                msg
            })?;

            info!(
                job_id,
                source_id = source.id,
                from = "running",
                to = "success",
                total_count,
                new_count,
                updated_count,
                skipped_count,
                auto_album_link_count,
                failed_count,
                "scan job status transition"
            );
            Ok(())
        }
        Err(scan_error) => {
            let finished_at = Utc::now().to_rfc3339();

            upsert_source_scan_state(
                &pool,
                &source.id,
                "failed",
                Some(&running_at),
                Some(&finished_at),
                None,
                None,
                None,
                None,
                Some(&scan_error),
            )
            .await?;

            sqlx::query(
                "UPDATE scan_jobs
                 SET status = 'failed', finished_at = ?, failed_count = 1, error_message = ?
                 WHERE id = ?",
            )
            .bind(&finished_at)
            .bind(&scan_error)
            .bind(&job_id)
            .execute(&pool)
            .await
            .map_err(|e| {
                let msg = format!(
                    "failed to switch scan job {} status running->failed for source {}: {}",
                    job_id, source.id, e
                );
                error!("{}", msg);
                msg
            })?;

            error!(job_id, source_id = source.id, from = "running", to = "failed", error = %scan_error, "scan job status transition");
            Err(scan_error)
        }
    }
}

#[derive(Debug)]
struct ScanCollectResult {
    candidates: Vec<ScannedPhotoCandidate>,
    failed_count: i64,
    first_error: Option<String>,
}

#[derive(Debug)]
struct ScannedPhotoCandidate {
    storage_file_id: String,
    file_path: String,
    file_name: String,
    file_ext: Option<String>,
    file_size: i64,
    mime_type: Option<String>,
    content_hash: String,
    shot_at: Option<String>,
    created_at_fs: Option<String>,
    modified_at_fs: Option<String>,
    sort_time: String,
    exif_json: Option<String>,
}

fn is_photo_path(path: &StdPath) -> bool {
    match file_ext_lowercase(path).as_deref() {
        Some("jpg")
        | Some("jpeg")
        | Some("png")
        | Some("gif")
        | Some("webp")
        | Some("heic")
        | Some("heif")
        | Some("bmp")
        | Some("tiff")
        | Some("tif") => true,
        _ => false,
    }
}

fn file_ext_lowercase(path: &StdPath) -> Option<String> {
    path.extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

fn mime_from_ext(ext: Option<&str>) -> Option<String> {
    match ext {
        Some("jpg") | Some("jpeg") => Some("image/jpeg".to_string()),
        Some("png") => Some("image/png".to_string()),
        Some("gif") => Some("image/gif".to_string()),
        Some("webp") => Some("image/webp".to_string()),
        Some("heic") => Some("image/heic".to_string()),
        Some("heif") => Some("image/heif".to_string()),
        Some("bmp") => Some("image/bmp".to_string()),
        Some("tiff") | Some("tif") => Some("image/tiff".to_string()),
        _ => None,
    }
}

fn parse_exif_for_scan(file_path: &str) -> Result<(Option<String>, Option<String>), String> {
    let file = std::fs::File::open(file_path).map_err(|e| {
        format!("failed to open file for exif parse at {}: {}", file_path, e)
    })?;
    let mut reader = std::io::BufReader::new(file);

    let exif = match exif::Reader::new().read_from_container(&mut reader) {
        Ok(v) => v,
        Err(_) => return Ok((None, None)),
    };

    let mut exif_map = serde_json::Map::new();
    let mut shot_at: Option<String> = None;

    for field in exif.fields() {
        let key = format!("{:?}", field.tag);
        let value = field.display_value().with_unit(&exif).to_string();
        exif_map.insert(key, serde_json::Value::String(value));
    }

    let shot_raw = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        .map(|f| f.display_value().with_unit(&exif).to_string());

    if let Some(raw) = shot_raw {
        shot_at = parse_exif_datetime_to_rfc3339(&raw);
    }

    let exif_json = if exif_map.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(exif_map).to_string())
    };

    Ok((shot_at, exif_json))
}

fn parse_exif_datetime_to_rfc3339(value: &str) -> Option<String> {
    let normalized = value.trim();
    let parsed = NaiveDateTime::parse_from_str(normalized, "%Y:%m:%d %H:%M:%S").ok()?;
    Some(DateTime::<Utc>::from_naive_utc_and_offset(parsed, Utc).to_rfc3339())
}

async fn ensure_album_by_dir_rule(
    pool: &SqlitePool,
    source_id: &str,
    file_path: &str,
    photo_id: &str,
    rule_patterns: &[AlbumRulePattern],
    cache: &mut HashMap<String, String>,
) -> Result<bool, String> {
    let path = StdPath::new(file_path);
    let dir_name = match path
        .parent()
        .and_then(|v| v.file_name())
        .map(|v| v.to_string_lossy().to_string())
    {
        Some(v) => v,
        None => return Ok(false),
    };

    let matched = match parse_album_from_dir_name_with_patterns(&dir_name, rule_patterns) {
        Some(v) => v,
        None => return Ok(false),
    };

    let cache_key = format!(
        "{}|{}|{}|{}",
        source_id, matched.rule_name, matched.album_date, matched.album_name
    );

    let album_id = if let Some(v) = cache.get(&cache_key) {
        v.clone()
    } else {
        let existing: Option<String> = sqlx::query_scalar(
            "SELECT id FROM albums WHERE auto_created = 1 AND rule_key = ? AND album_date = ? AND name = ? LIMIT 1",
        )
        .bind(&matched.rule_name)
        .bind(&matched.album_date)
        .bind(&matched.album_name)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            format!(
                "failed to lookup auto album (rule={}, date={}, name={}): {}",
                matched.rule_name, matched.album_date, matched.album_name, e
            )
        })?;

        let album_id = match existing {
            Some(id) => id,
            None => {
                let now = Utc::now().to_rfc3339();
                let stable_salt = sha256_hex(&format!("{}:{}", source_id, matched.rule_name));
                let id = build_album_id(
                    &matched.album_name,
                    Some(matched.album_date.as_str()),
                    &stable_salt,
                );
                sqlx::query(
                    "INSERT INTO albums (id, name, remark, cover_photo_id, auto_created, album_date, rule_key, created_at, updated_at)
                     VALUES (?, ?, NULL, NULL, 1, ?, ?, ?, ?)
                     ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        album_date = excluded.album_date,
                        rule_key = excluded.rule_key,
                        updated_at = excluded.updated_at",
                )
                .bind(&id)
                .bind(&matched.album_name)
                .bind(&matched.album_date)
                .bind(&matched.rule_name)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await
                .map_err(|e| {
                    format!(
                        "failed to create auto album (rule={}, date={}, name={}): {}",
                        matched.rule_name, matched.album_date, matched.album_name, e
                    )
                })?;

                info!(
                    album_id = id,
                    rule = matched.rule_name,
                    album_date = matched.album_date,
                    album_name = matched.album_name,
                    "auto album created"
                );
                id
            }
        };

        cache.insert(cache_key, album_id.clone());
        album_id
    };

    sqlx::query(
        "INSERT INTO photo_albums (photo_id, album_id, seq_no, created_at)
         VALUES (?, ?, NULL, ?)
         ON CONFLICT(photo_id, album_id) DO NOTHING",
    )
    .bind(photo_id)
    .bind(&album_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .map_err(|e| {
        format!(
            "failed to link photo to auto album (photo_id={}, album_id={}): {}",
            photo_id, album_id, e
        )
    })?;

    Ok(true)
}

fn build_album_rule_patterns(config: &crate::config::AppConfig) -> Vec<AlbumRulePattern> {
    let mut patterns = patterns_from_delimiters(&config.album_rules.date_delimiters)
        .into_iter()
        .map(AlbumRulePattern::Delimiter)
        .collect::<Vec<_>>();

    for p in &config.album_rules.regex_patterns {
        if p.regex.trim().is_empty() {
            continue;
        }

        patterns.push(AlbumRulePattern::Regex(RegexRulePattern {
            key: p.key.clone(),
            regex: p.regex.clone(),
            date_capture: p.date_capture.clone(),
            name_capture: p.name_capture.clone(),
            date_input_format: p.date_input_format.clone(),
        }));
    }

    patterns
}

#[allow(clippy::too_many_arguments)]
async fn upsert_source_scan_state(
    pool: &SqlitePool,
    source_id: &str,
    status: &str,
    last_scan_started_at: Option<&str>,
    last_scan_finished_at: Option<&str>,
    last_scanned_path: Option<&str>,
    last_scanned_storage_file_id: Option<&str>,
    last_scanned_modified_at: Option<&str>,
    last_scanned_content_hash: Option<&str>,
    last_error_message: Option<&str>,
) -> Result<(), String> {
    let updated_at = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO source_scan_states (
            source_id, status, last_scan_started_at, last_scan_finished_at,
            last_scanned_path, last_scanned_storage_file_id, last_scanned_modified_at,
            last_scanned_content_hash, last_error_message, updated_at
         ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(source_id) DO UPDATE SET
            status = excluded.status,
            last_scan_started_at = excluded.last_scan_started_at,
            last_scan_finished_at = excluded.last_scan_finished_at,
            last_scanned_path = excluded.last_scanned_path,
            last_scanned_storage_file_id = excluded.last_scanned_storage_file_id,
            last_scanned_modified_at = excluded.last_scanned_modified_at,
            last_scanned_content_hash = excluded.last_scanned_content_hash,
            last_error_message = excluded.last_error_message,
            updated_at = excluded.updated_at",
    )
    .bind(source_id)
    .bind(status)
    .bind(last_scan_started_at)
    .bind(last_scan_finished_at)
    .bind(last_scanned_path)
    .bind(last_scanned_storage_file_id)
    .bind(last_scanned_modified_at)
    .bind(last_scanned_content_hash)
    .bind(last_error_message)
    .bind(&updated_at)
    .execute(pool)
    .await
    .map_err(|e| {
        let msg = format!(
            "failed to upsert source_scan_states (source_id={}, status={}): {}",
            source_id, status, e
        );
        error!("{}", msg);
        msg
    })?;

    Ok(())
}
