use std::sync::Arc;

use axum::{routing::get, routing::patch, routing::post, Router};
use sqlx::SqlitePool;

use crate::config::AppConfig;

mod handlers;
mod types;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub config: AppConfig,
}

pub fn router(pool: SqlitePool, config: AppConfig) -> Router {
    let state = Arc::new(AppState { pool, config });

    Router::new()
        .route("/rpc/v1/health", get(handlers::health))
        .route("/rpc/v1/sources", get(handlers::list_sources).post(handlers::create_source))
        .route("/rpc/v1/sources/{source_id}/scan", post(handlers::trigger_source_scan))
        .route("/rpc/v1/scan-jobs/{job_id}", get(handlers::get_scan_job))
        .route("/rpc/v1/photos/search", post(handlers::search_photos))
        .route("/rpc/v1/photos/{photo_id}", get(handlers::get_photo_detail))
        .route("/rpc/v1/photos/{photo_id}/remark", patch(handlers::update_photo_remark))
        .route("/rpc/v1/photos/batch/delete", post(handlers::batch_delete_photos))
        .route("/rpc/v1/photos/batch/add-to-album", post(handlers::batch_add_to_album))
        .route("/rpc/v1/albums", get(handlers::list_albums).post(handlers::create_album))
        .route("/rpc/v1/albums/{album_id}", get(handlers::get_album_detail).patch(handlers::update_album))
        .route("/rpc/v1/albums/{album_id}/cover", patch(handlers::set_album_cover))
        .route("/rpc/v1/albums/{album_id}/photos:add", post(handlers::album_add_photos))
        .route("/rpc/v1/albums/{album_id}/photos:remove", post(handlers::album_remove_photos))
        .with_state(state)
}
