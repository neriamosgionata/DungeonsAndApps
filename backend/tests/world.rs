mod helpers;
use helpers::*;
use serde_json::json;

macro_rules! skip_no_db {
    () => {
        match make_app().await {
            Some(x) => x,
            None => return,
        }
    };
}

async fn setup_two_users(router: &axum::Router) -> (String, String, String) {
    let (master_tok, _) = register(router, "gm@w.com").await;
    let (player_tok, _) = register_with(router, "pl@w.com", Some(&master_tok)).await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "World" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();
    add_member_via_invite(router, &master_tok, &player_tok, "pl@w.com", &cid, "player").await;
    (master_tok, player_tok, cid)
}

#[tokio::test]
async fn recap_visibility_and_rbac() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    // master creates private session
    let (s, sess) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        Some(json!({ "title": "Session 1", "recap": "gm notes", "visibility": "master" })),
    )
    .await;
    assert_eq!(s, 201);
    let sid = sess["id"].as_str().unwrap();

    // player cannot list it
    let (_, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 0);

    // master flips to players
    let (_, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/sessions/{sid}"),
        Some(&mtok),
        Some(json!({ "visibility": "players" })),
    )
    .await;

    // player now sees it
    let (_, list2) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(list2.as_array().unwrap().len(), 1);

    // player cannot delete
    let (s2, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/sessions/{sid}"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s2, 403);
}

#[tokio::test]
async fn factions_npcs_visibility() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    let (_, f) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&mtok),
        Some(json!({ "name": "Harpers", "visibility": "players" })),
    )
    .await;
    let fid = f["id"].as_str().unwrap();

    let (_, n) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&mtok),
        Some(json!({ "name": "Volo", "faction_id": fid, "visibility": "master" })),
    )
    .await;
    let nid = n["id"].as_str().unwrap();

    // player sees faction, not npc
    let (_, fl) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(fl.as_array().unwrap().len(), 1);
    let (_, nl) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(nl.as_array().unwrap().len(), 0);
    let (s, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/npcs/{nid}"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s, 403);

    // master flips npc to players
    json_req(
        &router,
        "PATCH",
        &format!("/api/v1/npcs/{nid}"),
        Some(&mtok),
        Some(json!({ "visibility": "players" })),
    )
    .await;
    let (_, nl2) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(nl2.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn maps_and_pins() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    let (_, m) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&mtok),
        Some(json!({ "name": "Faerûn", "visibility": "players" })),
    )
    .await;
    let mid = m["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "POST", &format!("/api/v1/maps/{mid}/pins"), Some(&mtok),
        Some(json!({ "label": "Party", "kind": "group", "x": 10.0, "y": 20.0, "is_party": true, "visibility": "players" }))).await;
    assert_eq!(s, 201);

    // player can list, not create
    let (_, pins) = json_req(
        &router,
        "GET",
        &format!("/api/v1/maps/{mid}/pins"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(pins.as_array().unwrap().len(), 1);
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/maps/{mid}/pins"),
        Some(&ptok),
        Some(json!({ "label": "X", "kind": "note", "x": 0.0, "y": 0.0 })),
    )
    .await;
    assert_eq!(s2, 403);
}

#[tokio::test]
async fn group_party_loot_quests() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    // party auto-created on campaign create
    let (_, party) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/party"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(party["gp"], 0);

    // purse is GM-only (treasury): player PATCH is forbidden
    let (s_forbid, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/campaigns/{cid}/party"),
        Some(&ptok),
        Some(json!({ "gp": 150 })),
    )
    .await;
    assert_eq!(s_forbid, 403);

    // master updates coins
    let (s, party2) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/campaigns/{cid}/party"),
        Some(&mtok),
        Some(json!({ "gp": 150, "shared_notes": "split fairly" })),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(party2["gp"], 150);

    // loot
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&ptok),
        Some(json!({ "name": "Sword +1", "quantity": 1, "value_gp": 1000.0 })),
    )
    .await;
    assert_eq!(s2, 201);

    // quests — master only create
    let (s3, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Save the town", "visibility": "players" })),
    )
    .await;
    assert_eq!(s3, 201);
    assert_eq!(q["status"], "active");
    let (s4, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&ptok),
        Some(json!({ "title": "Nope" })),
    )
    .await;
    assert_eq!(s4, 403);
}

#[tokio::test]
async fn whisper_between_any_two_members() {
    let (router, _) = skip_no_db!();
    let (master_tok, master_body) = register(&router, "gm@wh.com").await;
    let (p1_tok, p1) = register_with(&router, "p1@wh.com", Some(&master_tok)).await;
    let (p2_tok, p2) = register_with(&router, "p2@wh.com", Some(&master_tok)).await;
    let p1_id = p1["user"]["id"].as_str().unwrap().to_string();
    let p2_id = p2["user"]["id"].as_str().unwrap().to_string();
    let m_id = master_body["user"]["id"].as_str().unwrap().to_string();

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Whispers" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();
    add_member_via_invite(&router, &master_tok, &p1_tok, "p1@wh.com", &cid, "player").await;
    add_member_via_invite(&router, &master_tok, &p2_tok, "p2@wh.com", &cid, "player").await;

    // player → master
    let (s1, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&p1_tok),
        Some(json!({ "scope": "whisper", "recipient_id": m_id, "body": "pssh" })),
    )
    .await;
    assert_eq!(s1, 201);

    // player → player (master not involved)
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&p1_tok),
        Some(json!({ "scope": "whisper", "recipient_id": p2_id, "body": "yo" })),
    )
    .await;
    assert_eq!(s2, 201);

    // master → player
    let (s3, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&master_tok),
        Some(json!({ "scope": "whisper", "recipient_id": p1_id, "body": "secret quest" })),
    )
    .await;
    assert_eq!(s3, 201);

    // p2 sees the p1↔p2 whisper thread
    let (_, thread) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/messages?whispers=true&with_user={p1_id}"),
        Some(&p2_tok),
        None,
    )
    .await;
    assert_eq!(thread.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn messages_campaign_and_whispers() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;
    let (_, me_m) = json_req(&router, "GET", "/api/v1/auth/me", Some(&mtok), None).await;
    let mid = me_m["id"].as_str().unwrap().to_string();
    let (_, me_p) = json_req(&router, "GET", "/api/v1/auth/me", Some(&ptok), None).await;
    let pid = me_p["id"].as_str().unwrap().to_string();

    // campaign chat
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&ptok),
        Some(json!({ "scope": "campaign", "body": "hello party" })),
    )
    .await;
    assert_eq!(s, 201);

    // whisper from player to master
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&ptok),
        Some(json!({ "scope": "whisper", "recipient_id": mid, "body": "psst" })),
    )
    .await;
    assert_eq!(s2, 201);

    // whisper needs recipient
    let (s3, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&ptok),
        Some(json!({ "scope": "whisper", "body": "no recipient" })),
    )
    .await;
    assert_eq!(s3, 400);

    // campaign chat list
    let (_, chat) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/messages"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(chat.as_array().unwrap().len(), 1);

    // whispers list involving master
    let (_, wh) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/messages?whispers=true&with_user={pid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(wh.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn maps_pins_sql_injection_prevention() {
    let (router, _db) = skip_no_db!();
    let (mtok, _ptok, cid) = setup_two_users(&router).await;

    // Create a map
    let (s, map) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&mtok),
        Some(json!({ "name": "Test Map" })),
    )
    .await;
    assert_eq!(s, 201);
    let map_id = map["id"].as_str().unwrap();

    // Create a pin
    let (s, pin) = json_req(
        &router,
        "POST",
        &format!("/api/v1/maps/{map_id}/pins"),
        Some(&mtok),
        Some(json!({ "label": "Test Pin", "kind": "note", "x": 100.0, "y": 200.0 })),
    )
    .await;
    assert_eq!(s, 201);
    let pin_id = pin["id"].as_str().unwrap();

    // Normal query - should work
    let (s, pins) = json_req(
        &router,
        "GET",
        &format!("/api/v1/maps/{map_id}/pins"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(pins.as_array().unwrap().len(), 1);

    // Test SQL injection in visibility filter parameter (percent-encoded so the
    // URI is valid; the handler binds params, never interpolates them).
    // These should NOT cause SQL errors - they should be safely handled.
    let malicious_queries = [
        format!("/api/v1/maps/{map_id}/pins?visibility=all%27%20OR%20%271%27%3D%271"),
        format!("/api/v1/maps/{map_id}/pins?visibility=all%27%3B%20DROP%20TABLE%20pins%3B%20--"),
        format!("/api/v1/maps/{map_id}/pins?visibility=all%22%20UNION%20SELECT%20*%20FROM%20users%20--"),
    ];

    for query in &malicious_queries {
        let (s, _body) = json_req(&router, "GET", query, Some(&mtok), None).await;
        // Should either return 200 (with safe fallback to 'all') or 400 (bad request)
        // but NOT 500 (server error which would indicate SQL injection worked)
        assert!(
            s == 200 || s == 400,
            "SQL injection attempt should return 200 or 400, got {}: {}",
            s,
            query
        );
    }

    // Verify the pin still exists and wasn't affected
    let (s, pins) = json_req(
        &router,
        "GET",
        &format!("/api/v1/maps/{map_id}/pins"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(pins.as_array().unwrap().len(), 1);
    assert_eq!(pins[0]["id"].as_str().unwrap(), pin_id);
}

// =====================================================================
// Recap (sessions) CRUD
// =====================================================================

#[tokio::test]
async fn recap_create_list_player_visibility() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    // Master creates private session
    let (s, sess) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        Some(json!({ "title": "Session Zero", "recap": "notes", "visibility": "master" })),
    )
    .await;
    assert_eq!(s, 201, "{sess}");
    let sid = sess["id"].as_str().unwrap();
    assert_eq!(sess["visibility"], "master");

    // Player cannot list master-only sessions
    let (_, player_list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(player_list.as_array().unwrap().len(), 0);

    // Master can see all
    let (_, master_list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(master_list.as_array().unwrap().len(), 1);

    // Flip to players
    let (s_patch, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/sessions/{sid}"),
        Some(&mtok),
        Some(json!({ "visibility": "players" })),
    )
    .await;
    assert_eq!(s_patch, 200);

    let (_, player_list2) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(player_list2.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn recap_delete_master_only() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup_two_users(&router).await;

    let (_, sess) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        Some(json!({ "title": "Killable Session", "visibility": "players" })),
    )
    .await;
    let sid = sess["id"].as_str().unwrap();

    // Player cannot delete
    let (s_forbid, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/sessions/{sid}"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s_forbid, 403);

    // Master can delete
    let (s_ok, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/sessions/{sid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_ok, 204);

    let (s_gone, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/sessions/{sid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_gone, 404);
}

#[tokio::test]
async fn recap_create_multiple_sessions_ordered() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup_two_users(&router).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        Some(json!({ "title": "Session 1", "session_number": 1, "visibility": "players" })),
    )
    .await;
    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        Some(json!({ "title": "Session 2", "session_number": 2, "visibility": "players" })),
    )
    .await;

    let (s, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/sessions"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(list.as_array().unwrap().len(), 2);
}

// =====================================================================
// Invitations
// =====================================================================

#[tokio::test]
async fn invitation_master_creates_player_accepts() {
    let (router, _) = skip_no_db!();
    let (master_tok, _, _, _) = bootstrap_two(&router, "gm@inv.test", "pl@inv.test").await;
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Invite Camp" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Master creates invitation for pl@inv.test
    let (s, inv) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok),
        Some(json!({ "email": "pl@inv.test", "role": "player" })),
    )
    .await;
    assert_eq!(s, 201, "{inv}");
    let inv_id = inv["id"].as_str().unwrap();

    // Player retrieves their pending invitation
    let (player_tok, _) = register_with(&router, "pl2@inv.test", Some(&master_tok)).await;
    let (_, mine) = json_req(
        &router,
        "GET",
        "/api/v1/invitations",
        Some(&master_tok),
        None,
    )
    .await;
    let _ = mine;

    // The invited player (pl@inv.test) accepts
    let (pl_login_s, pl_login) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "pl@inv.test", "password": helpers::TEST_PASSWORD })),
    )
    .await;
    assert_eq!(pl_login_s, 200, "{pl_login}");
    let pl_tok = pl_login["token"].as_str().unwrap();

    let (s_accept, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(pl_tok),
        None,
    )
    .await;
    assert_eq!(s_accept, 204);

    // Player is now a member
    let (s_view, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(pl_tok),
        None,
    )
    .await;
    assert_eq!(s_view, 200);

    let _ = player_tok;
}

#[tokio::test]
async fn invitation_duplicate_redemption_fails() {
    let (router, _) = skip_no_db!();
    let (master_tok, _, _, _) = bootstrap_two(&router, "gm@inv2.test", "pl@inv2.test").await;
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Dup Invite Camp" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let (_, inv) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok),
        Some(json!({ "email": "pl@inv2.test", "role": "player" })),
    )
    .await;
    let inv_id = inv["id"].as_str().unwrap();

    let (pl_s, pl_login) = json_req(
        &router,
        "POST",
        "/api/v1/auth/login",
        None,
        Some(json!({ "email": "pl@inv2.test", "password": helpers::TEST_PASSWORD })),
    )
    .await;
    assert_eq!(pl_s, 200);
    let pl_tok = pl_login["token"].as_str().unwrap();

    // First accept
    let (s1, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(pl_tok),
        None,
    )
    .await;
    assert_eq!(s1, 204);

    // Second accept of same invitation → 404 (responded_at is set, no longer pending)
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(pl_tok),
        None,
    )
    .await;
    assert_eq!(s2, 404);
}

#[tokio::test]
async fn invitation_non_target_cannot_accept() {
    let (router, _) = skip_no_db!();
    let (master_tok, _, other_tok, _) =
        bootstrap_two(&router, "gm@inv3.test", "pl@inv3.test").await;
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Steal Invite" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Create a third user to invite
    let (_, third) = register_with(&router, "third@inv3.test", Some(&master_tok)).await;
    let _ = third;

    let (_, inv) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok),
        Some(json!({ "email": "third@inv3.test", "role": "player" })),
    )
    .await;
    let inv_id = inv["id"].as_str().unwrap();

    // pl@inv3.test tries to accept an invitation meant for third@inv3.test
    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/invitations/{inv_id}/accept"),
        Some(&other_tok),
        None,
    )
    .await;
    assert_eq!(s, 403);
}
