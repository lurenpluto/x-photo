use std::path::PathBuf;

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub root: String,
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub logging: LoggingConfig,
    pub preview_cache: PreviewCacheConfig,
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
pub struct PreviewCacheConfig {
    pub enabled: bool,
    pub dir: String,
    pub ttl: String,
    pub ttl_hours: u64,
    #[serde(deserialize_with = "deserialize_size_bytes")]
    pub max_bytes: u64,
    pub cleanup_interval: String,
    pub cleanup_interval_seconds: u64,
    pub warmup_on_scan: bool,
    pub warmup_concurrency: usize,
}

impl PreviewCacheConfig {
    pub fn ttl_seconds(&self) -> u64 {
        parse_duration_seconds(&self.ttl).unwrap_or_else(|_| self.ttl_hours.saturating_mul(3600))
    }

    pub fn cleanup_interval_seconds_effective(&self) -> u64 {
        parse_duration_seconds(&self.cleanup_interval)
            .unwrap_or(self.cleanup_interval_seconds)
            .max(1)
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub allow_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ScanConfig {
    pub max_concurrent_jobs: usize,
    pub checkpoint_every: usize,
    pub search_index_sync_every: usize,
    pub hash_parallelism: usize,
    pub hash_batch_size: usize,
    pub resume_enabled: bool,
    pub task_dispatch_interval_ms: u64,
    pub task_stale_seconds: i64,
    pub source_change_detect_enabled: bool,
    pub source_change_detect_interval_ms: u64,
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
            preview_cache: PreviewCacheConfig {
                enabled: true,
                dir: "cache/previews".to_string(),
                ttl: "7d".to_string(),
                ttl_hours: 168,
                max_bytes: 8 * 1024 * 1024 * 1024,
                cleanup_interval: "5m".to_string(),
                cleanup_interval_seconds: 300,
                warmup_on_scan: false,
                warmup_concurrency: 2,
            },
            storage: StorageConfig {
                allow_delete: false,
            },
            scan: ScanConfig {
                max_concurrent_jobs: 2,
                checkpoint_every: 50,
                search_index_sync_every: 100,
                hash_parallelism: 0,
                hash_batch_size: 32,
                resume_enabled: true,
                task_dispatch_interval_ms: 2000,
                task_stale_seconds: 120,
                source_change_detect_enabled: false,
                source_change_detect_interval_ms: 30000,
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

impl Default for PreviewCacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dir: "cache/previews".to_string(),
            ttl: "7d".to_string(),
            ttl_hours: 168,
            max_bytes: 8 * 1024 * 1024 * 1024,
            cleanup_interval: "5m".to_string(),
            cleanup_interval_seconds: 300,
            warmup_on_scan: false,
            warmup_concurrency: 2,
        }
    }
}

impl Default for ScanConfig {
    fn default() -> Self {
        Self {
            max_concurrent_jobs: 2,
            checkpoint_every: 50,
            search_index_sync_every: 100,
            hash_parallelism: 0,
            hash_batch_size: 32,
            resume_enabled: true,
            task_dispatch_interval_ms: 2000,
            task_stale_seconds: 120,
            source_change_detect_enabled: false,
            source_change_detect_interval_ms: 30000,
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
    let default_config_path = default_root_dir().join("config.toml");
    ensure_default_config(&config_path, &default_config_path)?;
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
    if let Some(b) = env_parse("PREVIEW_CACHE_ENABLED") {
        config.preview_cache.enabled = b;
    }
    if let Ok(v) = std::env::var("PREVIEW_CACHE_DIR") {
        config.preview_cache.dir = v;
    }
    if let Some(v) = env_string_if("PREVIEW_CACHE_TTL", |v| parse_duration_seconds(v).is_ok()) {
        config.preview_cache.ttl = v;
    }
    if let Some(n) = env_parse::<u64>("PREVIEW_CACHE_TTL_HOURS") {
        config.preview_cache.ttl_hours = n;
        config.preview_cache.ttl = format!("{}h", n);
    }
    if let Ok(v) = std::env::var("PREVIEW_CACHE_MAX_BYTES")
        && let Ok(n) = parse_size_bytes(&v)
    {
        config.preview_cache.max_bytes = n;
    }
    if let Some(n) = env_parse::<u64>("PREVIEW_CACHE_CLEANUP_INTERVAL_SECONDS") {
        config.preview_cache.cleanup_interval_seconds = n;
        config.preview_cache.cleanup_interval = format!("{}s", n);
    }
    if let Some(v) = env_string_if("PREVIEW_CACHE_CLEANUP_INTERVAL", |v| {
        parse_duration_seconds(v).is_ok()
    }) {
        config.preview_cache.cleanup_interval = v;
    }
    if let Some(b) = env_parse("PREVIEW_CACHE_WARMUP_ON_SCAN") {
        config.preview_cache.warmup_on_scan = b;
    }
    if let Some(n) = env_parse("PREVIEW_CACHE_WARMUP_CONCURRENCY") {
        config.preview_cache.warmup_concurrency = n;
    }
    if let Some(n) = env_parse("SCAN_MAX_CONCURRENT_JOBS") {
        config.scan.max_concurrent_jobs = n;
    }
    if let Some(n) = env_parse("SCAN_CHECKPOINT_EVERY") {
        config.scan.checkpoint_every = n;
    }
    if let Some(n) = env_parse("SCAN_SEARCH_INDEX_SYNC_EVERY") {
        config.scan.search_index_sync_every = n;
    }
    if let Some(n) = env_parse("SCAN_HASH_PARALLELISM") {
        config.scan.hash_parallelism = n;
    }
    if let Some(n) = env_parse("SCAN_HASH_BATCH_SIZE") {
        config.scan.hash_batch_size = n;
    }
    if let Some(b) = env_parse("SCAN_RESUME_ENABLED") {
        config.scan.resume_enabled = b;
    }
    if let Some(n) = env_parse("SCAN_TASK_DISPATCH_INTERVAL_MS") {
        config.scan.task_dispatch_interval_ms = n;
    }
    if let Some(n) = env_parse("SCAN_TASK_STALE_SECONDS") {
        config.scan.task_stale_seconds = n;
    }
    if let Some(b) = env_parse("SCAN_SOURCE_CHANGE_DETECT_ENABLED") {
        config.scan.source_change_detect_enabled = b;
    }
    if let Some(n) = env_parse("SCAN_SOURCE_CHANGE_DETECT_INTERVAL_MS") {
        config.scan.source_change_detect_interval_ms = n;
    }

    let resolved_root = resolve_root_dir(&config.root);
    std::fs::create_dir_all(&resolved_root).map_err(|e| {
        format!(
            "Could not create root directory at {}: {}",
            resolved_root.display(),
            e
        )
    })?;

    if let Some(path) = config.database.url.strip_prefix("sqlite://")
        && !path.starts_with('/')
    {
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

    let log_dir_path = PathBuf::from(&config.logging.dir);
    if log_dir_path.is_relative() {
        config.logging.dir = resolved_root
            .join(log_dir_path)
            .to_string_lossy()
            .to_string();
    }

    let preview_cache_dir_path = PathBuf::from(&config.preview_cache.dir);
    if preview_cache_dir_path.is_relative() {
        config.preview_cache.dir = resolved_root
            .join(preview_cache_dir_path)
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

fn env_parse<T>(key: &str) -> Option<T>
where
    T: std::str::FromStr,
{
    std::env::var(key).ok()?.parse::<T>().ok()
}

fn env_string_if(key: &str, predicate: impl FnOnce(&str) -> bool) -> Option<String> {
    let value = std::env::var(key).ok()?;
    predicate(&value).then_some(value)
}

fn deserialize_size_bytes<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum RawSize {
        Int(u64),
        Text(String),
    }

    match RawSize::deserialize(deserializer)? {
        RawSize::Int(v) => Ok(v),
        RawSize::Text(v) => parse_size_bytes(&v).map_err(serde::de::Error::custom),
    }
}

fn parse_size_bytes(raw: &str) -> Result<u64, String> {
    let text = raw.trim().to_ascii_uppercase();
    if text.is_empty() {
        return Err("empty size value".to_string());
    }

    let split_at = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (num_part, unit_part) = text.split_at(split_at);
    if num_part.is_empty() {
        return Err(format!("invalid size '{}': missing number", raw));
    }

    let num = num_part
        .parse::<f64>()
        .map_err(|e| format!("invalid size '{}': {}", raw, e))?;
    let unit = unit_part.trim();
    let mul: f64 = match unit {
        "" | "B" => 1.0,
        "K" | "KB" => 1024.0,
        "M" | "MB" => 1024.0 * 1024.0,
        "G" | "GB" => 1024.0 * 1024.0 * 1024.0,
        "T" | "TB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => {
            return Err(format!(
                "invalid size unit '{}' in '{}', use B/KB/MB/GB/TB",
                unit, raw
            ));
        }
    };

    let bytes = num * mul;
    if !bytes.is_finite() || bytes < 0.0 {
        return Err(format!("invalid size '{}': out of range", raw));
    }
    Ok(bytes.round() as u64)
}

fn parse_duration_seconds(raw: &str) -> Result<u64, String> {
    let text = raw.trim().to_ascii_lowercase();
    if text.is_empty() {
        return Err("empty duration value".to_string());
    }

    let split_at = text
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(text.len());
    let (num_part, unit_part) = text.split_at(split_at);
    if num_part.is_empty() {
        return Err(format!("invalid duration '{}': missing number", raw));
    }

    let num = num_part
        .parse::<f64>()
        .map_err(|e| format!("invalid duration '{}': {}", raw, e))?;
    let unit = unit_part.trim();
    let mul: f64 = match unit {
        "" | "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
        "m" | "min" | "mins" | "minute" | "minutes" => 60.0,
        "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
        "d" | "day" | "days" => 86400.0,
        _ => {
            return Err(format!(
                "invalid duration unit '{}' in '{}', use s/m/h/d",
                unit, raw
            ));
        }
    };

    let secs = num * mul;
    if !secs.is_finite() || secs < 0.0 {
        return Err(format!("invalid duration '{}': out of range", raw));
    }
    Ok(secs.round() as u64)
}

fn resolve_config_path() -> PathBuf {
    let args: Vec<String> = std::env::args().collect();
    for i in 0..args.len() {
        if args[i] == "--config" && i + 1 < args.len() {
            return expand_user_home_path(&args[i + 1]);
        }
    }

    if let Ok(path) = std::env::var("CONFIG_PATH") {
        return expand_user_home_path(&path);
    }

    default_root_dir().join("config.toml")
}

fn default_root_dir() -> PathBuf {
    if let Some(home) = detect_home_dir() {
        return home.join(".xphoto");
    }
    PathBuf::from(".xphoto")
}

fn resolve_root_dir(raw_root: &str) -> PathBuf {
    if let Some(stripped) = strip_tilde_prefix(raw_root) {
        if let Some(home) = detect_home_dir() {
            return home.join(stripped);
        }
        return PathBuf::from(stripped);
    }
    let p = PathBuf::from(raw_root);
    if p.is_relative() {
        return default_root_dir().join(p);
    }
    p
}

fn expand_user_home_path(raw: &str) -> PathBuf {
    if let Some(stripped) = strip_tilde_prefix(raw) {
        if let Some(home) = detect_home_dir() {
            return home.join(stripped);
        }
        return PathBuf::from(stripped);
    }
    PathBuf::from(raw)
}

fn strip_tilde_prefix(raw: &str) -> Option<&str> {
    if raw == "~" {
        return Some("");
    }
    raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\"))
}

fn detect_home_dir() -> Option<PathBuf> {
    dirs::home_dir()
}

fn ensure_default_config(
    path: &PathBuf,
    default_config_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    if path.exists() {
        return Ok(());
    }

    if path != default_config_path {
        return Err(format!(
            "Config file not found at {}. Use --config with an existing file, or start once without --config to initialize default config at {}",
            path.display(),
            default_config_path.display(),
        )
        .into());
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

[preview_cache]
enabled = true
dir = "cache/previews"
ttl = "7d"
ttl_hours = 168
max_bytes = "8GB"
cleanup_interval = "5m"
cleanup_interval_seconds = 300
warmup_on_scan = false
warmup_concurrency = 2

[storage]
allow_delete = false

[scan]
max_concurrent_jobs = 2
checkpoint_every = 50
search_index_sync_every = 100
hash_parallelism = 0
hash_batch_size = 32
resume_enabled = true
task_dispatch_interval_ms = 2000
task_stale_seconds = 120
source_change_detect_enabled = false
source_change_detect_interval_ms = 30000

[album_rules]
enabled = true
date_delimiters = [".", "_", "-"]
"#;

    std::fs::write(path, default_text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_change_detect_should_be_opt_in_by_default() {
        assert!(!AppConfig::default().scan.source_change_detect_enabled);
        assert!(!ScanConfig::default().source_change_detect_enabled);
    }

    #[test]
    fn generated_default_config_should_keep_source_change_detect_disabled() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let path = tmp.path().join("config.toml");
        ensure_default_config(&path, &path).expect("write default config");

        let content = std::fs::read_to_string(&path).expect("read default config");
        assert!(content.contains("source_change_detect_enabled = false"));

        let parsed: AppConfig = toml::from_str(&content).expect("parse default config");
        assert!(!parsed.scan.source_change_detect_enabled);
    }
}
