use axum::{extract::Path, extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};
use tracing::{error, info, warn};

use crate::api::types::{
    ApiResponse, CreateAlbumRequest, CreateSourceRequest, PagedData, PhotoSearchRequest,
    ScanTriggerResponse,
};
use crate::domain::models::{build_album_id, Album, Photo, Source};
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

    info!(job_id, source_id = source.id, from = "pending", to = "running", "scan job status transition");

    let source_root = source.root_path.clone();
    let scan_result = tokio::task::spawn_blocking(move || {
        let adapter = LocalFsAdapter::new(false);
        let entries = adapter.list_entries(&source_root).map_err(|e| e.to_string())?;
        let file_count = entries.iter().filter(|e| !e.is_dir).count() as i64;
        Ok::<i64, String>(file_count)
    })
    .await
    .map_err(|e| {
        let msg = format!("scan task join failed for job {}: {}", job_id, e);
        error!("{}", msg);
        msg
    })?;

    match scan_result {
        Ok(total_count) => {
            let finished_at = Utc::now().to_rfc3339();
            sqlx::query(
                "UPDATE scan_jobs
                 SET status = 'success', finished_at = ?, total_count = ?, new_count = 0, updated_count = 0, failed_count = 0, error_message = NULL
                 WHERE id = ?",
            )
            .bind(&finished_at)
            .bind(total_count)
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

            info!(job_id, source_id = source.id, from = "running", to = "success", total_count, "scan job status transition");
            Ok(())
        }
        Err(scan_error) => {
            let finished_at = Utc::now().to_rfc3339();
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
