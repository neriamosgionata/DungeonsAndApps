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

async fn setup(router: &axum::Router) -> (String, String, String, String, String) {
    let (master_tok, master_body) = register(router, "gm@msg.test").await;
    let master_id = master_body["user"]["id"].as_str().unwrap().to_string();
    let (player_tok, player_body) = register_with(router, "pl@msg.test", Some(&master_tok)).await;
    let player_id = player_body["user"]["id"].as_str().unwrap().to_string();
    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Chat Camp" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();
    json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
        Some(json!({ "email": "pl@msg.test", "role": "player" }))).await;
    (master_tok, master_id, player_tok, player_id, cid)
}

#[tokio::test]
async fn member_can_send_campaign_message() {
    let (router, _db) = skip_no_db!();
    let (_mtok, _mid, player_tok, _pid, cid) = setup(&router).await;

    let (s, msg) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok),
        Some(json!({ "scope": "campaign", "body": "Hello world" }))).await;
    assert_eq!(s, 201, "{msg}");
    assert_eq!(msg["body"], "Hello world");
    assert_eq!(msg["scope"], "campaign");
    assert!(msg["id"].is_string());
}

#[tokio::test]
async fn non_member_cannot_send_message() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _mid, _ptok, _pid, cid) = setup(&router).await;
    let (outsider_tok, _) = register_with(&router, "out@msg.test", Some(&master_tok)).await;

    let (s, _) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&outsider_tok),
        Some(json!({ "scope": "campaign", "body": "Hacking in" }))).await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn non_member_cannot_list_messages() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _mid, _ptok, _pid, cid) = setup(&router).await;
    let (outsider_tok, _) = register_with(&router, "out2@msg.test", Some(&master_tok)).await;

    let (s, _) = json_req(&router, "GET", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&outsider_tok), None).await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn whisper_requires_recipient() {
    let (router, _db) = skip_no_db!();
    let (_mtok, _mid, player_tok, _pid, cid) = setup(&router).await;

    let (s, _) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok),
        Some(json!({ "scope": "whisper", "body": "no recipient" }))).await;
    assert_eq!(s, 400);
}

#[tokio::test]
async fn whisper_sent_and_visible_to_both_parties() {
    let (router, _db) = skip_no_db!();
    let (master_tok, master_id, player_tok, player_id, cid) = setup(&router).await;

    let (s, msg) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok),
        Some(json!({ "scope": "whisper", "recipient_id": master_id, "body": "secret" }))).await;
    assert_eq!(s, 201, "{msg}");
    assert_eq!(msg["scope"], "whisper");
    assert_eq!(msg["recipient_id"].as_str().unwrap(), master_id);

    // Player sees their own whisper
    let (_, player_view) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/messages?whispers=true&with_user={master_id}"),
        Some(&player_tok), None).await;
    assert_eq!(player_view.as_array().unwrap().len(), 1);

    // Master sees the whisper thread
    let (_, master_view) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/messages?whispers=true&with_user={player_id}"),
        Some(&master_tok), None).await;
    assert_eq!(master_view.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn whisper_not_visible_to_third_party() {
    let (router, _db) = skip_no_db!();
    let (master_tok, master_id, player_tok, player_id, cid) = setup(&router).await;
    let (p2_tok, p2) = register_with(&router, "p2@msg.test", Some(&master_tok)).await;
    let p2_id = p2["user"]["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
        Some(json!({ "email": "p2@msg.test", "role": "player" }))).await;

    // player → master whisper
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok),
        Some(json!({ "scope": "whisper", "recipient_id": master_id, "body": "secret" }))).await;

    // p2 tries to read the whisper thread between player and master
    let (_, p2_view) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/messages?whispers=true&with_user={player_id}"),
        Some(&p2_tok), None).await;
    assert_eq!(p2_view.as_array().unwrap().len(), 0,
        "third party should see no whispers between other two users");

    let _ = (p2_id, master_id);
}

#[tokio::test]
async fn list_campaign_messages_excludes_whispers() {
    let (router, _db) = skip_no_db!();
    let (master_tok, master_id, player_tok, _pid, cid) = setup(&router).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok), Some(json!({ "scope": "campaign", "body": "public" }))).await;
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&player_tok),
        Some(json!({ "scope": "whisper", "recipient_id": master_id, "body": "private" }))).await;

    let (s, list) = json_req(&router, "GET", &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&master_tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["scope"], "campaign");
}
