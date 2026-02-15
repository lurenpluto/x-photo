use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub bind_addr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub dir: String,
    pub level: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    pub allow_delete: bool,
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
