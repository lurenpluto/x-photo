use std::path::Path;
use std::process::Command;

use axum::body::{Body, to_bytes};
use axum::http::{Method, Request};
use filetime::{FileTime, set_file_mtime};
use image::{ImageBuffer, Rgb};
use serde_json::{Value, json};
use service::{api, config::AppConfig, db};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use tokio::time::{Duration, sleep};
use tower::ServiceExt;

#[tokio::test]
async fn scan_and_search_should_work() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    std::fs::create_dir_all(&photos_root).expect("create photos root");

    let dot_album = photos_root.join("2020.02.01.Test.Album");
    let underscore_album = photos_root.join("2020_02_02_Another_Album");
    let misc_album = photos_root.join("misc_folder");
    std::fs::create_dir_all(&dot_album).expect("create dot album dir");
    std::fs::create_dir_all(&underscore_album).expect("create underscore album dir");
    std::fs::create_dir_all(&misc_album).expect("create misc dir");

    build_test_image(&dot_album.join("IMG_A1.jpg"));
    build_test_image(&underscore_album.join("IMG_B2.jpg"));
    build_test_image(&misc_album.join("IMG_C3.jpg"));

    let db_file = tmp.path().join("test.db");
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
    cfg.scan.max_concurrent_jobs = 1;
    cfg.scan.task_dispatch_interval_ms = 200;
    let pool_for_test = pool.clone();
    let app = api::router(pool, cfg);

    let create_source_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": "local-test",
            "root_path": photos_root.to_string_lossy(),
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
    let job_id = trigger_resp["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();

    let mut done = false;
    for _ in 0..50 {
        let status_resp = call_json(
            &app,
            Method::GET,
            &format!("/rpc/v1/scan-jobs/{}", job_id),
            None,
        )
        .await;
        let status = status_resp["data"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if status == "success" {
            done = true;
            break;
        }
        if status == "failed" || status == "cancelled" {
            panic!("scan job ends in {}: {}", status, status_resp);
        }
        sleep(Duration::from_millis(100)).await;
    }
    assert!(done, "scan job not completed in time");

    let search_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(search_resp["code"], 0);
    let total = search_resp["data"]["total"].as_i64().unwrap_or(0);
    assert!(total >= 3, "expected >=3 photos, got {}", total);

    let album_keyword_search = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"keyword": "Test.Album", "page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(album_keyword_search["code"], 0);
    assert!(album_keyword_search["data"]["total"].as_i64().unwrap_or(0) >= 1);

    let one_photo_id = search_resp["data"]["items"][0]["id"]
        .as_str()
        .expect("photo id from search")
        .to_string();
    sqlx::query("UPDATE photos SET exif_json = ? WHERE id = ?")
        .bind(r#"{"Model":"XPhotoCam","LensModel":"SearchableLens777"}"#)
        .bind(&one_photo_id)
        .execute(&pool_for_test)
        .await
        .expect("update exif_json for search test");

    let exif_keyword_search = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"keyword": "SearchableLens777", "page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(exif_keyword_search["code"], 0);
    assert!(exif_keyword_search["data"]["total"].as_i64().unwrap_or(0) >= 1);

    let prefixed_search = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"keyword": "exif:SearchableLens777", "page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(prefixed_search["code"], 0);
    assert!(prefixed_search["data"]["total"].as_i64().unwrap_or(0) >= 1);

    let spaced_prefixed_search = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"keyword": "album: Test.Album", "page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(spaced_prefixed_search["code"], 0);
    assert!(
        spaced_prefixed_search["data"]["total"]
            .as_i64()
            .unwrap_or(0)
            >= 1
    );

    let favorite_set_resp = call_json(
        &app,
        Method::PATCH,
        &format!("/rpc/v1/photos/{}/favorite", one_photo_id),
        Some(json!({"favorite": true})),
    )
    .await;
    assert_eq!(favorite_set_resp["code"], 0);

    let favorite_list_resp = call_json(
        &app,
        Method::GET,
        "/rpc/v1/photos/favorites?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(favorite_list_resp["code"], 0);
    assert!(favorite_list_resp["data"]["total"].as_i64().unwrap_or(0) >= 1);

    let detail_after_favorite = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/photos/{}", one_photo_id),
        None,
    )
    .await;
    assert_eq!(detail_after_favorite["code"], 0);
    assert_eq!(detail_after_favorite["data"]["is_favorite"], true);

    let favorite_unset_resp = call_json(
        &app,
        Method::PATCH,
        &format!("/rpc/v1/photos/{}/favorite", one_photo_id),
        Some(json!({"favorite": false})),
    )
    .await;
    assert_eq!(favorite_unset_resp["code"], 0);

    let albums_resp = call_json(&app, Method::GET, "/rpc/v1/albums", None).await;
    assert_eq!(albums_resp["code"], 0);
    let albums = albums_resp["data"].as_array().cloned().unwrap_or_default();
    assert!(
        albums.iter().any(|a| a["name"] == "Test.Album"),
        "dot-rule auto album missing"
    );
    assert!(
        albums.iter().any(|a| a["name"] == "Another_Album"),
        "underscore-rule auto album missing"
    );
}

#[tokio::test]
async fn generated_sample_data_should_scan_exif_and_album() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("generated_sample");
    let builder = env!("CARGO_BIN_EXE_testdata_builder");
    let status = Command::new(builder)
        .arg(&photos_root)
        .arg("--strategy")
        .arg("sample")
        .arg("--year")
        .arg("2025")
        .arg("--seed")
        .arg("42")
        .status()
        .expect("run testdata_builder");
    assert!(status.success(), "testdata_builder should succeed");

    let db_file = tmp.path().join("generated_sample.db");
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
    cfg.scan.max_concurrent_jobs = 1;
    cfg.scan.task_dispatch_interval_ms = 200;
    let app = api::router(pool, cfg);

    let create_source_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": "generated-sample",
            "root_path": photos_root.to_string_lossy(),
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
    let job_id = trigger_resp["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();

    wait_scan_success(&app, &job_id).await;

    let search_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(search_resp["code"], 0);
    assert_eq!(search_resp["data"]["total"], 1);
    let item = &search_resp["data"]["items"][0];
    assert_eq!(item["file_name"], "IMG_SAMPLE_001.jpg");
    assert_eq!(item["shot_at"], "2020-02-01T10:11:12+00:00");
    assert_eq!(item["sort_time"], "2020-02-01T10:11:12+00:00");
    assert_close(item["gps_lat"].as_f64().expect("gps_lat"), 37.86, 0.0001);
    assert_close(item["gps_lng"].as_f64().expect("gps_lng"), -119.54, 0.0001);

    let albums_resp = call_json(&app, Method::GET, "/rpc/v1/albums", None).await;
    assert_eq!(albums_resp["code"], 0);
    let albums = albums_resp["data"].as_array().cloned().unwrap_or_default();
    assert!(
        albums
            .iter()
            .any(|a| a["name"] == "Sample.Album" && a["album_date"] == "2020-02-01"),
        "sample auto album missing: {}",
        albums_resp
    );
}

#[tokio::test]
async fn manual_rescan_should_not_resume_after_previous_success() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    std::fs::create_dir_all(&photos_root).expect("create photos root");
    build_test_image(&photos_root.join("z_existing.jpg"));

    let db_file = tmp.path().join("manual_rescan.db");
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
    cfg.scan.max_concurrent_jobs = 1;
    cfg.scan.task_dispatch_interval_ms = 200;
    let pool_for_assert = pool.clone();
    let app = api::router(pool, cfg);

    let create_source_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/sources",
        Some(json!({
            "name": "manual-rescan",
            "root_path": photos_root.to_string_lossy(),
            "source_type": "local_fs"
        })),
    )
    .await;
    assert_eq!(create_source_resp["code"], 0);
    let source_id = create_source_resp["data"]["id"]
        .as_str()
        .expect("source id")
        .to_string();

    let first_trigger = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(first_trigger["code"], 0);
    let first_job_id = first_trigger["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();
    wait_scan_success(&app, &first_job_id).await;

    let persisted_cursor: Option<String> =
        sqlx::query_scalar("SELECT last_scanned_path FROM source_scan_states WHERE source_id = ?")
            .bind(&source_id)
            .fetch_optional(&pool_for_assert)
            .await
            .expect("query source scan state")
            .flatten();
    assert_eq!(
        persisted_cursor, None,
        "successful scans should not leave a source-level resume cursor"
    );

    build_test_image(&photos_root.join("a_new_before_cursor.jpg"));
    let second_trigger = call_json(
        &app,
        Method::POST,
        &format!("/rpc/v1/sources/{}/scan", source_id),
        Some(json!({})),
    )
    .await;
    assert_eq!(second_trigger["code"], 0);
    let second_job_id = second_trigger["data"]["job_id"]
        .as_str()
        .expect("job id")
        .to_string();
    wait_scan_success(&app, &second_job_id).await;

    let second_job = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/scan-jobs/{}", second_job_id),
        None,
    )
    .await;
    assert_eq!(second_job["code"], 0);
    assert_eq!(second_job["data"]["total_count"], 2);
    assert_eq!(second_job["data"]["new_count"], 1);

    let search_resp = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(search_resp["code"], 0);
    assert_eq!(search_resp["data"]["total"], 2);
    let items = search_resp["data"]["items"]
        .as_array()
        .expect("search items");
    assert!(
        items
            .iter()
            .any(|item| item["file_name"] == "a_new_before_cursor.jpg"),
        "manual rescan skipped the new file sorted before previous cursor: {}",
        search_resp
    );
}

async fn wait_scan_success(app: &axum::Router, job_id: &str) {
    for _ in 0..50 {
        let status_resp = call_json(
            app,
            Method::GET,
            &format!("/rpc/v1/scan-jobs/{}", job_id),
            None,
        )
        .await;
        let status = status_resp["data"]["status"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        if status == "success" {
            return;
        }
        if status == "failed" || status == "cancelled" {
            panic!("scan job ends in {}: {}", status, status_resp);
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!("scan job not completed in time");
}

fn assert_close(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {} within {} of {}, got {}",
        actual,
        tolerance,
        expected,
        actual
    );
}

fn build_test_image(path: &Path) {
    let img = ImageBuffer::from_pixel(640, 360, Rgb([240_u8, 240_u8, 240_u8]));
    img.save(path).expect("save image");
    let ft = FileTime::from_unix_time(1_580_513_600, 0);
    set_file_mtime(path, ft).expect("set mtime");
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
