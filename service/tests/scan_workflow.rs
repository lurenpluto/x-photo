mod common;

use axum::http::Method;
use common::{
    assert_close, build_test_app, build_test_image, call_json, create_source, generate_sample_data,
    trigger_scan, wait_scan_success,
};
use serde_json::json;
use tempfile::TempDir;

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

    let test_app = build_test_app(tmp.path(), "test.db").await;
    let app = test_app.app;
    let pool_for_test = test_app.pool;

    let source_id = create_source(&app, "local-test", &photos_root).await;
    let job_id = trigger_scan(&app, &source_id).await;
    wait_scan_success(&app, &job_id).await;

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
    let test_album = albums
        .iter()
        .find(|a| a["name"] == "Test.Album")
        .expect("dot-rule auto album");
    assert_eq!(test_album["photo_count"], 1);
    let another_album = albums
        .iter()
        .find(|a| a["name"] == "Another_Album")
        .expect("underscore-rule auto album");
    assert_eq!(another_album["photo_count"], 1);
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
    let manifest_path = generate_sample_data(&photos_root);

    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest_path).expect("read manifest"))
            .expect("parse manifest");
    assert_eq!(manifest["strategy"], "sample");
    assert_eq!(manifest["total_photos"], 1);

    let test_app = build_test_app(tmp.path(), "generated_sample.db").await;
    let app = test_app.app;
    let source_id = create_source(&app, "generated-sample", &photos_root).await;
    let job_id = trigger_scan(&app, &source_id).await;

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
        albums.iter().any(|a| a["name"] == "Sample.Album"
            && a["album_date"] == "2020-02-01"
            && a["photo_count"] == 1),
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

    let test_app = build_test_app(tmp.path(), "manual_rescan.db").await;
    let app = test_app.app;
    let pool_for_assert = test_app.pool;

    let source_id = create_source(&app, "manual-rescan", &photos_root).await;
    let first_job_id = trigger_scan(&app, &source_id).await;
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
    let second_job_id = trigger_scan(&app, &source_id).await;
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
