use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
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
            server: ServerConfig {
                bind_addr: "0.0.0.0:8080".to_string(),
            },
            database: DatabaseConfig {
                url: "sqlite://xphoto.db".to_string(),
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
            bind_addr: "0.0.0.0:8080".to_string(),
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
    pub config: AppConfig,
}

pub fn load() -> Result<LoadedConfig, Box<dyn std::error::Error>> {
    let config_path = resolve_config_path();
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

    Ok(LoadedConfig {
        path: config_path.to_string_lossy().to_string(),
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

    PathBuf::from("config/config.toml")
}
