mod common;

use axum::http::Method;
use common::{build_test_app, call_json, wait_scan_terminal};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn failed_scan_should_support_retry_and_overview() {
    let tmp = TempDir::new().expect("temp dir");
    let test_app = build_test_app(tmp.path(), "task_control.db").await;
    let app = test_app.app;

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
