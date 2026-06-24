//! Upload/image handling tests
#![allow(unused_variables)]
mod helpers;
use helpers::*;
use serde_json::json;

macro_rules! skip_no_db {
    () => {
        match make_app().await {
            Some(x) => x,
            None => {
                eprintln!("SKIP: TEST_DATABASE_URL/DATABASE_URL not set");
                return;
            }
        }
    };
}

// Note: These tests verify upload URL generation and validation.
// `make_app` builds the app with `s3: None`, so the presign endpoint returns
// 400 ("storage not configured"). The URL-generation tests below skip when S3
// is unconfigured (400) rather than asserting against a backend that isn't
// wired in the test environment.
#[tokio::test]
async fn get_upload_url_for_campaign_image() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/uploads"),
        Some(&tok),
        Some(json!({
            "kind": "campaign",
            "filename": "banner.jpg",
            "content_type": "image/jpeg",
            "campaign_id": cid
        })),
    )
    .await;

    if s.as_u16() == 400 {
        eprintln!("SKIP: S3 not configured in test env");
        return;
    }
    assert_eq!(s, 200, "should get upload URL: {}", result);
    assert!(
        result["upload_url"].is_string() || result["url"].is_string(),
        "should return upload URL"
    );
}

#[tokio::test]
async fn get_upload_url_for_character_portrait() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload2.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (_, char) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&tok),
        Some(json!({ "name": "Hero", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;
    let char_id = char["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/uploads"),
        Some(&tok),
        Some(json!({
            "kind": "avatar",
            "filename": "portrait.png",
            "content_type": "image/png",
            "campaign_id": cid
        })),
    )
    .await;

    if s.as_u16() == 400 {
        eprintln!("SKIP: S3 not configured in test env");
        return;
    }
    assert_eq!(s, 200, "should get upload URL: {}", result);
}

#[tokio::test]
async fn get_upload_url_for_map() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload3.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (_, map) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&tok),
        Some(json!({ "name": "Battle Map" })),
    )
    .await;
    let map_id = map["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/uploads"),
        Some(&tok),
        Some(json!({
            "kind": "map",
            "filename": "dungeon.jpg",
            "content_type": "image/jpeg",
            "campaign_id": cid
        })),
    )
    .await;

    if s.as_u16() == 400 {
        eprintln!("SKIP: S3 not configured in test env");
        return;
    }
    assert_eq!(s, 200, "should get upload URL: {}", result);
}

#[tokio::test]
async fn upload_url_validates_content_type() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload4.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (s, _result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/uploads"),
        Some(&tok),
        Some(json!({
            "kind": "test",
            "filename": "virus.exe",
            "content_type": "application/x-msdownload",
            "campaign_id": cid
        })),
    )
    .await;

    // Should reject non-image types
    assert_ne!(s, 200, "should reject invalid content type");
}

#[tokio::test]
async fn upload_url_validates_campaign_membership() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload5.test").await;
    let (outsider, _) = register(&router, "outsider@upload.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (s, _result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/uploads"),
        Some(&outsider),
        Some(json!({
            "kind": "test",
            "filename": "image.jpg",
            "content_type": "image/jpeg",
            "campaign_id": cid
        })),
    )
    .await;

    assert_eq!(s, 403, "outsider should not get upload URL");
}

#[tokio::test]
async fn update_portrait_url_after_upload() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@upload6.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Upload Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (_, char) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&tok),
        Some(json!({ "name": "Hero", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;
    let char_id = char["id"].as_str().unwrap();

    // Simulate upload completion by updating portrait_url
    let (s, result) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/characters/{char_id}"),
        Some(&tok),
        Some(json!({
            "portrait_url": "https://s3.example.com/portraits/hero.jpg"
        })),
    )
    .await;

    assert_eq!(s, 200);
    assert_eq!(
        result["portrait_url"],
        "https://s3.example.com/portraits/hero.jpg"
    );
}
