use std::sync::Arc;

use axum::{routing::get, routing::patch, routing::post, Router};
use sqlx::SqlitePool;
use tokio::sync::Semaphore;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::Level;

use crate::config::AppConfig;

mod handlers;
mod types;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
    pub scan_limiter: Arc<Semaphore>,
    pub preview_warmup_limiter: Arc<Semaphore>,
}

pub fn router(pool: SqlitePool, config: AppConfig) -> Router {
    let limiter_size = config.scan.max_concurrent_jobs.max(1);
    let preview_warmup_size = config.preview_cache.warmup_concurrency.max(1);
    let state = Arc::new(AppState {
        pool,
        config,
        scan_limiter: Arc::new(Semaphore::new(limiter_size)),
        preview_warmup_limiter: Arc::new(Semaphore::new(preview_warmup_size)),
    });

    handlers::start_task_dispatcher(state.clone());

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let trace = TraceLayer::new_for_http()
        .make_span_with(tower_http::trace::DefaultMakeSpan::new().level(Level::DEBUG))
        .on_request(tower_http::trace::DefaultOnRequest::new().level(Level::DEBUG))
        .on_response(tower_http::trace::DefaultOnResponse::new().level(Level::DEBUG))
        .on_failure(tower_http::trace::DefaultOnFailure::new().level(Level::ERROR));

    Router::new()
        .route("/rpc/v1/health", get(handlers::health))
        .route("/rpc/v1/sources", get(handlers::list_sources).post(handlers::create_source))
        .route("/rpc/v1/sources/{source_id}/scan", post(handlers::trigger_source_scan))
        .route("/rpc/v1/sources/{source_id}/scan:fs-watch", post(handlers::trigger_source_scan_fs_watch))
        .route("/rpc/v1/scan-jobs/{job_id}", get(handlers::get_scan_job))
        .route("/rpc/v1/scan-jobs/{job_id}/cancel", post(handlers::cancel_scan_job))
        .route("/rpc/v1/scan-jobs/{job_id}/retry", post(handlers::retry_scan_job))
        .route("/rpc/v1/task-jobs", get(handlers::list_task_jobs))
        .route("/rpc/v1/task-jobs/active", get(handlers::list_active_task_jobs))
        .route("/rpc/v1/task-jobs/health", get(handlers::get_task_health))
        .route("/rpc/v1/task-jobs/overview", get(handlers::get_task_overview))
        .route("/rpc/v1/task-jobs/{job_id}", get(handlers::get_task_job))
        .route("/rpc/v1/task-jobs/{job_id}/cancel", post(handlers::cancel_task_job))
        .route("/rpc/v1/task-jobs/{job_id}/retry", post(handlers::retry_task_job))
        .route("/rpc/v1/photos/search", post(handlers::search_photos))
        .route("/rpc/v1/photos/favorites", get(handlers::list_favorite_photos))
        .route("/rpc/v1/photos/{photo_id}", get(handlers::get_photo_detail))
        .route("/rpc/v1/photos/{photo_id}/file", get(handlers::get_photo_file))
        .route("/rpc/v1/photos/{photo_id}/remark", patch(handlers::update_photo_remark))
        .route("/rpc/v1/photos/{photo_id}/favorite", patch(handlers::update_photo_favorite))
        .route("/rpc/v1/photos/batch/delete", post(handlers::batch_delete_photos))
        .route("/rpc/v1/photos/batch/add-to-album", post(handlers::batch_add_to_album))
        .route("/rpc/v1/albums", get(handlers::list_albums).post(handlers::create_album))
        .route("/rpc/v1/albums/{album_id}", get(handlers::get_album_detail).patch(handlers::update_album))
        .route("/rpc/v1/albums/{album_id}/cover", patch(handlers::set_album_cover))
        .route("/rpc/v1/albums/{album_id}/photos:add", post(handlers::album_add_photos))
        .route("/rpc/v1/albums/{album_id}/photos:remove", post(handlers::album_remove_photos))
        .with_state(state)
        .layer(cors)
        .layer(trace)
}
