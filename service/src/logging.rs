use std::fs::OpenOptions;
use std::path::PathBuf;

use chrono::Local;
use tracing::{error, info};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

pub fn init_logging(service_name: &str) -> Result<WorkerGuard, Box<dyn std::error::Error>> {
    let log_dir = std::env::var("LOG_DIR").unwrap_or_else(|_| "logs".to_string());
    let log_dir_path = PathBuf::from(&log_dir);

    std::fs::create_dir_all(&log_dir_path).map_err(|e| {
        let msg = format!(
            "Could not create log directory at {}: {}",
            log_dir_path.display(),
            e
        );
        error!("{}", msg);
        msg
    })?;

    let date = Local::now().format("%Y%m%d").to_string();
    let pid = std::process::id();
    let file_name = format!("{}_{}_{}.log", service_name, date, pid);
    let file_path = log_dir_path.join(file_name);

    let log_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&file_path)
        .map_err(|e| {
            let msg = format!("Could not open log file at {}: {}", file_path.display(), e);
            error!("{}", msg);
            msg
        })?;

    let (non_blocking, guard) = tracing_appender::non_blocking(log_file);
    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .with_writer(non_blocking)
        .try_init()
        .map_err(|e| {
            let msg = format!("Could not initialize logging subscriber: {}", e);
            error!("{}", msg);
            msg
        })?;

    info!(
        service_name,
        pid,
        log_dir = %log_dir_path.display(),
        log_file = %file_path.display(),
        "logging initialized"
    );

    Ok(guard)
}
