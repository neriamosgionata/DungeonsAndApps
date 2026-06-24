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

    let (s, result) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({
            "name": "Epic Quest",
            "description": "A legendary adventure",
            "leveling": "milestone",
            "starting_level": 3
        })),
    )
    .await;

    assert_eq!(s, 201);
    assert_eq!(result["name"], "Epic Quest");
    assert_eq!(result["leveling"], "milestone");
}

#[tokio::test]
async fn get_campaign_by_id() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Test Campaign" })),
    )
    .await;

    let cid = created["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "Test Campaign");
}

#[tokio::test]
async fn list_my_campaigns() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Campaign 1" })),
    )
    .await;

    json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Campaign 2" })),
    )
    .await;

    let (s, result) = json_req(&router, "GET", "/api/v1/campaigns", Some(&tok), None).await;

    assert_eq!(s, 200);
    let camps = result.as_array().expect("should be array");
    assert!(camps.len() >= 2);
}

#[tokio::test]
async fn update_campaign_settings() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Old Name", "leveling": "xp" })),
    )
    .await;

    let cid = created["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok),
        Some(json!({ "name": "New Name", "leveling": "milestone" })),
    )
    .await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "New Name");
    assert_eq!(result["leveling"], "milestone");
}

#[tokio::test]
async fn delete_campaign() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@campaign.test").await;

    let (_, created) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "To Delete" })),
    )
    .await;

    let cid = created["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(s, 204);

    // Verify deleted: membership is cascade-removed, so the now-orphaned owner
    // is treated as a non-member → 403 (the API does not leak existence as 404).
    let (s2, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&tok),
        None,
    )
    .await;
    assert_eq!(s2, 403);
}

// =====================================================================
// Invitations
// =====================================================================

#[tokio::test]
async fn create_invitation_code() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@invite.test").await;
    let (_player_tok, _) = register(&router, "invitee@invite.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Invite Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&tok),
        Some(json!({ "email": "invitee@invite.test", "role": "player" })),
    )
    .await;

    assert_eq!(s, 201, "{result}");
    assert!(result["id"].is_string(), "should have invitation id");
    assert_eq!(result["role"], "player");
}

#[tokio::test]
async fn join_campaign_with_code() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@invite.test").await;
    let (player_tok, _) = register(&router, "player@invite.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Join Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    add_member_via_invite(
        &router,
        &master_tok,
        &player_tok,
        "player@invite.test",
        cid,
        "player",
    )
    .await;

    // The accepted player is now a campaign member.
    let (s, members) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert!(
        members.as_array().unwrap().iter().any(|m| m["role"] == "player"),
        "joined player should appear with role player"
    );
}

#[tokio::test]
async fn list_campaign_members() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@members.test").await;
    let (player_tok, _) = register(&router, "player@members.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Members Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    add_member_via_invite(
        &router,
        &master_tok,
        &player_tok,
        "player@members.test",
        cid,
        "player",
    )
    .await;

    let (s, result) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok),
        None,
    )
    .await;

    assert_eq!(s, 200);
    let members = result.as_array().expect("should be array");
    assert_eq!(members.len(), 2, "should have master and player");
}

#[tokio::test]
async fn update_member_role() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@role.test").await;
    let (player_tok, player_user) = register(&router, "player@role.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Role Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    add_member_via_invite(
        &router,
        &master_tok,
        &player_tok,
        "player@role.test",
        cid,
        "player",
    )
    .await;

    let player_id = player_user["user"]["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"),
        Some(&master_tok),
        Some(json!({ "role": "master" })),
    )
    .await;

    assert_eq!(s, 200);
    assert_eq!(result["role"], "master");
}

#[tokio::test]
async fn remove_member_from_campaign() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@remove.test").await;
    let (player_tok, player_user) = register(&router, "player@remove.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Remove Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    add_member_via_invite(
        &router,
        &master_tok,
        &player_tok,
        "player@remove.test",
        cid,
        "player",
    )
    .await;

    let player_id = player_user["user"]["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"),
        Some(&master_tok),
        None,
    )
    .await;

    assert_eq!(s, 204);
}

#[tokio::test]
async fn player_cannot_create_invitation() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm@perms.test").await;
    let (player_tok, _) = register(&router, "player@perms.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Perms Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    add_member_via_invite(
        &router,
        &master_tok,
        &player_tok,
        "player@perms.test",
        cid,
        "player",
    )
    .await;

    // Player (member) tries to invite someone else - should be forbidden
    let (outsider_tok, _) = register(&router, "outsider@perms.test").await;
    let _ = outsider_tok;
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&player_tok),
        Some(json!({ "email": "outsider@perms.test", "role": "player" })),
    )
    .await;

    assert_eq!(s, 403);
}

// =====================================================================
// Invalid Code Handling
// =====================================================================

#[tokio::test]
async fn invalid_invite_code_fails() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@invalid.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Invalid Test" })),
    )
    .await;
    let _cid = camp["id"].as_str().unwrap();

    // Accepting a non-existent invitation id → 404 (the code+/join flow is gone;
    // invitations are accepted by id via POST /invitations/{id}/accept).
    let bogus = uuid::Uuid::new_v4();
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{bogus}/accept"),
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(s, 404);
}

#[tokio::test]
async fn already_responded_invite_fails() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@expired.test").await;
    let (player_tok, _) = register(&router, "player@expired.test").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Expired Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    let (_, invite) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&tok),
        Some(json!({ "email": "player@expired.test", "role": "player" })),
    )
    .await;
    let inv_id = invite["id"].as_str().unwrap();

    // First accept succeeds
    let (s1, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s1, 204);

    // Second accept of the same invitation → 404 (responded_at is set)
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s2, 404);
}
