mod api;
mod config;
mod db;
mod domain;
mod infra;
mod logging;

use std::net::SocketAddr;

use axum::Router;
use sqlx::sqlite::SqlitePoolOptions;
use tracing::{error, info};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = config::load()?;
    let cfg = loaded.config;
    let _log_guard = logging::init_logging("xphoto-service", &cfg.logging.dir, &cfg.logging.level)?;

    let database_url = cfg.database.url;
    let bind_addr = cfg.server.bind_addr;
    let cmd_args: Vec<String> = std::env::args().collect();

    info!(
        config_path = %loaded.path,
        database_url = %database_url,
        bind_addr = %bind_addr,
        log_dir = %cfg.logging.dir,
        log_level = %cfg.logging.level,
        storage_allow_delete = cfg.storage.allow_delete,
        args = ?cmd_args,
        "bootstrapping service with runtime inputs"
    );

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .map_err(|e| {
            let msg = format!("failed to connect sqlite database (url={}): {}", database_url, e);
            error!("{}", msg);
            msg
        })?;

    info!("database connection established");

    db::init_schema(&pool).await.map_err(|e| {
        let msg = format!("failed to initialize schema: {}", e);
        error!("{}", msg);
        msg
    })?;
    info!("database schema initialization completed");

    let app: Router = api::router(pool);
    let addr: SocketAddr = bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(listen_addr = %listener.local_addr()?, "service is ready to serve");
    axum::serve(listener, app).await?;
    Ok(())
}
