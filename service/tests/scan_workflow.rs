use std::path::Path;

use axum::body::{to_bytes, Body};
use axum::http::{Method, Request};
use filetime::{set_file_mtime, FileTime};
use image::{ImageBuffer, Rgb};
use serde_json::{json, Value};
use service::{api, config::AppConfig, db};
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;
use tokio::time::{sleep, Duration};
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
