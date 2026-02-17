use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use axum::Router;
use service::{api, config, db, logging};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use tracing::{error, info};

fn env_or_unset(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| "<unset>".to_string())
}

fn collect_boot_env_snapshot() -> Vec<(String, String)> {
    [
        "HOME",
        "USER",
        "HOSTNAME",
        "CONFIG_PATH",
        "BIND_ADDR",
        "DATABASE_URL",
        "LOG_DIR",
        "LOG_LEVEL",
        "PREVIEW_CACHE_ENABLED",
        "PREVIEW_CACHE_DIR",
        "PREVIEW_CACHE_TTL_HOURS",
        "PREVIEW_CACHE_MAX_BYTES",
        "PREVIEW_CACHE_CLEANUP_INTERVAL_SECONDS",
        "PREVIEW_CACHE_WARMUP_ON_SCAN",
        "PREVIEW_CACHE_WARMUP_CONCURRENCY",
        "SCAN_MAX_CONCURRENT_JOBS",
        "SCAN_CHECKPOINT_EVERY",
        "SCAN_SEARCH_INDEX_SYNC_EVERY",
        "SCAN_HASH_PARALLELISM",
        "SCAN_HASH_BATCH_SIZE",
        "SCAN_RESUME_ENABLED",
        "SCAN_TASK_DISPATCH_INTERVAL_MS",
        "SCAN_TASK_STALE_SECONDS",
        "SCAN_SOURCE_CHANGE_DETECT_ENABLED",
        "SCAN_SOURCE_CHANGE_DETECT_INTERVAL_MS",
    ]
    .into_iter()
    .map(|k| (k.to_string(), env_or_unset(k)))
    .collect()
}

fn split_env_snapshot(snapshot: &[(String, String)]) -> (Vec<(String, String)>, Vec<String>) {
    let mut overrides = Vec::new();
    let mut unset = Vec::new();
    for (k, v) in snapshot {
        if v == "<unset>" {
            unset.push(k.clone());
        } else {
            overrides.push((k.clone(), v.clone()));
        }
    }
    (overrides, unset)
}

fn sqlite_file_path(database_url: &str) -> String {
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        return path.to_string();
    }
    "<non-sqlite-url>".to_string()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let loaded = config::load()?;
    let cfg = loaded.config;
    let _log_guard = logging::init_logging("xphoto-service", &cfg.logging.dir, &cfg.logging.level)?;

    let database_url = cfg.database.url.clone();
    let bind_addr = cfg.server.bind_addr.clone();
    let cmd_args: Vec<String> = std::env::args().collect();
    let env_snapshot = collect_boot_env_snapshot();
    let (env_overrides, env_unset_keys) = split_env_snapshot(&env_snapshot);
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("<unknown-cwd>"))
        .to_string_lossy()
        .to_string();
    let exe_path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("<unknown-exe>"))
        .to_string_lossy()
        .to_string();
    let db_file_path = sqlite_file_path(&database_url);

    info!(
        service_name = "xphoto-service",
        service_version = env!("CARGO_PKG_VERSION"),
        crate_name = env!("CARGO_PKG_NAME"),
        rust_debug = cfg!(debug_assertions),
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
        family = std::env::consts::FAMILY,
        cwd = %cwd,
        executable = %exe_path,
        env_snapshot = ?env_snapshot,
        env_overrides = ?env_overrides,
        env_unset_keys = ?env_unset_keys,
        config_path = %loaded.path,
        config_root = %loaded.root,
        database_url = %database_url,
        sqlite_db_file = %db_file_path,
        bind_addr = %bind_addr,
        log_dir = %cfg.logging.dir,
        log_level = %cfg.logging.level,
        preview_cache_enabled = cfg.preview_cache.enabled,
        preview_cache_dir = %cfg.preview_cache.dir,
        preview_cache_ttl_hours = cfg.preview_cache.ttl_hours,
        preview_cache_max_bytes = cfg.preview_cache.max_bytes,
        preview_cache_cleanup_interval_seconds = cfg.preview_cache.cleanup_interval_seconds,
        preview_cache_warmup_on_scan = cfg.preview_cache.warmup_on_scan,
        preview_cache_warmup_concurrency = cfg.preview_cache.warmup_concurrency,
        storage_allow_delete = cfg.storage.allow_delete,
        scan_max_concurrent_jobs = cfg.scan.max_concurrent_jobs,
        scan_checkpoint_every = cfg.scan.checkpoint_every,
        scan_search_index_sync_every = cfg.scan.search_index_sync_every,
        scan_hash_parallelism = cfg.scan.hash_parallelism,
        scan_hash_batch_size = cfg.scan.hash_batch_size,
        scan_resume_enabled = cfg.scan.resume_enabled,
        scan_task_dispatch_interval_ms = cfg.scan.task_dispatch_interval_ms,
        scan_task_stale_seconds = cfg.scan.task_stale_seconds,
        scan_source_change_detect_enabled = cfg.scan.source_change_detect_enabled,
        scan_source_change_detect_interval_ms = cfg.scan.source_change_detect_interval_ms,
        album_rules_enabled = cfg.album_rules.enabled,
        album_rule_regex_count = cfg.album_rules.regex_patterns.len(),
        date_delimiters = ?cfg.album_rules.date_delimiters,
        args = ?cmd_args,
        "bootstrapping service with full runtime context"
    );

    let connect_opts = database_url
        .parse::<SqliteConnectOptions>()
        .map_err(|e| {
            let msg = format!("failed to parse sqlite database url (url={}): {}", database_url, e);
            error!("{}", msg);
            msg
        })?
        .busy_timeout(Duration::from_secs(30))
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(12)
        .connect_with(connect_opts)
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

    let app: Router = api::router(pool, cfg.clone());
    let addr: SocketAddr = bind_addr.parse()?;
    let listener = tokio::net::TcpListener::bind(addr).await?;

    info!(listen_addr = %listener.local_addr()?, "service is ready to serve");
    axum::serve(listener, app).await?;
    Ok(())
}
