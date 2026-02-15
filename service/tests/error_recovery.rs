use std::path::Path;

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request};
use image::{ImageBuffer, Rgb};
use serde_json::{json, Value};
use service::{api, config::AppConfig, db};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
use tower::ServiceExt;

#[tokio::test]
async fn cancel_scan_should_reach_cancelled() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("cancel_case");
    std::fs::create_dir_all(&photos_root).expect("create photos root");

    for i in 0..250 {
        build_test_image(&photos_root.join(format!("IMG_{:04}.jpg", i + 1)));
    }

    let app = build_app(tmp.path()).await;
    let source_id = create_source(&app, &photos_root).await;

    let trigger = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(trigger["code"], 0, "trigger failed: {trigger}");
    let job_id = trigger["data"]["job_id"].as_str().expect("job id").to_string();

    let cancel = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/scan-jobs/{}/cancel", job_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(cancel["code"], 0);
    let affected = cancel["data"]["affected"].as_i64().unwrap_or(0);
    assert!(affected >= 1, "cancel should affect at least one row");

    let final_status = wait_scan_terminal(&app, &job_id).await;
    assert_eq!(final_status, "cancelled", "scan should end cancelled");
}

#[tokio::test]
async fn duplicate_scan_trigger_should_be_rejected() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("duplicate_case");
    std::fs::create_dir_all(&photos_root).expect("create photos root");

    for i in 0..1500 {
        build_test_image(&photos_root.join(format!("IMG_{:04}.jpg", i + 1)));
    }

    let app = build_app(tmp.path()).await;
    let source_id = create_source(&app, &photos_root).await;

    let first = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(first["code"], 0, "first trigger failed: {first}");

    let second = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_ne!(second["code"], 0, "second trigger should be rejected: {second}");
}

#[tokio::test]
async fn unreadable_photo_should_count_failed_but_not_break_scan() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("broken_file_case");
    std::fs::create_dir_all(&photos_root).expect("create photos root");

    let valid = photos_root.join("IMG_VALID.jpg");
    let broken = photos_root.join("IMG_BROKEN.jpg");
    build_test_image(&valid);
    build_test_image(&broken);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&broken).expect("metadata broken").permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&broken, perms).expect("set broken permission");
    }

    let app = build_app(tmp.path()).await;
    let source_id = create_source(&app, &photos_root).await;

    let trigger = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(trigger["code"], 0, "trigger failed: {trigger}");
    let job_id = trigger["data"]["job_id"].as_str().expect("job id").to_string();

    let final_status = wait_scan_terminal(&app, &job_id).await;
    assert_eq!(final_status, "success");

    let detail = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/scan-jobs/{}", job_id),
        None,
    )
    .await;
    let failed_count = detail["data"]["failed_count"].as_i64().unwrap_or(0);
    let new_count = detail["data"]["new_count"].as_i64().unwrap_or(0);
    assert!(failed_count >= 1, "failed_count should >= 1");
    assert!(new_count >= 1, "new_count should >= 1");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&broken).expect("metadata broken").permissions();
        perms.set_mode(0o644);
        let _ = std::fs::set_permissions(&broken, perms);
    }
}

async fn build_app(db_root: &Path) -> axum::Router {
    let db_file = db_root.join("error_recovery.db");
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
    api::router(pool, cfg)
}

async fn create_source(app: &axum::Router, photos_root: &Path) -> String {
    let create_source_resp = call_json(
        app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": "test-source",
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

async fn wait_scan_terminal(app: &axum::Router, job_id: &str) -> String {
    for _ in 0..120 {
        let status_resp = call_json(app, Method::GET, &format!("/rpc/v1/scan-jobs/{}", job_id), None).await;
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

fn build_test_image(path: &Path) {
    let img = ImageBuffer::from_pixel(640, 360, Rgb([250_u8, 250_u8, 250_u8]));
    img.save(path).expect("save image");
}

async fn call_json(app: &axum::Router, method: Method, path: &str, body: Option<Value>) -> Value {
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
