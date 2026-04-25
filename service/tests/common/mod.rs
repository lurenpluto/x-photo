#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request};
use filetime::{FileTime, set_file_mtime};
use image::{ImageBuffer, Rgb};
use serde_json::{Value, json};
use service::{api, config::AppConfig, db};
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tokio::time::{Duration, sleep};
use tower::ServiceExt;

/// Test router plus database handle for assertions that need direct SQL access.
pub struct TestApp {
    pub app: axum::Router,
    pub pool: SqlitePool,
}

/// Build a test app with scan settings tuned for deterministic integration tests.
pub async fn build_test_app(db_root: &Path, db_name: &str) -> TestApp {
    build_test_app_with_config(db_root, db_name, |_| {}).await
}

/// Build a test app and allow a caller to customize the default test config.
pub async fn build_test_app_with_config<F>(db_root: &Path, db_name: &str, configure: F) -> TestApp
where
    F: FnOnce(&mut AppConfig),
{
    let db_file = db_root.join(db_name);
    let database_url = format!("sqlite://{}", db_file.display());
    let connect_opts = database_url
        .parse::<SqliteConnectOptions>()
        .expect("parse sqlite connect options")
        .create_if_missing(true)
        .busy_timeout(std::time::Duration::from_secs(5));
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_opts)
        .await
        .expect("connect sqlite");
    db::init_schema(&pool).await.expect("init schema");

    let mut cfg = AppConfig::default();
    cfg.scan.max_concurrent_jobs = 1;
    cfg.scan.task_dispatch_interval_ms = 200;
    configure(&mut cfg);

    TestApp {
        app: api::router(pool.clone(), cfg),
        pool,
    }
}

/// Call the JSON RPC API through the in-memory Axum router.
pub async fn call_json(
    app: &axum::Router,
    method: Method,
    path: &str,
    body: Option<Value>,
) -> Value {
    let payload = body.unwrap_or_else(|| json!({}));
    let req = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .expect("build request");

    let response = app.clone().oneshot(req).await.expect("call route");
    let bytes = to_bytes(response.into_body(), 5 * 1024 * 1024)
        .await
        .expect("read body");
    serde_json::from_slice(&bytes).expect("parse json")
}

/// Create a local filesystem source and return its id.
pub async fn create_source(app: &axum::Router, name: &str, photos_root: &Path) -> String {
    let create_source_resp = call_json(
        app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": name,
            "root_path": photos_root.to_string_lossy(),
            "source_type": "local_fs"
        })),
    )
    .await;
    assert_eq!(create_source_resp["code"], 0);
    create_source_resp["data"]["id"]
        .as_str()
        .expect("source id")
        .to_string()
}

/// Trigger a full source scan and return the created scan job id.
pub async fn trigger_scan(app: &axum::Router, source_id: &str) -> String {
    let trigger_resp = call_json(
        app,
        Method::POST,
        &format!("/rpc/v1/sources/{source_id}/scan"),
        Some(json!({})),
    )
    .await;
    assert_eq!(trigger_resp["code"], 0, "trigger failed: {trigger_resp}");
    trigger_resp["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string()
}

/// Wait for a scan job to finish successfully.
pub async fn wait_scan_success(app: &axum::Router, job_id: &str) {
    let final_status = wait_scan_terminal(app, job_id).await;
    assert_eq!(final_status, "success", "scan should end successfully");
}

/// Wait for a scan job to reach any terminal state.
pub async fn wait_scan_terminal(app: &axum::Router, job_id: &str) -> String {
    for _ in 0..120 {
        let status_resp = call_json(
            app,
            Method::GET,
            &format!("/rpc/v1/scan-jobs/{job_id}"),
            None,
        )
        .await;
        let status = status_resp["data"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if status == "success" || status == "failed" || status == "cancelled" {
            return status;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("scan job does not reach terminal state in time");
}

/// Create a small deterministic JPEG test image.
pub fn build_test_image(path: &Path) {
    let img = ImageBuffer::from_pixel(640, 360, Rgb([240_u8, 240_u8, 240_u8]));
    img.save(path).expect("save image");
    let ft = FileTime::from_unix_time(1_580_513_600, 0);
    set_file_mtime(path, ft).expect("set mtime");
}

/// Generate the canonical sample test dataset and return the manifest path.
pub fn generate_sample_data(output_dir: &Path) -> PathBuf {
    let manifest_path = output_dir.join("manifest.json");
    let builder = env!("CARGO_BIN_EXE_testdata_builder");
    let status = Command::new(builder)
        .arg(output_dir)
        .arg("--strategy")
        .arg("sample")
        .arg("--year")
        .arg("2025")
        .arg("--seed")
        .arg("42")
        .arg("--clean")
        .arg("--manifest")
        .arg(&manifest_path)
        .status()
        .expect("run testdata_builder");
    assert!(status.success(), "testdata_builder should succeed");
    manifest_path
}

/// Assert two floating point values are close enough for GPS metadata checks.
pub fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {} within {} of {}, got {}",
        actual,
        tolerance,
        expected,
        actual
    );
}
