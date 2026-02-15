use std::path::Path as StdPath;

use axum::{extract::Path, extract::State, http::StatusCode, Json};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tracing::{error, info, warn};

use crate::api::types::{
    ApiResponse, CreateAlbumRequest, CreateSourceRequest, PagedData, PhotoSearchRequest,
    ScanTriggerResponse,
};
use crate::domain::models::{build_album_id, build_photo_id, Album, Photo, Source};
use crate::infra::storage::local_fs::LocalFsAdapter;
use crate::infra::storage::StorageAdapter;

pub async fn health() -> Json<ApiResponse<Value>> {
    info!("health check requested");
    Json(ApiResponse::ok(json!({"status": "ok"})))
}

pub async fn list_sources(
    State(pool): State<SqlitePool>,
) -> Result<Json<ApiResponse<Vec<Source>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!("list_sources requested");
    let rows = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         ORDER BY created_at DESC",
    )
    .fetch_all(&pool)
    .await
    .map_err(|err| internal_db_error("list_sources.fetch_all", json!({}), err))?;

    info!(count = rows.len(), "list_sources completed");

    Ok(Json(ApiResponse::ok(rows)))
}

pub async fn create_source(
    State(pool): State<SqlitePool>,
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
    .execute(&pool)
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
    State(pool): State<SqlitePool>,
) -> Result<Json<ApiResponse<ScanTriggerResponse>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(source_id, "trigger_source_scan requested");

    let source = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         WHERE id = ?",
    )
    .bind(&source_id)
    .fetch_optional(&pool)
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

    let now = Utc::now().to_rfc3339();
    let job_id = sha256_hex(&format!("{}:{}", source.id, now));

    sqlx::query(
        "INSERT INTO scan_jobs (id, source_id, status, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message)
         VALUES (?, ?, 'pending', NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    )
    .bind(&job_id)
    .bind(&source.id)
    .execute(&pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "trigger_source_scan.insert_job",
            json!({"job_id": job_id, "source_id": source.id}),
            err,
        )
    })?;

    info!(job_id, source_id = source.id, from = "none", to = "pending", "scan job status transition");

    let pool_for_task = pool.clone();
    let job_id_for_task = job_id.clone();
    tokio::spawn(async move {
        if let Err(e) = run_scan_job(pool_for_task, job_id_for_task.clone(), source).await {
            error!(job_id = job_id_for_task, error = %e, "scan job execution failed at task level");
        }
    });

    Ok(Json(ApiResponse::ok(ScanTriggerResponse { job_id })))
}

pub async fn get_scan_job(
    Path(job_id): Path<String>,
    State(pool): State<SqlitePool>,
) -> Result<Json<ApiResponse<Value>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "get_scan_job requested");

    let row = sqlx::query(
        "SELECT id, source_id, status, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message
         FROM scan_jobs
         WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_optional(&pool)
    .await
    .map_err(|err| internal_db_error("get_scan_job.fetch", json!({"job_id": job_id}), err))?
    .ok_or_else(|| bad_request("scan job 不存在"))?;

    let data = json!({
        "id": row.get::<String, _>("id"),
        "source_id": row.get::<String, _>("source_id"),
        "status": row.get::<String, _>("status"),
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

pub async fn search_photos(
    State(pool): State<SqlitePool>,
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
            .fetch_one(&pool)
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
            .fetch_one(&pool)
            .await
            .map_err(|err| {
                internal_db_error("search_photos.count_like", json!({"like": like.clone()}), err)
            })?
    };

    let items: Vec<Photo> = if keyword.is_empty() {
        sqlx::query_as(list_sql)
            .bind(page_size)
            .bind(offset)
            .fetch_all(&pool)
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
            .fetch_all(&pool)
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

pub async fn list_albums(
    State(pool): State<SqlitePool>,
) -> Result<Json<ApiResponse<Vec<Album>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!("list_albums requested");
    let rows = sqlx::query_as::<_, Album>(
        "SELECT id, name, remark, cover_photo_id, auto_created, album_date, rule_key, created_at, updated_at
         FROM albums
         ORDER BY created_at DESC
         LIMIT 200",
    )
    .fetch_all(&pool)
    .await
    .map_err(|err| internal_db_error("list_albums.fetch_all", json!({}), err))?;

    info!(count = rows.len(), "list_albums completed");

    Ok(Json(ApiResponse::ok(rows)))
}

pub async fn create_album(
    State(pool): State<SqlitePool>,
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
    .execute(&pool)
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

async fn run_scan_job(pool: SqlitePool, job_id: String, source: Source) -> Result<(), String> {
    let running_at = Utc::now().to_rfc3339();
    sqlx::query("UPDATE scan_jobs SET status = 'running', started_at = ? WHERE id = ?")
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

            let modified_at_fs = entry
                .modified_at
                .map(|t| DateTime::<Utc>::from(t).to_rfc3339());
            let sort_time = modified_at_fs
                .clone()
                .unwrap_or_else(|| Utc::now().to_rfc3339());

            candidates.push(ScannedPhotoCandidate {
                storage_file_id,
                file_path,
                file_name,
                file_ext: file_ext.clone(),
                file_size: std::cmp::min(entry.size, i64::MAX as u64) as i64,
                mime_type: mime_from_ext(file_ext.as_deref()),
                content_hash,
                created_at_fs: None,
                modified_at_fs,
                sort_time,
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
            let mut last_scanned_path: Option<String> = None;
            let mut last_scanned_storage_file_id: Option<String> = None;
            let mut last_scanned_modified_at: Option<String> = None;
            let mut last_scanned_content_hash: Option<String> = None;

            for candidate in &result.candidates {
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
                        ?, NULL, ?, ?, ?, NULL, NULL,
                        NULL, NULL, NULL, NULL, NULL, ?, ?
                     )
                     ON CONFLICT(source_id, storage_file_id) DO UPDATE SET
                        file_path = excluded.file_path,
                        file_name = excluded.file_name,
                        file_ext = excluded.file_ext,
                        file_size = excluded.file_size,
                        mime_type = excluded.mime_type,
                        content_hash = excluded.content_hash,
                        created_at_fs = excluded.created_at_fs,
                        modified_at_fs = excluded.modified_at_fs,
                        sort_time = excluded.sort_time,
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
                .bind(candidate.created_at_fs.as_deref())
                .bind(candidate.modified_at_fs.as_deref())
                .bind(&candidate.sort_time)
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
    created_at_fs: Option<String>,
    modified_at_fs: Option<String>,
    sort_time: String,
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
