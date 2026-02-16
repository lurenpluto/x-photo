use std::collections::{HashMap, HashSet};
use std::path::Path as StdPath;
use std::sync::Arc;

use axum::{extract::Path, extract::Query, extract::State, http::StatusCode, Json};
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{QueryBuilder, Row, Sqlite, SqlitePool};
use tokio::time::{sleep, Duration};
use tracing::{error, info, warn};

use crate::api::types::{
    AlbumDetailData, AlbumPhotosRequest, AlbumSimple, ApiResponse, BatchAddToAlbumRequest,
    BatchDeletePhotosRequest, BatchOperationResult, CreateAlbumRequest, CreateSourceRequest,
    ActiveTaskQuery, FsWatchScanTriggerRequest, PagedData, PaginationQuery, PhotoDetailData,
    PhotoSearchRequest, ScanTriggerResponse, SetAlbumCoverRequest, DaemonTaskHealthItem,
    TaskHealthData, TaskHealthQuery, TaskJobData, TaskJobsQuery, TaskOverviewData,
    TaskOverviewQuery, UpdateAlbumRequest, UpdatePhotoRemarkRequest,
};
use crate::api::AppState;
use crate::domain::album_rules::{
    patterns_from_delimiters, parse_album_from_dir_name_with_patterns, AlbumRulePattern,
    RegexRulePattern,
};
use crate::domain::models::{build_album_id, build_photo_id, Album, Photo, Source};
use crate::domain::task_consts::{
    STATUS_CANCELLED, STATUS_FAILED, STATUS_PENDING, STATUS_RUNNING, TRIGGER_FS_WATCH,
    TRIGGER_MANUAL, TRIGGER_RETRY, TRIGGER_SYSTEM,
};
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

    let task_job_id = create_scan_task_job(
        state.clone(),
        &source.id,
        TRIGGER_MANUAL,
        json!({"source_id": source.id}),
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

    let job_id = execute_scan_task_job(state.clone(), &task_job_id)
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

pub async fn trigger_source_scan_fs_watch(
    Path(source_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(req): Json<FsWatchScanTriggerRequest>,
) -> Result<Json<ApiResponse<ScanTriggerResponse>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(source_id, changed_paths = req.changed_paths.len(), "trigger_source_scan_fs_watch requested");

    if req.changed_paths.is_empty() {
        return Err(bad_request("changed_paths 不能为空"));
    }

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
            "trigger_source_scan_fs_watch.find_source",
            json!({"source_id": source_id}),
            err,
        )
    })?
    .ok_or_else(|| bad_request("source 不存在"))?;

    if !source.enabled {
        return Err(bad_request("source 已禁用，无法扫描"));
    }

    let existing_job_id: Option<String> = sqlx::query_scalar(
        "SELECT id
         FROM scan_jobs
         WHERE source_id = ? AND trigger_type = 'fs_watch' AND status IN ('pending', 'running')
         ORDER BY started_at DESC, id DESC
         LIMIT 1",
    )
    .bind(&source.id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| {
        internal_db_error(
            "trigger_source_scan_fs_watch.find_existing",
            json!({"source_id": source.id}),
            err,
        )
    })?;

    if let Some(job_id) = existing_job_id {
        info!(source_id = source.id, job_id, "reuse existing fs_watch scan job");
        return Ok(Json(ApiResponse::ok(ScanTriggerResponse { job_id })));
    }

    let payload = json!({
        "source_id": source.id,
        "changed_paths": req.changed_paths,
    });
    let task_job_id = create_scan_task_job(state.clone(), &source.id, TRIGGER_FS_WATCH, payload)
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

    let job_id = execute_scan_task_job(state.clone(), &task_job_id)
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
        "SELECT id, source_id, trigger_type, status, cancel_requested, processed_count, resume_cursor_path, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message
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
        "trigger_type": row.get::<String, _>("trigger_type"),
        "status": row.get::<String, _>("status"),
        "cancel_requested": row.get::<i64, _>("cancel_requested") == 1,
        "processed_count": row.get::<i64, _>("processed_count"),
        "resume_cursor_path": row.get::<Option<String>, _>("resume_cursor_path"),
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

    if status == STATUS_PENDING {
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
            STATUS_CANCELLED,
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

    if status == STATUS_RUNNING {
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
    if status != STATUS_FAILED && status != STATUS_CANCELLED {
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

    let task_job_id = create_scan_task_job(
        state.clone(),
        &source.id,
        TRIGGER_RETRY,
        json!({"source_id": source.id, "retry_from_job_id": job_id}),
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

    let new_job_id = execute_scan_task_job(state.clone(), &task_job_id)
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

pub async fn list_task_jobs(
    Query(query): Query<TaskJobsQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<PagedData<TaskJobData>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(100).clamp(1, 500);
    let offset = (page - 1) * page_size;
    let job_type = query.job_type.unwrap_or_default().trim().to_string();
    let status = query.status.unwrap_or_default().trim().to_string();

    info!(page, page_size, job_type = %job_type, status = %status, "list_task_jobs requested");

    let mut count_builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(1) FROM task_jobs tj WHERE 1=1");
    apply_task_job_filters(&mut count_builder, &job_type, &status);
    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "list_task_jobs.count",
                json!({"job_type": job_type, "status": status}),
                err,
            )
        })?;

    let mut list_builder = QueryBuilder::<Sqlite>::new(
        "SELECT id, job_type, trigger_type, status, is_daemon, heartbeat_at, scan_job_id, payload_json, checkpoint_json,
                progress_done, progress_total, retry_count, max_retries, error_message,
                run_after, started_at, finished_at, created_at, updated_at
         FROM task_jobs tj
         WHERE 1=1",
    );
    apply_task_job_filters(&mut list_builder, &job_type, &status);
    list_builder
        .push(" ORDER BY created_at DESC LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind(offset);

    let rows = list_builder
        .build()
        .fetch_all(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "list_task_jobs.list",
                json!({"page": page, "page_size": page_size}),
                err,
            )
        })?;

    let items = rows.into_iter().map(task_job_from_row).collect::<Vec<_>>();

    Ok(Json(ApiResponse::ok(PagedData {
        total,
        page,
        page_size,
        items,
    })))
}

pub async fn list_active_task_jobs(
    Query(query): Query<ActiveTaskQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<Vec<TaskJobData>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let include_all_daemon = query.include_all_daemon.unwrap_or(true);
    info!(include_all_daemon, "list_active_task_jobs requested");
    let items = fetch_active_tasks(&state.pool, include_all_daemon)
        .await
        .map_err(|err| {
            internal_db_error(
                "list_active_task_jobs.fetch",
                json!({"include_all_daemon": include_all_daemon}),
                err,
            )
        })?;
    Ok(Json(ApiResponse::ok(items)))
}

pub async fn get_task_health(
    Query(query): Query<TaskHealthQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<TaskHealthData>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let stale_after_seconds = query.stale_after_seconds.unwrap_or(15).max(1);
    info!(stale_after_seconds, "get_task_health requested");

    let health = fetch_task_health(&state.pool, stale_after_seconds)
        .await
        .map_err(|err| {
            internal_db_error(
                "get_task_health.fetch",
                json!({"stale_after_seconds": stale_after_seconds}),
                err,
            )
        })?;

    Ok(Json(ApiResponse::ok(health)))
}

pub async fn get_task_overview(
    Query(query): Query<TaskOverviewQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<TaskOverviewData>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let include_all_daemon = query.include_all_daemon.unwrap_or(true);
    let stale_after_seconds = query.stale_after_seconds.unwrap_or(15).max(1);
    info!(include_all_daemon, stale_after_seconds, "get_task_overview requested");

    let active_tasks = fetch_active_tasks(&state.pool, include_all_daemon)
        .await
        .map_err(|err| {
            internal_db_error(
                "get_task_overview.active",
                json!({"include_all_daemon": include_all_daemon}),
                err,
            )
        })?;

    let health = fetch_task_health(&state.pool, stale_after_seconds)
        .await
        .map_err(|err| {
            internal_db_error(
                "get_task_overview.health",
                json!({"stale_after_seconds": stale_after_seconds}),
                err,
            )
        })?;

    Ok(Json(ApiResponse::ok(TaskOverviewData { active_tasks, health })))
}

pub async fn get_task_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<TaskJobData>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "get_task_job requested");

    let row = sqlx::query(
        "SELECT id, job_type, trigger_type, status, is_daemon, heartbeat_at, scan_job_id, payload_json, checkpoint_json,
                progress_done, progress_total, retry_count, max_retries, error_message,
                run_after, started_at, finished_at, created_at, updated_at
         FROM task_jobs
         WHERE id = ?",
    )
    .bind(&job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|err| internal_db_error("get_task_job.fetch", json!({"job_id": job_id}), err))?
    .ok_or_else(|| bad_request("task job 不存在"))?;

    Ok(Json(ApiResponse::ok(task_job_from_row(row))))
}

pub async fn cancel_task_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "cancel_task_job requested");

    let task_row = sqlx::query("SELECT status, scan_job_id FROM task_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| internal_db_error("cancel_task_job.fetch", json!({"job_id": job_id}), err))?
        .ok_or_else(|| bad_request("task job 不存在"))?;

    let task_status: String = task_row.get("status");
    if task_status != STATUS_PENDING && task_status != STATUS_RUNNING {
        return Ok(Json(ApiResponse::ok(BatchOperationResult { affected: 0 })));
    }

    let linked_scan_job_id: Option<String> = task_row.get("scan_job_id");
    if let Some(scan_job_id) = linked_scan_job_id {
        let scan_row = sqlx::query("SELECT source_id, status FROM scan_jobs WHERE id = ?")
            .bind(&scan_job_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|err| {
                internal_db_error(
                    "cancel_task_job.fetch_scan",
                    json!({"job_id": job_id, "scan_job_id": scan_job_id}),
                    err,
                )
            })?;

        if let Some(scan_row) = scan_row {
            let source_id: String = scan_row.get("source_id");
            let scan_status: String = scan_row.get("status");
            let now = Utc::now().to_rfc3339();

            if scan_status == STATUS_PENDING {
                sqlx::query(
                    "UPDATE scan_jobs
                     SET status = 'cancelled', cancel_requested = 1, finished_at = ?, error_message = ?
                     WHERE id = ?",
                )
                .bind(&now)
                .bind("cancelled by task api")
                .bind(&scan_job_id)
                .execute(&state.pool)
                .await
                .map_err(|err| {
                    internal_db_error(
                        "cancel_task_job.cancel_scan_pending",
                        json!({"job_id": job_id, "scan_job_id": scan_job_id}),
                        err,
                    )
                })?;

                upsert_source_scan_state(
                    &state.pool,
                    &source_id,
                    STATUS_CANCELLED,
                    None,
                    Some(&now),
                    None,
                    None,
                    None,
                    None,
                    Some("cancelled by task api"),
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
            } else if scan_status == STATUS_RUNNING {
                sqlx::query("UPDATE scan_jobs SET cancel_requested = 1 WHERE id = ?")
                    .bind(&scan_job_id)
                    .execute(&state.pool)
                    .await
                    .map_err(|err| {
                        internal_db_error(
                            "cancel_task_job.request_scan_running",
                            json!({"job_id": job_id, "scan_job_id": scan_job_id}),
                            err,
                        )
                    })?;
            }
        }
    }

    let result = sqlx::query(
        "UPDATE task_jobs
         SET status = 'cancelled', error_message = COALESCE(error_message, 'cancelled by api'), finished_at = ?, updated_at = ?
         WHERE id = ? AND status IN ('pending', 'running')",
    )
    .bind(Utc::now().to_rfc3339())
    .bind(Utc::now().to_rfc3339())
    .bind(&job_id)
    .execute(&state.pool)
    .await
    .map_err(|err| internal_db_error("cancel_task_job.update", json!({"job_id": job_id}), err))?;

    Ok(Json(ApiResponse::ok(BatchOperationResult {
        affected: result.rows_affected() as i64,
    })))
}

pub async fn retry_task_job(
    Path(job_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiResponse<BatchOperationResult>>, (StatusCode, Json<ApiResponse<Value>>)> {
    info!(job_id, "retry_task_job requested");

    let row = sqlx::query("SELECT status, retry_count, max_retries FROM task_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|err| internal_db_error("retry_task_job.fetch", json!({"job_id": job_id}), err))?
        .ok_or_else(|| bad_request("task job 不存在"))?;

    let status: String = row.get("status");
    let retry_count: i64 = row.get("retry_count");
    let max_retries: i64 = row.get("max_retries");

    if status != STATUS_FAILED && status != STATUS_CANCELLED {
        return Err(bad_request("仅 failed/cancelled 任务支持重试"));
    }
    if retry_count >= max_retries {
        return Err(bad_request("task 已达到最大重试次数"));
    }

    let now = Utc::now().to_rfc3339();
    let result = sqlx::query(
        "UPDATE task_jobs
         SET status = 'pending', retry_count = retry_count + 1,
             scan_job_id = NULL, checkpoint_json = NULL,
             error_message = NULL, started_at = NULL, finished_at = NULL,
             run_after = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(&job_id)
    .execute(&state.pool)
    .await
    .map_err(|err| internal_db_error("retry_task_job.update", json!({"job_id": job_id}), err))?;

    Ok(Json(ApiResponse::ok(BatchOperationResult {
        affected: result.rows_affected() as i64,
    })))
}

pub async fn search_photos(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PhotoSearchRequest>,
) -> Result<Json<ApiResponse<PagedData<Photo>>>, (StatusCode, Json<ApiResponse<Value>>)> {
    let page = req.page.unwrap_or(1).max(1);
    let page_size = req.page_size.unwrap_or(100).clamp(1, 500);
    let offset = (page - 1) * page_size;
    let keyword = req.keyword.unwrap_or_default().trim().to_string();
    let album_id = req.album_id.unwrap_or_default().trim().to_string();
    let source_id = req.source_id.unwrap_or_default().trim().to_string();
    let start_time = req.start_time.unwrap_or_default().trim().to_string();
    let end_time = req.end_time.unwrap_or_default().trim().to_string();
    let order = req.order.unwrap_or_else(|| "desc".to_string());
    let order_desc = !order.eq_ignore_ascii_case("asc");

    info!(
        page,
        page_size,
        offset,
        keyword = %keyword,
        album_id = %album_id,
        source_id = %source_id,
        start_time = %start_time,
        end_time = %end_time,
        order = %order,
        "search_photos requested"
    );

    let mut count_builder = QueryBuilder::<Sqlite>::new("SELECT COUNT(1) FROM photos p WHERE p.deleted_at IS NULL");
    apply_photo_search_filters(
        &mut count_builder,
        &keyword,
        &album_id,
        &source_id,
        &start_time,
        &end_time,
    );

    let total: i64 = count_builder
        .build_query_scalar()
        .fetch_one(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "search_photos.count_dynamic",
                json!({
                    "keyword": keyword,
                    "album_id": album_id,
                    "source_id": source_id,
                    "start_time": start_time,
                    "end_time": end_time,
                }),
                err,
            )
        })?;

    let mut list_builder = QueryBuilder::<Sqlite>::new(
        "SELECT p.id, p.source_id, p.storage_file_id, p.file_path, p.file_name, p.file_ext, p.file_size, p.mime_type,
                p.content_hash, p.shot_at, p.created_at_fs, p.modified_at_fs, p.sort_time, p.width, p.height,
                p.exif_json, p.gps_lat, p.gps_lng, p.remark, p.deleted_at, p.created_at, p.updated_at
         FROM photos p
         WHERE p.deleted_at IS NULL",
    );
    apply_photo_search_filters(
        &mut list_builder,
        &keyword,
        &album_id,
        &source_id,
        &start_time,
        &end_time,
    );

    if order_desc {
        list_builder.push(" ORDER BY p.sort_time DESC");
    } else {
        list_builder.push(" ORDER BY p.sort_time ASC");
    }
    list_builder
        .push(" LIMIT ")
        .push_bind(page_size)
        .push(" OFFSET ")
        .push_bind(offset);

    let items: Vec<Photo> = list_builder
        .build_query_as()
        .fetch_all(&state.pool)
        .await
        .map_err(|err| {
            internal_db_error(
                "search_photos.list_dynamic",
                json!({
                    "page": page,
                    "page_size": page_size,
                    "offset": offset,
                    "order": if order_desc { "desc" } else { "asc" },
                }),
                err,
            )
        })?;

    info!(total, returned = items.len(), page, page_size, "search_photos completed");

    Ok(Json(ApiResponse::ok(PagedData {
        total,
        page,
        page_size,
        items,
    })))
}

fn apply_photo_search_filters(
    builder: &mut QueryBuilder<Sqlite>,
    keyword: &str,
    album_id: &str,
    source_id: &str,
    start_time: &str,
    end_time: &str,
) {
    if !keyword.is_empty() {
        let like = format!("%{}%", keyword);
        builder.push(" AND (p.file_name LIKE ");
        builder.push_bind(like.clone());
        builder.push(" OR p.file_path LIKE ");
        builder.push_bind(like.clone());
        builder.push(" OR IFNULL(p.remark, '') LIKE ");
        builder.push_bind(like);
        builder.push(")");
    }

    if !album_id.is_empty() {
        builder.push(
            " AND EXISTS (SELECT 1 FROM photo_albums pa WHERE pa.photo_id = p.id AND pa.album_id = ",
        );
        builder.push_bind(album_id.to_string());
        builder.push(")");
    }

    if !source_id.is_empty() {
        builder.push(" AND p.source_id = ");
        builder.push_bind(source_id.to_string());
    }

    if !start_time.is_empty() {
        builder.push(" AND p.sort_time >= ");
        builder.push_bind(start_time.to_string());
    }

    if !end_time.is_empty() {
        builder.push(" AND p.sort_time <= ");
        builder.push_bind(end_time.to_string());
    }
}

fn apply_task_job_filters(
    builder: &mut QueryBuilder<Sqlite>,
    job_type: &str,
    status: &str,
) {
    if !job_type.is_empty() {
        builder.push(" AND tj.job_type = ");
        builder.push_bind(job_type.to_string());
    }
    if !status.is_empty() {
        builder.push(" AND tj.status = ");
        builder.push_bind(status.to_string());
    }
}

async fn fetch_active_tasks(
    pool: &SqlitePool,
    include_all_daemon: bool,
) -> Result<Vec<TaskJobData>, sqlx::Error> {
    let sql = if include_all_daemon {
        "SELECT id, job_type, trigger_type, status, is_daemon, heartbeat_at, scan_job_id, payload_json, checkpoint_json,
                progress_done, progress_total, retry_count, max_retries, error_message,
                run_after, started_at, finished_at, created_at, updated_at
         FROM task_jobs
         WHERE status IN ('pending', 'running') OR is_daemon = 1
         ORDER BY updated_at DESC"
    } else {
        "SELECT id, job_type, trigger_type, status, is_daemon, heartbeat_at, scan_job_id, payload_json, checkpoint_json,
                progress_done, progress_total, retry_count, max_retries, error_message,
                run_after, started_at, finished_at, created_at, updated_at
         FROM task_jobs
         WHERE status IN ('pending', 'running')
         ORDER BY updated_at DESC"
    };

    let rows = sqlx::query(sql).fetch_all(pool).await?;
    Ok(rows.into_iter().map(task_job_from_row).collect::<Vec<_>>())
}

async fn fetch_task_health(
    pool: &SqlitePool,
    stale_after_seconds: i64,
) -> Result<TaskHealthData, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, job_type, status, heartbeat_at
         FROM task_jobs
         WHERE is_daemon = 1
         ORDER BY updated_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let now = Utc::now();
    let mut all_healthy = true;
    let mut daemon_tasks = Vec::with_capacity(rows.len());

    for row in rows {
        let id: String = row.get("id");
        let task_type: String = row.get("job_type");
        let status: String = row.get("status");
        let heartbeat_at: Option<String> = row.get("heartbeat_at");

        let heartbeat_healthy = heartbeat_at
            .as_deref()
            .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
            .map(|dt| {
                let dt_utc = dt.with_timezone(&Utc);
                (now - dt_utc).num_seconds() <= stale_after_seconds
            })
            .unwrap_or(false);

        let healthy = status == STATUS_RUNNING && heartbeat_healthy;
        if !healthy {
            all_healthy = false;
        }

        daemon_tasks.push(DaemonTaskHealthItem {
            id,
            task_type,
            status,
            heartbeat_at,
            healthy,
        });
    }

    Ok(TaskHealthData {
        healthy: all_healthy,
        stale_after_seconds,
        daemon_tasks,
    })
}

fn task_job_from_row(row: sqlx::sqlite::SqliteRow) -> TaskJobData {
    let status: String = row.get("status");
    let progress_done: i64 = row.get("progress_done");
    let progress_total: Option<i64> = row.get("progress_total");
    let progress_percent = progress_total.and_then(|total| {
        if total > 0 {
            let percent = (progress_done as f64) * 100.0 / (total as f64);
            Some(percent.min(100.0))
        } else {
            None
        }
    });

    TaskJobData {
        id: row.get("id"),
        job_type: row.get("job_type"),
        trigger_type: row.get("trigger_type"),
        status: status.clone(),
        is_daemon: row.get::<i64, _>("is_daemon") == 1,
        heartbeat_at: row.get("heartbeat_at"),
        scan_job_id: row.get("scan_job_id"),
        payload_json: row.get("payload_json"),
        checkpoint_json: row.get("checkpoint_json"),
        progress_done,
        progress_total,
        progress_percent,
        is_active: status == STATUS_PENDING || status == STATUS_RUNNING,
        retry_count: row.get("retry_count"),
        max_retries: row.get("max_retries"),
        error_message: row.get("error_message"),
        run_after: row.get("run_after"),
        started_at: row.get("started_at"),
        finished_at: row.get("finished_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
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

pub fn start_task_dispatcher(state: Arc<AppState>) {
    let interval_ms = state.config.scan.task_dispatch_interval_ms.max(200);
    tokio::spawn(async move {
        if let Err(e) = ensure_daemon_task_row(state.clone(), "daemon:task_dispatcher", "task_dispatcher").await {
            error!(error = %e, "failed to ensure task dispatcher daemon row");
        }

        if let Err(e) = bootstrap_failed_sources_for_retry(state.clone()).await {
            error!(error = %e, "failed to bootstrap failed sources for retry");
        }

        loop {
            if let Err(e) = update_daemon_heartbeat(state.clone(), "daemon:task_dispatcher").await {
                error!(error = %e, "failed to update task dispatcher heartbeat");
            }
            if let Err(e) = dispatch_one_pending_scan_task(state.clone()).await {
                error!(error = %e, "task dispatcher iteration failed");
            }
            if let Err(e) = reconcile_scan_task_statuses(state.clone()).await {
                error!(error = %e, "scan task reconcile iteration failed");
            }
            sleep(Duration::from_millis(interval_ms)).await;
        }
    });
}

async fn bootstrap_failed_sources_for_retry(state: Arc<AppState>) -> Result<(), String> {
    let failed_source_ids = sqlx::query_scalar::<_, String>(
        "SELECT s.id
         FROM sources s
         INNER JOIN source_scan_states st ON st.source_id = s.id
         WHERE s.enabled = 1 AND st.status = 'failed'",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("failed to load failed sources for startup bootstrap: {}", e))?;

    if failed_source_ids.is_empty() {
        return Ok(());
    }

    let active_scan_sources = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT source_id
         FROM scan_jobs
         WHERE status IN ('pending', 'running')",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("failed to load active scan sources for startup bootstrap: {}", e))?;

    let mut blocked_source_ids = active_scan_sources.into_iter().collect::<HashSet<_>>();

    let active_task_rows = sqlx::query(
        "SELECT payload_json
         FROM task_jobs
         WHERE job_type = 'scan' AND status IN ('pending', 'running')",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("failed to load active scan tasks for startup bootstrap: {}", e))?;

    for row in active_task_rows {
        let payload_json: Option<String> = row.get("payload_json");
        if let Some(source_id) = extract_source_id_from_payload(payload_json.as_deref()) {
            blocked_source_ids.insert(source_id);
        }
    }

    let mut scheduled = 0_i64;
    for source_id in failed_source_ids {
        if blocked_source_ids.contains(&source_id) {
            continue;
        }

        let task_job_id = create_scan_task_job(
            state.clone(),
            &source_id,
            TRIGGER_RETRY,
            json!({
                "source_id": source_id,
                "startup_recovery": true,
            }),
        )
        .await?;

        scheduled += 1;
        info!(task_job_id, source_id, "startup bootstrap scheduled retry scan task for failed source");
    }

    info!(scheduled, "startup bootstrap completed for failed source scan retries");
    Ok(())
}

fn extract_source_id_from_payload(payload_json: Option<&str>) -> Option<String> {
    payload_json
        .and_then(|v| serde_json::from_str::<Value>(v).ok())
        .and_then(|v| v.get("source_id").and_then(|s| s.as_str()).map(|s| s.to_string()))
}

async fn ensure_daemon_task_row(
    state: Arc<AppState>,
    daemon_id: &str,
    daemon_type: &str,
) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "INSERT INTO task_jobs (
            id, job_type, trigger_type, status,
            is_daemon, heartbeat_at,
            scan_job_id,
            payload_json, checkpoint_json,
            progress_done, progress_total,
            retry_count, max_retries,
            error_message, run_after,
            started_at, finished_at,
            created_at, updated_at
         ) VALUES (
            ?, ?, ?, 'running',
            1, ?,
            NULL,
            NULL, NULL,
            0, NULL,
            0, 0,
            NULL, NULL,
            ?, NULL,
            ?, ?
         )
         ON CONFLICT(id) DO UPDATE SET
            is_daemon = 1,
            status = 'running',
            heartbeat_at = excluded.heartbeat_at,
            updated_at = excluded.updated_at",
    )
    .bind(daemon_id)
    .bind(daemon_type)
    .bind(TRIGGER_SYSTEM)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("failed to upsert daemon task row (id={}): {}", daemon_id, e))?;
    Ok(())
}

async fn update_daemon_heartbeat(state: Arc<AppState>, daemon_id: &str) -> Result<(), String> {
    let now = Utc::now().to_rfc3339();
    sqlx::query(
        "UPDATE task_jobs
         SET heartbeat_at = ?, updated_at = ?
         WHERE id = ? AND is_daemon = 1",
    )
    .bind(&now)
    .bind(&now)
    .bind(daemon_id)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("failed to update daemon heartbeat (id={}): {}", daemon_id, e))?;
    Ok(())
}

async fn dispatch_one_pending_scan_task(state: Arc<AppState>) -> Result<(), String> {
    let task_id: Option<String> = sqlx::query_scalar(
        "SELECT id
         FROM task_jobs
         WHERE job_type = 'scan' AND status = 'pending' AND (run_after IS NULL OR run_after <= ?)
         ORDER BY created_at ASC
         LIMIT 1",
    )
    .bind(Utc::now().to_rfc3339())
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| format!("failed to fetch pending scan task: {}", e))?;

    if let Some(task_id) = task_id {
        let _ = execute_scan_task_job(state, &task_id).await?;
    }

    Ok(())
}

async fn reconcile_scan_task_statuses(state: Arc<AppState>) -> Result<(), String> {
    let rows = sqlx::query(
        "SELECT tj.id AS task_id, tj.status AS task_status, tj.scan_job_id,
                tj.updated_at AS task_updated_at,
                sj.status AS scan_status, sj.processed_count, sj.total_count,
                sj.error_message, sj.finished_at
         FROM task_jobs tj
         LEFT JOIN scan_jobs sj ON sj.id = tj.scan_job_id
         WHERE tj.job_type = 'scan'
            AND tj.status IN ('pending', 'running')",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| format!("failed to reconcile scan task statuses: {}", e))?;

    for row in rows {
        let task_id: String = row.get("task_id");
        let task_status: String = row.get("task_status");
        let scan_job_id: Option<String> = row.get("scan_job_id");
        let task_updated_at: String = row.get("task_updated_at");
        let scan_status: Option<String> = row.get("scan_status");
        let processed_count: Option<i64> = row.get("processed_count");
        let total_count: Option<i64> = row.get("total_count");
        let error_message: Option<String> = row.get("error_message");
        let finished_at: Option<String> = row.get("finished_at");

        let now = Utc::now().to_rfc3339();

        if scan_job_id.is_none() {
            if task_status == STATUS_RUNNING {
                let stale_seconds = state.config.scan.task_stale_seconds.max(1);
                let is_stale = DateTime::parse_from_rfc3339(&task_updated_at)
                    .ok()
                    .map(|dt| {
                        let dt_utc = dt.with_timezone(&Utc);
                        (Utc::now() - dt_utc).num_seconds() > stale_seconds
                    })
                    .unwrap_or(false);

                if is_stale {
                    sqlx::query(
                        "UPDATE task_jobs
                         SET status = ?, error_message = ?, finished_at = ?, updated_at = ?
                         WHERE id = ? AND status = ?",
                    )
                    .bind(STATUS_FAILED)
                    .bind("stale running task without scan_job_id")
                    .bind(&now)
                    .bind(&now)
                    .bind(&task_id)
                    .bind(STATUS_RUNNING)
                    .execute(&state.pool)
                    .await
                    .map_err(|e| format!("failed to fail stale task without scan link (task_id={}): {}", task_id, e))?;
                }
            }
            continue;
        }

        let scan_job_id = scan_job_id.unwrap_or_default();
        let scan_status = match scan_status {
            Some(v) => v,
            None => {
                sqlx::query(
                    "UPDATE task_jobs
                     SET status = ?, error_message = ?, finished_at = ?, updated_at = ?
                     WHERE id = ? AND status IN (?, ?)",
                )
                .bind(STATUS_FAILED)
                .bind("linked scan_job not found")
                .bind(&now)
                .bind(&now)
                .bind(&task_id)
                .bind(STATUS_PENDING)
                .bind(STATUS_RUNNING)
                .execute(&state.pool)
                .await
                .map_err(|e| format!("failed to fail task with missing scan row (task_id={}): {}", task_id, e))?;
                continue;
            }
        };

        let processed_count = processed_count.unwrap_or(0);

        if scan_status == STATUS_PENDING || scan_status == STATUS_RUNNING {
            let checkpoint = json!({
                "scan_job_id": scan_job_id,
                "scan_status": scan_status,
            });
            sqlx::query(
                "UPDATE task_jobs
                 SET status = 'running', scan_job_id = ?, checkpoint_json = ?, heartbeat_at = ?, updated_at = ?
                 WHERE id = ? AND status IN ('pending', 'running')",
            )
            .bind(&scan_job_id)
            .bind(checkpoint.to_string())
            .bind(&now)
            .bind(&now)
            .bind(&task_id)
            .execute(&state.pool)
            .await
            .map_err(|e| format!("failed to update running task heartbeat in reconcile (task_id={}, prev_status={}): {}", task_id, task_status, e))?;
            continue;
        }

        let checkpoint = json!({
            "scan_job_id": scan_job_id,
            "scan_status": scan_status,
            "processed_count": processed_count,
            "total_count": total_count,
        });

        sqlx::query(
            "UPDATE task_jobs
             SET status = ?, progress_done = ?, progress_total = ?, error_message = ?,
                 finished_at = COALESCE(?, finished_at), heartbeat_at = ?, checkpoint_json = ?, updated_at = ?
             WHERE id = ? AND status IN ('pending', 'running')",
        )
        .bind(&scan_status)
        .bind(processed_count)
        .bind(total_count)
        .bind(error_message.as_deref())
        .bind(finished_at.as_deref())
        .bind(&now)
        .bind(checkpoint.to_string())
        .bind(&now)
        .bind(&task_id)
        .execute(&state.pool)
        .await
        .map_err(|e| format!("failed to update task final state in reconcile (task_id={}): {}", task_id, e))?;
    }

    Ok(())
}

async fn create_scan_task_job(
    state: Arc<AppState>,
    source_id: &str,
    trigger_type: &str,
    payload: Value,
) -> Result<String, String> {
    let now = Utc::now().to_rfc3339();
    let task_job_id = sha256_hex(&format!("task:{}:{}:{}", trigger_type, source_id, now));
    sqlx::query(
        "INSERT INTO task_jobs (
            id, job_type, trigger_type, status,
            is_daemon, heartbeat_at, scan_job_id,
            payload_json, checkpoint_json,
            progress_done, progress_total,
            retry_count, max_retries,
            error_message, run_after,
            started_at, finished_at,
            created_at, updated_at
         ) VALUES (
            ?, 'scan', ?, 'pending',
            0, NULL,
            NULL,
            ?, NULL,
            0, NULL,
            0, 3,
            NULL, ?,
            NULL, NULL,
            ?, ?
         )",
    )
    .bind(&task_job_id)
    .bind(trigger_type)
    .bind(payload.to_string())
    .bind(&now)
    .bind(&now)
    .bind(&now)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        format!(
            "failed to create scan task job (task_id={}, source_id={}, trigger_type={}): {}",
            task_job_id, source_id, trigger_type, e
        )
    })?;

    Ok(task_job_id)
}

async fn execute_scan_task_job(state: Arc<AppState>, task_job_id: &str) -> Result<String, String> {
    let task_row = sqlx::query(
        "SELECT id, status, trigger_type, scan_job_id, payload_json
         FROM task_jobs
         WHERE id = ? AND job_type = 'scan'",
    )
    .bind(task_job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| format!("failed to fetch scan task job {}: {}", task_job_id, e))?
    .ok_or_else(|| format!("scan task job not found: {}", task_job_id))?;

    let status: String = task_row.get("status");
    if status != STATUS_PENDING {
        let existing_scan_job_id: Option<String> = task_row.get("scan_job_id");
        if let Some(scan_job_id) = existing_scan_job_id {
            return Ok(scan_job_id);
        }

        let checkpoint_json: Option<String> = sqlx::query_scalar(
            "SELECT checkpoint_json FROM task_jobs WHERE id = ?",
        )
        .bind(task_job_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(|e| format!("failed to get checkpoint for task {}: {}", task_job_id, e))?
        .flatten();

        if let Some(checkpoint_json) = checkpoint_json {
            let parsed: Value = serde_json::from_str(&checkpoint_json).unwrap_or_else(|_| json!({}));
            if let Some(scan_job_id) = parsed.get("scan_job_id").and_then(|v| v.as_str()) {
                return Ok(scan_job_id.to_string());
            }
        }

        return Err(format!("task {} is not pending, status={}", task_job_id, status));
    }

    let now = Utc::now().to_rfc3339();
    let transitioned = sqlx::query(
        "UPDATE task_jobs
         SET status = 'running', started_at = ?, updated_at = ?
         WHERE id = ? AND status = 'pending'",
    )
    .bind(&now)
    .bind(&now)
    .bind(task_job_id)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("failed to set task {} running: {}", task_job_id, e))?;

    if transitioned.rows_affected() == 0 {
        for _ in 0..20 {
            let latest = sqlx::query(
                "SELECT status, scan_job_id, checkpoint_json
                 FROM task_jobs
                 WHERE id = ?",
            )
            .bind(task_job_id)
            .fetch_optional(&state.pool)
            .await
            .map_err(|e| format!("failed to re-fetch task {} after race: {}", task_job_id, e))?;

            let Some(latest) = latest else {
                return Err(format!("task {} disappeared during execution race", task_job_id));
            };

            let latest_status: String = latest.get("status");
            let latest_scan_job_id: Option<String> = latest.get("scan_job_id");
            if let Some(scan_job_id) = latest_scan_job_id {
                return Ok(scan_job_id);
            }

            let checkpoint_json: Option<String> = latest.get("checkpoint_json");
            if let Some(checkpoint_json) = checkpoint_json {
                let parsed: Value = serde_json::from_str(&checkpoint_json).unwrap_or_else(|_| json!({}));
                if let Some(scan_job_id) = parsed.get("scan_job_id").and_then(|v| v.as_str()) {
                    return Ok(scan_job_id.to_string());
                }
            }

            if latest_status == STATUS_FAILED || latest_status == STATUS_CANCELLED {
                return Err(format!(
                    "task {} moved to terminal status {} without scan job id",
                    task_job_id, latest_status
                ));
            }

            sleep(Duration::from_millis(25)).await;
        }

        return Err(format!("task {} status changed by other worker", task_job_id));
    }

    let trigger_type: String = task_row.get("trigger_type");
    let payload_json: Option<String> = task_row.get("payload_json");
    let payload: Value = payload_json
        .as_deref()
        .and_then(|v| serde_json::from_str(v).ok())
        .unwrap_or_else(|| json!({}));
    let source_id = payload
        .get("source_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| format!("task {} missing source_id", task_job_id))?;

    let source = sqlx::query_as::<_, Source>(
        "SELECT id, name, root_path, source_type, enabled, created_at, updated_at
         FROM sources
         WHERE id = ?",
    )
    .bind(source_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| format!("failed to load source {} for task {}: {}", source_id, task_job_id, e))?
    .ok_or_else(|| format!("source not found for task {}: {}", task_job_id, source_id))?;

    let scan_job_id = match enqueue_scan_job(state.clone(), source, &trigger_type).await {
        Ok(v) => v,
        Err(e) => {
            let failed_at = Utc::now().to_rfc3339();
            let _ = sqlx::query(
                "UPDATE task_jobs
                 SET status = 'failed', error_message = ?, finished_at = ?, updated_at = ?
                 WHERE id = ?",
            )
            .bind(&e)
            .bind(&failed_at)
            .bind(&failed_at)
            .bind(task_job_id)
            .execute(&state.pool)
            .await;
            return Err(e);
        }
    };

    let now_after_enqueue = Utc::now().to_rfc3339();
    let checkpoint = json!({"scan_job_id": scan_job_id, "scan_status": STATUS_PENDING});
    sqlx::query(
        "UPDATE task_jobs
         SET status = 'running', scan_job_id = ?, checkpoint_json = ?,
             progress_done = 0, progress_total = NULL,
             heartbeat_at = ?,
             finished_at = NULL, updated_at = ?
         WHERE id = ?",
    )
    .bind(&scan_job_id)
    .bind(checkpoint.to_string())
    .bind(&now_after_enqueue)
    .bind(&now_after_enqueue)
    .bind(task_job_id)
    .execute(&state.pool)
    .await
    .map_err(|e| format!("failed to mark task {} running after enqueue: {}", task_job_id, e))?;

    Ok(scan_job_id)
}

async fn enqueue_scan_job(
    state: Arc<AppState>,
    source: Source,
    trigger_type: &str,
) -> Result<String, String> {
    let now = Utc::now().to_rfc3339();
    let job_id = sha256_hex(&format!("{}:{}", source.id, now));

    sqlx::query(
        "INSERT INTO scan_jobs (id, source_id, trigger_type, status, cancel_requested, processed_count, resume_cursor_path, started_at, finished_at, total_count, new_count, updated_count, failed_count, error_message)
         VALUES (?, ?, ?, 'pending', 0, 0, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL)",
    )
    .bind(&job_id)
    .bind(&source.id)
    .bind(trigger_type)
    .execute(&state.pool)
    .await
    .map_err(|e| {
        format!(
            "failed to create scan job (job_id={}, source_id={}, trigger_type={}): {}",
            job_id, source.id, trigger_type, e
        )
    })?;

    info!(job_id, source_id = source.id, trigger_type, from = "none", to = "pending", "scan job status transition");

    let pool_for_task = state.pool.clone();
    let limiter = state.scan_limiter.clone();
    let job_id_for_task = job_id.clone();
    let checkpoint_every = state.config.scan.checkpoint_every.max(1);
    let resume_enabled = state.config.scan.resume_enabled;
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
            checkpoint_every,
            resume_enabled,
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

async fn update_scan_checkpoint(
    pool: &SqlitePool,
    job_id: &str,
    processed_count: i64,
    resume_cursor_path: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE scan_jobs
         SET processed_count = ?, resume_cursor_path = ?
         WHERE id = ?",
    )
    .bind(processed_count)
    .bind(resume_cursor_path)
    .bind(job_id)
    .execute(pool)
    .await
    .map_err(|e| {
        format!(
            "failed to update scan checkpoint (job_id={}, processed_count={}): {}",
            job_id, processed_count, e
        )
    })?;
    Ok(())
}

async fn run_scan_job(
    pool: SqlitePool,
    job_id: String,
    source: Source,
    album_rule_patterns: Vec<AlbumRulePattern>,
    checkpoint_every: usize,
    resume_enabled: bool,
) -> Result<(), String> {
    let current_status: Option<String> = sqlx::query_scalar("SELECT status FROM scan_jobs WHERE id = ?")
        .bind(&job_id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| format!("failed to read scan job status before run (job_id={}): {}", job_id, e))?;
    if current_status.as_deref() == Some(STATUS_CANCELLED) {
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

    let resume_cursor_path: Option<String> = if resume_enabled {
        sqlx::query_scalar("SELECT last_scanned_path FROM source_scan_states WHERE source_id = ?")
            .bind(&source.id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| {
                format!(
                    "failed to load resume cursor for source {} in job {}: {}",
                    source.id, job_id, e
                )
            })?
            .flatten()
    } else {
        None
    };

    let source_root = source.root_path.clone();
    let resume_cursor_for_collect = resume_cursor_path.clone();
    let collect_result = tokio::task::spawn_blocking(move || {
        let adapter = LocalFsAdapter::new(false);
        let mut entries = adapter.list_entries(&source_root).map_err(|e| {
            format!(
                "failed to list source entries at {}: {}",
                source_root,
                e
            )
        })?;
        entries.sort_by_key(|e| e.path.to_string_lossy().to_string());

        let mut candidates: Vec<ScannedPhotoCandidate> = Vec::new();
        let mut failed_count: i64 = 0;
        let mut first_error: Option<String> = None;
        let mut resume_reached = resume_cursor_for_collect.is_none();

        for entry in entries {
            if entry.is_dir || !is_photo_path(&entry.path) {
                continue;
            }

            let file_path = entry.path.to_string_lossy().to_string();

            if !resume_reached {
                if Some(file_path.clone()) == resume_cursor_for_collect {
                    resume_reached = true;
                }
                continue;
            }

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
            let mut processed_count: i64 = 0;
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
                     SET status = 'cancelled', finished_at = ?, processed_count = ?, resume_cursor_path = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                     WHERE id = ?",
                )
                .bind(&finished_at)
                .bind(processed_count)
                .bind(last_scanned_path.as_deref())
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
                         SET status = 'cancelled', finished_at = ?, processed_count = ?, resume_cursor_path = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                         WHERE id = ?",
                    )
                    .bind(&finished_at)
                    .bind(processed_count)
                    .bind(last_scanned_path.as_deref())
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

                processed_count += 1;
                last_scanned_path = Some(candidate.file_path.clone());
                last_scanned_storage_file_id = Some(candidate.storage_file_id.clone());
                last_scanned_modified_at = candidate.modified_at_fs.clone();
                last_scanned_content_hash = Some(candidate.content_hash.clone());

                if processed_count % (checkpoint_every as i64) == 0 {
                    update_scan_checkpoint(
                        &pool,
                        &job_id,
                        processed_count,
                        last_scanned_path.as_deref(),
                    )
                    .await?;
                }

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
                 SET status = 'success', finished_at = ?, processed_count = ?, resume_cursor_path = ?, total_count = ?, new_count = ?, updated_count = ?, failed_count = ?, error_message = ?
                 WHERE id = ?",
            )
            .bind(&finished_at)
            .bind(processed_count)
            .bind(last_scanned_path.as_deref())
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
                 SET status = 'failed', finished_at = ?, processed_count = ?, resume_cursor_path = ?, failed_count = 1, error_message = ?
                 WHERE id = ?",
            )
            .bind(&finished_at)
            .bind(0_i64)
            .bind(Option::<&str>::None)
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
