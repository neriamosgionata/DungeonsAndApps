//! Campaign management and invitation tests
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

// =====================================================================
// Campaign CRUD
// =====================================================================

#[tokio::test]
async fn create_campaign_with_settings() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (s, result) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok),
        Some(json!({
            "name": "Epic Quest",
            "description": "A legendary adventure",
            "leveling": "milestone",
            "starting_level": 3
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["name"], "Epic Quest");
    assert_eq!(result["leveling"], "milestone");
}

#[tokio::test]
async fn get_campaign_by_id() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Test Campaign" }))).await;

    let cid = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "Test Campaign");
}

#[tokio::test]
async fn list_my_campaigns() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Campaign 1" }))).await;

    json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Campaign 2" }))).await;

    let (s, result) = json_req(&router, "GET", "/api/v1/campaigns",
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let camps = result.as_array().expect("should be array");
    assert!(camps.len() >= 2);
}

#[tokio::test]
async fn update_campaign_settings() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Old Name", "leveling": "xp" }))).await;

    let cid = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok),
        Some(json!({ "name": "New Name", "leveling": "milestone" }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "New Name");
    assert_eq!(result["leveling"], "milestone");
}

#[tokio::test]
async fn delete_campaign() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "To Delete" }))).await;

    let cid = created["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok), None).await;

    assert_eq!(s, 200);

    // Verify deleted
    let (s2, _) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok), None).await;
    assert_eq!(s2, 404);
}

// =====================================================================
// Invitations
// =====================================================================

#[tokio::test]
async fn create_invitation_code() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@invite.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Invite Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&tok),
        Some(json!({ "role": "player", "max_uses": 5 }))).await;

    assert_eq!(s, 201);
    assert!(result["code"].is_string(), "should have invitation code");
    assert_eq!(result["role"], "player");
}

#[tokio::test]
async fn join_campaign_with_code() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@invite.test").await;
    let (player_tok, _) = register(&router, "player@invite.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&master_tok), Some(json!({ "name": "Join Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok),
        Some(json!({ "code": code }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["role"], "player");
}

#[tokio::test]
async fn list_campaign_members() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@members.test").await;
    let (player_tok, _) = register(&router, "player@members.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&master_tok), Some(json!({ "name": "Members Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok), None).await;

    assert_eq!(s, 200);
    let members = result.as_array().expect("should be array");
    assert_eq!(members.len(), 2, "should have master and player");
}

#[tokio::test]
async fn update_member_role() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@role.test").await;
    let (player_tok, player_user) = register(&router, "player@role.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&master_tok), Some(json!({ "name": "Role Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    let player_id = player_user["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"),
        Some(&master_tok),
        Some(json!({ "role": "co_master" }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["role"], "co_master");
}

#[tokio::test]
async fn remove_member_from_campaign() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@remove.test").await;
    let (player_tok, player_user) = register(&router, "player@remove.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&master_tok), Some(json!({ "name": "Remove Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    let player_id = player_user["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"),
        Some(&master_tok), None).await;

    assert_eq!(s, 200);
}

#[tokio::test]
async fn player_cannot_create_invitation() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@perms.test").await;
    let (player_tok, _) = register(&router, "player@perms.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&master_tok), Some(json!({ "name": "Perms Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    // Player tries to create invitation - should fail
    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&player_tok), Some(json!({ "role": "player" }))).await;

    assert_eq!(s, 403);
}

// =====================================================================
// Invalid Code Handling
// =====================================================================

#[tokio::test]
async fn invalid_invite_code_fails() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@invalid.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Invalid Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&tok),
        Some(json!({ "code": "INVALID123" }))).await;

    assert_eq!(s, 400);
}

#[tokio::test]
async fn expired_invite_code_fails() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@expired.test").await;

    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns",
        Some(&tok), Some(json!({ "name": "Expired Test" }))).await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&tok),
        Some(json!({ "role": "player", "expires_at": "2000-01-01T00:00:00Z" }))).await;
    let code = invite["code"].as_str().unwrap();

    let (player_tok, _) = register(&router, "player@expired.test").await;

    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok),
        Some(json!({ "code": code }))).await;

    assert_eq!(s, 400);
}
