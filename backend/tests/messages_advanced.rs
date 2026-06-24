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

/// Returns (master_tok, master_id, player_tok, player_id, cid).
async fn setup_campaign_with_members(
    router: &axum::Router,
) -> (String, String, String, String, String) {
    let (master_tok, master_body) = register(router, "gm@msg.test").await;
    let master_id = master_body["user"]["id"].as_str().unwrap().to_string();
    let (player_tok, player_body) = register(router, "player@msg.test").await;
    let player_id = player_body["user"]["id"].as_str().unwrap().to_string();

    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Message Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    add_member_via_invite(
        router,
        &master_tok,
        &player_tok,
        "player@msg.test",
        &cid,
        "player",
    )
    .await;

    (master_tok, master_id, player_tok, player_id, cid)
}

// =====================================================================
// Send Messages
// =====================================================================

#[tokio::test]
async fn send_public_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "body": "Hello everyone!",
            "scope": "campaign"
        })),
    )
    .await;

    assert_eq!(s, 201);
    assert_eq!(result["body"], "Hello everyone!");
    assert_eq!(result["scope"], "campaign");
}

#[tokio::test]
async fn send_private_whisper() {
    let (router, _db) = skip_no_db!();
    let (_master, master_id, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "body": "Secret to GM",
            "scope": "whisper",
            "recipient_id": master_id
        })),
    )
    .await;

    assert_eq!(s, 201);
    assert_eq!(result["scope"], "whisper");
}

#[tokio::test]
async fn send_roll_in_chat() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "body": "/roll 1d20+5",
            "scope": "campaign",
            "is_roll": true
        })),
    )
    .await;

    assert_eq!(s, 201);
    let roll = &result["roll_result"];
    assert!(roll.is_object(), "roll should be evaluated server-side");
    let total = roll["total"].as_i64().expect("total");
    assert!((6..=25).contains(&total), "1d20+5 total in range, got {total}");
    assert_eq!(roll["expression"], "1d20+5");
}

// =====================================================================
// List Messages
// =====================================================================

#[tokio::test]
async fn list_messages_paginated() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    for i in 0..5 {
        json_req(
            &router,
            "POST",
            &format!("/api/v1/campaigns/{cid}/messages"),
            Some(&player),
            Some(json!({ "body": format!("Message {}", i), "scope": "campaign" })),
        )
        .await;
    }

    let (s, result) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/messages?limit=3"),
        Some(&player),
        None,
    )
    .await;

    assert_eq!(s, 200);
    let messages = result.as_array().expect("should be array");
    assert!(messages.len() <= 3, "should respect limit");
}

#[tokio::test]
async fn list_messages_with_cursor() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "First", "scope": "campaign" })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "Second", "scope": "campaign" })),
    )
    .await;

    let (s, _result) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/messages?limit=1"),
        Some(&player),
        None,
    )
    .await;

    assert_eq!(s, 200);
}

// =====================================================================
// Edit/Delete Messages
// =====================================================================

#[tokio::test]
async fn edit_own_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (_, created) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "Original", "scope": "campaign" })),
    )
    .await;

    let msg_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/messages/{msg_id}"),
        Some(&player),
        Some(json!({ "body": "Edited" })),
    )
    .await;

    assert_eq!(s, 200);
    assert_eq!(result["body"], "Edited");
    assert!(
        result["edited_at"].is_string(),
        "should have edited timestamp"
    );
}

#[tokio::test]
async fn cannot_edit_others_message() {
    let (router, _db) = skip_no_db!();
    let (master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (gm2_tok, _) = register(&router, "gm2@edit.test").await;
    add_member_via_invite(&router, &master, &gm2_tok, "gm2@edit.test", &cid, "master").await;

    let (_, msg) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&gm2_tok),
        Some(json!({ "body": "GM Message", "scope": "campaign" })),
    )
    .await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/messages/{msg_id}"),
        Some(&player),
        Some(json!({ "body": "Hacked" })),
    )
    .await;

    assert_eq!(s, 403);
}

#[tokio::test]
async fn delete_own_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (_, created) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "To Delete", "scope": "campaign" })),
    )
    .await;

    let msg_id = created["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/messages/{msg_id}"),
        Some(&player),
        None,
    )
    .await;

    assert_eq!(s, 204);
}

// =====================================================================
// Reactions
// =====================================================================

#[tokio::test]
async fn add_reaction_to_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (_, msg) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "React to this", "scope": "campaign" })),
    )
    .await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/messages/{msg_id}/reactions"),
        Some(&player),
        Some(json!({ "emoji": "👍" })),
    )
    .await;

    assert_eq!(s, 200);
    let reactions = result["reactions"].as_array().expect("reactions array");
    assert_eq!(reactions.len(), 1);
    assert_eq!(reactions[0]["emoji"], "👍");
    assert_eq!(reactions[0]["count"], 1);

    // Removing the reaction empties the group list.
    let (s2, result2) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/messages/{msg_id}/reactions"),
        Some(&player),
        Some(json!({ "emoji": "👍" })),
    )
    .await;
    assert_eq!(s2, 200);
    assert_eq!(result2["reactions"].as_array().unwrap().len(), 0);
}

// =====================================================================
// Permissions
// =====================================================================

#[tokio::test]
async fn non_member_cannot_send_message() {
    let (router, _db) = skip_no_db!();
    let (_master, _mid, _player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (outsider, _) = register(&router, "outsider@msg.test").await;

    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&outsider),
        Some(json!({ "body": "Intruder!", "scope": "campaign" })),
    )
    .await;

    assert_eq!(s, 403);
}

#[tokio::test]
async fn master_can_delete_any_message() {
    let (router, _db) = skip_no_db!();
    let (master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    let (_, msg) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({ "body": "Player message", "scope": "campaign" })),
    )
    .await;

    let msg_id = msg["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/messages/{msg_id}"),
        Some(&master),
        None,
    )
    .await;

    assert_eq!(s, 204);
}

// =====================================================================
// Notifications
// =====================================================================

#[tokio::test]
async fn mention_creates_notification() {
    let (router, _db) = skip_no_db!();
    let (master, _mid, player, _pid, cid) = setup_campaign_with_members(&router).await;

    // Master's display_name derives from "gm@msg.test" → "gm".
    let (s, _result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player),
        Some(json!({
            "body": "@gm Look at this!",
            "scope": "campaign"
        })),
    )
    .await;

    assert_eq!(s, 201);

    // The mentioned master should have a chat.mention notification.
    let (ns, notifs) = json_req(
        &router,
        "GET",
        "/api/v1/notifications",
        Some(&master),
        None,
    )
    .await;
    assert_eq!(ns, 200);
    let arr = notifs.as_array().expect("notifications array");
    assert!(
        arr.iter().any(|n| n["kind"] == "chat.mention"),
        "expected a chat.mention notification, got {arr:?}"
    );
}
