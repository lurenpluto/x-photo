use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub root: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub storage: StorageConfig,
    pub scan: ScanConfig,
    pub album_rules: AlbumRulesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub bind_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub dir: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub allow_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub max_concurrent_jobs: usize,
    pub checkpoint_every: usize,
    pub resume_enabled: bool,
    pub task_dispatch_interval_ms: u64,
    pub task_stale_seconds: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumRulesConfig {
    pub enabled: bool,
    pub date_delimiters: Vec<String>,
    pub regex_patterns: Vec<AlbumRegexRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AlbumRegexRuleConfig {
    pub key: String,
    pub regex: String,
    pub date_capture: String,
    pub name_capture: String,
    pub date_input_format: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            root: default_root_dir().to_string_lossy().to_string(),
            server: ServerConfig {
                bind_addr: "127.0.0.1:8080".to_string(),
            },
            database: DatabaseConfig {
                url: "sqlite://data/xphoto.db".to_string(),
            },
            logging: LoggingConfig {
                dir: "logs".to_string(),
                level: "info".to_string(),
            },
            storage: StorageConfig {
                allow_delete: false,
            },
            scan: ScanConfig {
                max_concurrent_jobs: 2,
                checkpoint_every: 50,
                resume_enabled: true,
                task_dispatch_interval_ms: 2000,
                task_stale_seconds: 120,
            },
            album_rules: AlbumRulesConfig {
                enabled: true,
                date_delimiters: vec![".".to_string(), "_".to_string(), "-".to_string()],
                regex_patterns: Vec::new(),
            },
        }
    }
}

impl Default for AlbumRegexRuleConfig {
    fn default() -> Self {
        Self {
            key: "custom_regex".to_string(),
            regex: String::new(),
            date_capture: "date".to_string(),
            name_capture: "name".to_string(),
            date_input_format: "%Y.%m.%d".to_string(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:8080".to_string(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: "sqlite://xphoto.db".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            dir: "logs".to_string(),
            level: "info".to_string(),
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            allow_delete: false,
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 2,
            checkpoint_every: 50,
            resume_enabled: true,
            task_dispatch_interval_ms: 2000,
            task_stale_seconds: 120,
        }
    }
}

impl Default for AlbumRulesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            date_delimiters: vec![".".to_string(), "_".to_string(), "-".to_string()],
            regex_patterns: Vec::new(),
        }
    }
}

pub struct LoadedConfig {
    pub path: String,
    pub root: String,
    pub config: AppConfig,
}

pub fn load() -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let config_path = resolve_config_path();
    ensure_default_config(&config_path)?;
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        format!(
            "Could not read config file at {}: {}",
            config_path.display(),
            e
        )
    })?;

    let mut config: AppConfig = toml::from_str(&content).map_err(|e| {
        format!(
            "Could not parse config file at {}: {}",
            config_path.display(),
            e
        )
    })?;

    if let Ok(bind_addr) = std::env::var("BIND_ADDR") {
        config.server.bind_addr = bind_addr;
    }
    if let Ok(database_url) = std::env::var("DATABASE_URL") {
        config.database.url = database_url;
    }
    if let Ok(log_dir) = std::env::var("LOG_DIR") {
        config.logging.dir = log_dir;
    }
    if let Ok(log_level) = std::env::var("LOG_LEVEL") {
        config.logging.level = log_level;
    }
    if let Ok(v) = std::env::var("SCAN_MAX_CONCURRENT_JOBS") {
        if let Ok(n) = v.parse::<usize>() {
            config.scan.max_concurrent_jobs = n;
        }
    }
    if let Ok(v) = std::env::var("SCAN_CHECKPOINT_EVERY") {
        if let Ok(n) = v.parse::<usize>() {
            config.scan.checkpoint_every = n;
        }
    }
    if let Ok(v) = std::env::var("SCAN_RESUME_ENABLED") {
        if let Ok(b) = v.parse::<bool>() {
            config.scan.resume_enabled = b;
        }
    }
    if let Ok(v) = std::env::var("SCAN_TASK_DISPATCH_INTERVAL_MS") {
        if let Ok(n) = v.parse::<u64>() {
            config.scan.task_dispatch_interval_ms = n;
        }
    }
    if let Ok(v) = std::env::var("SCAN_TASK_STALE_SECONDS") {
        if let Ok(n) = v.parse::<i64>() {
            config.scan.task_stale_seconds = n;
        }
    }

    let resolved_root = resolve_root_dir(&config.root);
    std::fs::create_dir_all(&resolved_root).map_err(|e| {
        format!(
            "Could not create root directory at {}: {}",
            resolved_root.display(),
            e
        )
    })?;

    if let Some(path) = config.database.url.strip_prefix("sqlite://") {
        if !path.starts_with('/') {
            let db_path = resolved_root.join(path);
            if let Some(parent) = db_path.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    format!(
                        "Could not create database parent directory at {}: {}",
                        parent.display(),
                        e
                    )
                })?;
            }
            config.database.url = format!("sqlite://{}", db_path.to_string_lossy());
        }
    }

    let log_dir_path = PathBuf::from(&config.logging.dir);
    if log_dir_path.is_relative() {
        config.logging.dir = resolved_root
            .join(log_dir_path)
            .to_string_lossy()
            .to_string();
    }

    config.root = resolved_root.to_string_lossy().to_string();

    Ok(LoadedConfig {
        path: config_path.to_string_lossy().to_string(),
        root: config.root.clone(),
        config,
    })
}

fn resolve_config_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--config" && i + 1 < args.len() {
            return PathBuf::from(&args[i + 1]);
        }
    }

    if let Ok(path) = std::env::var("CONFIG_PATH") {
        return PathBuf::from(path);
    }

    default_root_dir().join("config.toml")
}

fn default_root_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".xphoto");
    }
    PathBuf::from(".xphoto")
}

fn resolve_root_dir(raw_root: &str) -> PathBuf {
    if let Some(stripped) = raw_root.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }
    let p = PathBuf::from(raw_root);
    if p.is_relative() {
        return default_root_dir().join(p);
    }
    p
}

fn ensure_default_config(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let default_text = r#"root = "~/.xphoto"

[server]
bind_addr = "127.0.0.1:8080"

[database]
url = "sqlite://data/xphoto.db"

[logging]
dir = "logs"
level = "info"

[storage]
allow_delete = false

[scan]
max_concurrent_jobs = 2
checkpoint_every = 50
resume_enabled = true
task_dispatch_interval_ms = 2000
task_stale_seconds = 120

[album_rules]
enabled = true
date_delimiters = [".", "_", "-"]
"#;

    std::fs::write(path, default_text)?;
    Ok(())
}
