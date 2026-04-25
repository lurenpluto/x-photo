mod common;

use std::collections::HashMap;

use axum::http::{Method, StatusCode, header};
use common::{
    build_test_app, build_test_app_with_config, build_test_image, call_http, call_json,
    create_source, trigger_scan, wait_scan_success,
};
use serde_json::{Value, json};
use tempfile::TempDir;

#[tokio::test]
async fn rescan_should_soft_delete_missing_files_and_repair_related_views() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    let album_dir = photos_root.join("2024.01.01.Delete.Sync");
    std::fs::create_dir_all(&album_dir).expect("create album dir");

    let removed_path = album_dir.join("IMG_REMOVED.jpg");
    let kept_path = album_dir.join("IMG_KEPT.jpg");
    build_test_image(&removed_path);
    build_test_image(&kept_path);

    let test_app = build_test_app(tmp.path(), "delete_sync.db").await;
    let app = test_app.app;

    let source_id = create_source(&app, "delete-sync", &photos_root).await;
    let first_job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &first_job_id).await;

    let photos = search_photo_ids_by_name(&app).await;
    let removed_id = photos["IMG_REMOVED.jpg"].clone();
    let kept_id = photos["IMG_KEPT.jpg"].clone();

    favorite_photo(&app, &removed_id).await;
    let album_id = create_manual_album(&app, "Deletion Repair Album", Some(&removed_id)).await;
    add_photos_to_album(&app, &album_id, &[kept_id.as_str()], 1).await;

    std::fs::remove_file(&removed_path).expect("remove source file");
    let second_job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &second_job_id).await;

    let second_job = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/scan-jobs/{second_job_id}"),
        None,
    )
    .await;
    assert_eq!(second_job["code"], 0);
    assert!(
        second_job["data"]["updated_count"].as_i64().unwrap_or(0) >= 1,
        "delete sync should be counted as an update: {second_job}"
    );

    let search = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(search["code"], 0);
    assert_eq!(search["data"]["total"], 1);
    assert_eq!(search["data"]["items"][0]["file_name"], "IMG_KEPT.jpg");

    let deleted_detail = call_json(
        &app,
        Method::GET,
        &format!("/rpc/v1/photos/{removed_id}"),
        None,
    )
    .await;
    assert_ne!(deleted_detail["code"], 0);

    let favorites = call_json(
        &app,
        Method::GET,
        "/rpc/v1/photos/favorites?page=1&page_size=20",
        None,
    )
    .await;
    assert_eq!(favorites["code"], 0);
    assert_eq!(favorites["data"]["total"], 0);

    let album = album_detail(&app, &album_id, 1, 20).await;
    assert_eq!(album["data"]["photos"]["total"], 1);
    assert_eq!(album["data"]["album"]["cover_photo_id"], kept_id);
}

#[tokio::test]
async fn photo_file_and_thumbnail_endpoints_should_serve_image_bytes() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    std::fs::create_dir_all(&photos_root).expect("create photos root");
    build_test_image(&photos_root.join("IMG_BINARY.jpg"));

    let test_app = build_test_app_with_config(tmp.path(), "binary_endpoints.db", |cfg| {
        cfg.preview_cache.dir = tmp
            .path()
            .join("preview-cache")
            .to_string_lossy()
            .to_string();
    })
    .await;
    let app = test_app.app;

    let source_id = create_source(&app, "binary-endpoints", &photos_root).await;
    let job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &job_id).await;
    let photo_id = search_photo_ids_by_name(&app).await["IMG_BINARY.jpg"].clone();

    let original = call_http(
        &app,
        Method::GET,
        &format!("/rpc/v1/photos/{photo_id}/file"),
        None,
    )
    .await;
    assert_eq!(original.status, StatusCode::OK);
    assert_header_contains(&original, header::CONTENT_TYPE.as_str(), "image/jpeg");
    assert_jpeg(&original.body);

    let thumb = call_http(
        &app,
        Method::GET,
        &format!("/rpc/v1/photos/{photo_id}/thumb?max_edge=128"),
        None,
    )
    .await;
    assert_eq!(thumb.status, StatusCode::OK);
    assert_header_contains(&thumb, header::CONTENT_TYPE.as_str(), "image/jpeg");
    assert_jpeg(&thumb.body);
}

#[tokio::test]
async fn favorite_list_should_page_and_order_by_favorite_time() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    std::fs::create_dir_all(&photos_root).expect("create photos root");
    for name in ["IMG_FAV_A.jpg", "IMG_FAV_B.jpg", "IMG_FAV_C.jpg"] {
        build_test_image(&photos_root.join(name));
    }

    let test_app = build_test_app(tmp.path(), "favorite_paging.db").await;
    let app = test_app.app;
    let pool = test_app.pool;

    let source_id = create_source(&app, "favorite-paging", &photos_root).await;
    let job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &job_id).await;
    let photos = search_photo_ids_by_name(&app).await;

    for name in ["IMG_FAV_A.jpg", "IMG_FAV_B.jpg", "IMG_FAV_C.jpg"] {
        favorite_photo(&app, &photos[name]).await;
    }
    for (name, ts) in [
        ("IMG_FAV_A.jpg", "2026-04-25T10:00:00+00:00"),
        ("IMG_FAV_B.jpg", "2026-04-25T10:01:00+00:00"),
        ("IMG_FAV_C.jpg", "2026-04-25T10:02:00+00:00"),
    ] {
        sqlx::query("UPDATE photo_favorites SET created_at = ? WHERE photo_id = ?")
            .bind(ts)
            .bind(&photos[name])
            .execute(&pool)
            .await
            .expect("update favorite timestamp");
    }

    let first_page = call_json(
        &app,
        Method::GET,
        "/rpc/v1/photos/favorites?page=1&page_size=2",
        None,
    )
    .await;
    assert_eq!(first_page["code"], 0);
    assert_eq!(first_page["data"]["total"], 3);
    assert_eq!(
        favorite_file_names(&first_page),
        vec!["IMG_FAV_C.jpg", "IMG_FAV_B.jpg"]
    );

    let second_page = call_json(
        &app,
        Method::GET,
        "/rpc/v1/photos/favorites?page=2&page_size=2",
        None,
    )
    .await;
    assert_eq!(second_page["code"], 0);
    assert_eq!(favorite_file_names(&second_page), vec!["IMG_FAV_A.jpg"]);
}

#[tokio::test]
async fn manual_album_workflow_should_update_membership_cover_and_search_index() {
    let tmp = TempDir::new().expect("temp dir");
    let photos_root = tmp.path().join("photos");
    std::fs::create_dir_all(&photos_root).expect("create photos root");
    for name in ["IMG_ALBUM_A.jpg", "IMG_ALBUM_B.jpg"] {
        build_test_image(&photos_root.join(name));
    }

    let test_app = build_test_app(tmp.path(), "manual_album.db").await;
    let app = test_app.app;

    let source_id = create_source(&app, "manual-album", &photos_root).await;
    let job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &job_id).await;
    let photos = search_photo_ids_by_name(&app).await;
    let photo_a = photos["IMG_ALBUM_A.jpg"].clone();
    let photo_b = photos["IMG_ALBUM_B.jpg"].clone();

    let album_id = create_manual_album(&app, "Manual Album", None).await;
    add_photos_to_album(&app, &album_id, &[photo_a.as_str(), photo_b.as_str()], 2).await;

    let detail = album_detail(&app, &album_id, 1, 20).await;
    assert_eq!(detail["data"]["photos"]["total"], 2);
    assert_eq!(detail["data"]["album"]["cover_photo_id"], photo_a);

    let cover = call_json(
        &app,
        Method::PATCH,
        &format!("/rpc/v1/albums/{album_id}/cover"),
        Some(json!({"cover_photo_id": photo_b})),
    )
    .await;
    assert_eq!(cover["code"], 0);

    let update = call_json(
        &app,
        Method::PATCH,
        &format!("/rpc/v1/albums/{album_id}"),
        Some(json!({"name": "Renamed Manual Album", "remark": "curated"})),
    )
    .await;
    assert_eq!(update["code"], 0);

    let search_by_album = call_json(
        &app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"keyword": "album:Renamed", "page": 1, "page_size": 20})),
    )
    .await;
    assert_eq!(search_by_album["code"], 0);
    assert_eq!(search_by_album["data"]["total"], 2);

    remove_photos_from_album(&app, &album_id, &[photo_b.as_str()], 1).await;
    let after_remove_cover = album_detail(&app, &album_id, 1, 20).await;
    assert_eq!(after_remove_cover["data"]["photos"]["total"], 1);
    assert_eq!(
        after_remove_cover["data"]["album"]["cover_photo_id"],
        photo_a
    );

    remove_photos_from_album(&app, &album_id, &[photo_a.as_str()], 1).await;
    let empty_album = album_detail(&app, &album_id, 1, 20).await;
    assert_eq!(empty_album["data"]["photos"]["total"], 0);
    assert!(empty_album["data"]["album"]["cover_photo_id"].is_null());

    let missing_album = call_json(
        &app,
        Method::POST,
        "/rpc/v1/albums/not-exists/photos:add",
        Some(json!({"photo_ids": [photo_a]})),
    )
    .await;
    assert_ne!(missing_album["code"], 0);
}

async fn search_photo_ids_by_name(app: &axum::Router) -> HashMap<String, String> {
    let resp = call_json(
        app,
        Method::POST,
        "/rpc/v1/photos/search",
        Some(json!({"page": 1, "page_size": 100})),
    )
    .await;
    assert_eq!(resp["code"], 0);
    resp["data"]["items"]
        .as_array()
        .expect("photo items")
        .iter()
        .map(|item| {
            (
                item["file_name"].as_str().expect("file name").to_string(),
                item["id"].as_str().expect("photo id").to_string(),
            )
        })
        .collect()
}

async fn favorite_photo(app: &axum::Router, photo_id: &str) {
    let resp = call_json(
        app,
        Method::PATCH,
        &format!("/rpc/v1/photos/{photo_id}/favorite"),
        Some(json!({"favorite": true})),
    )
    .await;
    assert_eq!(resp["code"], 0);
}

async fn create_manual_album(
    app: &axum::Router,
    name: &str,
    cover_photo_id: Option<&str>,
) -> String {
    let mut body = json!({"name": name, "remark": "manual test album"});
    if let Some(cover_photo_id) = cover_photo_id {
        body["cover_photo_id"] = json!(cover_photo_id);
    }
    let resp = call_json(app, Method::POST, "/rpc/v1/albums", Some(body)).await;
    assert_eq!(resp["code"], 0);
    resp["data"]["id"].as_str().expect("album id").to_string()
}

async fn add_photos_to_album(
    app: &axum::Router,
    album_id: &str,
    photo_ids: &[&str],
    expected_affected: i64,
) {
    let resp = call_json(
        app,
        Method::POST,
        &format!("/rpc/v1/albums/{album_id}/photos:add"),
        Some(json!({"photo_ids": photo_ids})),
    )
    .await;
    assert_eq!(resp["code"], 0);
    assert_eq!(resp["data"]["affected"], expected_affected);
}

async fn remove_photos_from_album(
    app: &axum::Router,
    album_id: &str,
    photo_ids: &[&str],
    expected_affected: i64,
) {
    let resp = call_json(
        app,
        Method::POST,
        &format!("/rpc/v1/albums/{album_id}/photos:remove"),
        Some(json!({"photo_ids": photo_ids})),
    )
    .await;
    assert_eq!(resp["code"], 0);
    assert_eq!(resp["data"]["affected"], expected_affected);
}

async fn album_detail(app: &axum::Router, album_id: &str, page: i64, page_size: i64) -> Value {
    let resp = call_json(
        app,
        Method::GET,
        &format!("/rpc/v1/albums/{album_id}?page={page}&page_size={page_size}"),
        None,
    )
    .await;
    assert_eq!(resp["code"], 0);
    resp
}

fn favorite_file_names(resp: &Value) -> Vec<&str> {
    resp["data"]["items"]
        .as_array()
        .expect("favorite items")
        .iter()
        .map(|item| item["file_name"].as_str().expect("favorite file name"))
        .collect()
}

fn assert_header_contains(resp: &common::TestHttpResponse, name: &str, expected: &str) {
    let value = resp
        .headers
        .get(name)
        .unwrap_or_else(|| panic!("missing response header {name}"))
        .to_str()
        .expect("header string");
    assert!(
        value.contains(expected),
        "expected header {name} to contain {expected}, got {value}"
    );
}

fn assert_jpeg(bytes: &[u8]) {
    assert!(bytes.len() > 16, "jpeg body should not be empty");
    assert_eq!(&bytes[0..2], &[0xFF, 0xD8], "jpeg magic mismatch");
}
