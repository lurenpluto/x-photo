mod common;

use axum::http::Method;
use common::{
    build_test_app, build_test_image, call_json, create_source, trigger_scan, wait_scan_terminal,
};
use serde_json::json;
use tempfile::TempDir;

#[tokio::test]
async fn cancel_scan_should_reach_cancelled() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("cancel_case");
    std::fs::create_dir_all(&photos_root).expect("create photos root");

    for i in 0..250 {
        build_test_image(&photos_root.join(format!("IMG_{:04}.jpg", i + 1)));
    }

    let test_app = build_test_app(tmp.path(), "cancel_scan.db").await;
    let app = test_app.app;
    let source_id = create_source(&app, "test-source", &photos_root).await;

    let job_id = trigger_scan(&app, &source_id).await;

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

    let test_app = build_test_app(tmp.path(), "duplicate_scan.db").await;
    let app = test_app.app;
    let source_id = create_source(&app, "test-source", &photos_root).await;

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
    assert_ne!(
        second["code"], 0,
        "second trigger should be rejected: {second}"
    );
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
        let mut perms = std::fs::metadata(&broken)
            .expect("metadata broken")
            .permissions();
        perms.set_mode(0o000);
        std::fs::set_permissions(&broken, perms).expect("set broken permission");
    }

    let test_app = build_test_app(tmp.path(), "unreadable_photo.db").await;
    let app = test_app.app;
    let source_id = create_source(&app, "test-source", &photos_root).await;

    let trigger = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(trigger["code"], 0, "trigger failed: {trigger}");
    let job_id = trigger["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();

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
        let mut perms = std::fs::metadata(&broken)
            .expect("metadata broken")
            .permissions();
        perms.set_mode(0o644);
        let _ = std::fs::set_permissions(&broken, perms);
    }
}
