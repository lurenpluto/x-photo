use axum::body::{Body, to_bytes};
use axum::http::{Method, Request};
use serde_json::{Value, json};
use service::{api, config::AppConfig, db};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use tempfile::TempDir;
use tokio::time::{Duration, sleep};
use tower::ServiceExt;

#[tokio::test]
async fn failed_scan_should_support_retry_and_overview() {
    let tmp = TempDir::new().expect("temp dir");
    let db_file = tmp.path().join("task_control.db");
    let database_url = format!("sqlite://{}", db_file.display());
    let connect_opts = database_url
        .parse::<SqliteConnectOptions>()
        .expect("parse sqlite connect options")
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(connect_opts)
        .await
        .expect("connect sqlite");
    db::init_schema(&pool).await.expect("init schema");

    let mut cfg = AppConfig::default();
    cfg.scan.task_dispatch_interval_ms = 200;
    cfg.scan.max_concurrent_jobs = 1;
    let app = api::router(pool, cfg);

    let bad_root = tmp.path().join("not_exists_scan_dir");
    let create_source_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": "broken-source",
            "root_path": bad_root.to_string_lossy(),
            "source_type": "local_fs"
        })),
    )
    .await;
    assert_eq!(create_source_resp["code"], 0);

    let source_id = create_source_resp["data"]["id"]
        .as_str()
        .expect("source id")
        .to_string();

    let trigger_resp = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(trigger_resp["code"], 0);
    let first_job_id = trigger_resp["data"]["job_id"]
        .as_str()
        .expect("first job id")
        .to_string();

    wait_scan_terminal(&app, &first_job_id).await;
    let first_status = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/scan-jobs/{}", first_job_id),
        None,
    )
    .await;
    assert_eq!(first_status["data"]["status"], "failed");

    let retry_resp = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/scan-jobs/{}/retry", first_job_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(retry_resp["code"], 0);
    let second_job_id = retry_resp["data"]["job_id"]
        .as_str()
        .expect("second job id")
        .to_string();
    assert_ne!(first_job_id, second_job_id);

    let overview_resp = call_json(&app, Method::GET, "/rpc/v1/task-jobs/overview", None).await;
    assert_eq!(overview_resp["code"], 0);
    assert!(overview_resp["data"]["health"].is_object());
    assert!(overview_resp["data"]["active_tasks"].is_array());
}

async fn wait_scan_terminal(app: &axum::Router, job_id: &str) {
    for _ in 0..80 {
        let status_resp = call_json(
            app,
            Method::GET,
            &format!("/rpc/v1/scan-jobs/{}", job_id),
            None,
        )
        .await;
        let status = status_resp["data"]["status"].as_str().unwrap_or_default();
        if status == "success" || status == "failed" || status == "cancelled" {
            return;
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("scan job does not reach terminal state in time");
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
