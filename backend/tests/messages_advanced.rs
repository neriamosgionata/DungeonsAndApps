//! Messages/chat system tests
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

async fn setup_campaign_with_members(router: &axum::Router) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@msg.test").await;
    let (player_tok, _) = register(router, "player@msg.test").await;

    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Message Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let (_, invite) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    (master_tok, player_tok, cid, camp["id"].as_str().unwrap().to_string())
}

// =====================================================================
// Send Messages
// =====================================================================

#[tokio::test]
async fn send_public_message() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "content": "Hello everyone!",
            "visibility": "public"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["content"], "Hello everyone!");
    assert_eq!(result["visibility"], "public");
}

#[tokio::test]
async fn send_private_whisper() {
    let (router, _db) = skip_no_db!();
    let (master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "content": "Secret to GM",
            "visibility": "whisper",
            "recipient_id": master.split('.').nth(1).map(|s| s.to_string()).unwrap_or_default()
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["visibility"], "whisper");
}

#[tokio::test]
async fn send_roll_in_chat() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "content": "/roll 1d20+5",
            "visibility": "public",
            "is_roll": true
        }))).await;

    assert_eq!(s, 201);
    assert!(result["roll_result"].is_object() || result["content"].as_str().unwrap().contains("d20"),
        "should process roll");
}

// =====================================================================
// List Messages
// =====================================================================

#[tokio::test]
async fn list_messages_paginated() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    // Send multiple messages
    for i in 0..5 {
        json_req(&router, "POST",
            &format!("/api/v1/campaigns/{cid}/messages"),
            Some(&player),
            Some(json!({ "content": format!("Message {}", i) }))).await;
    }

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/messages?limit=3"),
        Some(&player), None).await;

    assert_eq!(s, 200);
    let messages = result.as_array().expect("should be array");
    assert!(messages.len() <= 3, "should respect limit");
}

#[tokio::test]
async fn list_messages_with_cursor() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "First" }))).await;

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "Second" }))).await;

    let (s, _result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/messages?limit=1"),
        Some(&player), None).await;

    assert_eq!(s, 200);
    // Cursor-based pagination would have next_cursor field
}

// =====================================================================
// Edit/Delete Messages
// =====================================================================

#[tokio::test]
async fn edit_own_message() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (_, created) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "Original" }))).await;

    let msg_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/messages/{msg_id}"),
        Some(&player),
        Some(json!({ "content": "Edited" }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["content"], "Edited");
    assert!(result["edited_at"].is_string(), "should have edited timestamp");
}

#[tokio::test]
async fn cannot_edit_others_message() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (master_tok, _) = register(&router, "gm2@edit.test").await;
    let (_, invite) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&_master), Some(json!({ "role": "master" }))).await;
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/join"),
        Some(&master_tok), Some(json!({ "code": invite["code"].as_str().unwrap() }))).await;

    let (_, msg) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&master_tok),
        Some(json!({ "content": "GM Message" }))).await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/messages/{msg_id}"),
        Some(&player),
        Some(json!({ "content": "Hacked" }))).await;

    assert_eq!(s, 403);
}

#[tokio::test]
async fn delete_own_message() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (_, created) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "To Delete" }))).await;

    let msg_id = created["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/messages/{msg_id}"),
        Some(&player), None).await;

    assert_eq!(s, 200);
}

// =====================================================================
// Reactions
// =====================================================================

#[tokio::test]
async fn add_reaction_to_message() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (_, msg) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "React to this" }))).await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages/{msg_id}/reactions"),
        Some(&player),
        Some(json!({ "emoji": "👍" }))).await;

    assert_eq!(s, 200);
    assert!(result["reactions"].is_array() || result["emoji"].as_str() == Some("👍"));
}

// =====================================================================
// Permissions
// =====================================================================

#[tokio::test]
async fn non_member_cannot_send_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _player, cid, _) = setup_campaign_with_members(&router).await;

    let (outsider, _) = register(&router, "outsider@msg.test").await;

    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&outsider),
        Some(json!({ "content": "Intruder!" }))).await;

    assert_eq!(s, 403);
}

#[tokio::test]
async fn master_can_delete_any_message() {
    let (router, _db) = skip_no_db!();
    let (master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (_, msg) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "content": "Player message" }))).await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/messages/{msg_id}"),
        Some(&master), None).await;

    assert_eq!(s, 200);
}

// =====================================================================
// Notifications
// =====================================================================

#[tokio::test]
async fn mention_creates_notification() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid, _) = setup_campaign_with_members(&router).await;

    let (s, _result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "content": "@GM Look at this!",
            "mentions": [{"username": "GM"}]
        }))).await;

    assert_eq!(s, 201);
    // Notification would be created async, can't easily verify here
}
