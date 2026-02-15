use axum::{extract::State, http::StatusCode, Json};
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tracing::{error, info, warn};

use crate::api::types::{
    ApiResponse, CreateAlbumRequest, CreateSourceRequest, PagedData, PhotoSearchRequest,
};
use crate::domain::models::{build_album_id, Album, Photo, Source};

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
