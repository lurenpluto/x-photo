use axum::{routing::get, routing::post, Router};
use sqlx::SqlitePool;

mod handlers;
mod types;

pub fn router(pool: SqlitePool) -> Router {
    Router::new()
        .route("/rpc/v1/health", get(handlers::health))
        .route("/rpc/v1/sources", get(handlers::list_sources).post(handlers::create_source))
        .route("/rpc/v1/photos/search", post(handlers::search_photos))
        .route("/rpc/v1/albums", get(handlers::list_albums).post(handlers::create_album))
        .with_state(pool)
}
