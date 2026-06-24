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

async fn setup(router: &axum::Router) -> (String, String, String) {
    let (master_tok, _) = register(router, "gm@notif.test").await;
    let (player_tok, _) = register_with(router, "pl@notif.test", Some(&master_tok)).await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "NotifCamp" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();
    add_member_via_invite(
        router,
        &master_tok,
        &player_tok,
        "pl@notif.test",
        &cid,
        "player",
    )
    .await;
    (master_tok, player_tok, cid)
}

// =====================================================================
// Notification List — Empty
// =====================================================================

#[tokio::test]
async fn notifications_list_empty() {
    let (router, _) = skip_no_db!();
    // Fresh user with no campaign involvement has zero notifications.
    let (tok, _) = register(&router, "lonely@notif.test").await;

    let (s, body) = json_req(&router, "GET", "/api/v1/notifications", Some(&tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(body.as_array().unwrap().len(), 0);
}

// =====================================================================
// Notifications Generated on Combat Turn
// =====================================================================

#[tokio::test]
async fn notifications_generated_on_combat_turn() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    // Player creates a character
    let (_, ch) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&ptok),
        Some(json!({ "name": "Aela", "race": "Human", "level_total": 3 })),
    )
    .await;
    let char_id = ch["id"].as_str().unwrap();

    // Master creates an encounter
    let (_, enc) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&mtok),
        Some(json!({ "name": "Battle" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap();

    // Master adds the player's character as a combatant, rolled
    let _ = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "character",
            "character_id": char_id,
            "display_name": "Aela",
            "initiative": 18,
            "hp_current": 30,
            "hp_max": 30,
            "ac": 16,
            "initiative_rolled": true
        })),
    )
    .await;

    // Master adds an NPC combatant
    let _ = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "npc",
            "display_name": "Goblin",
            "initiative": 10,
            "hp_current": 7,
            "hp_max": 7,
            "ac": 13
        })),
    )
    .await;

    // Start the encounter — triggers notify_turn for the first combatant
    let (s_start, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_start, 200);

    // Advance turn — triggers notify_turn for the next combatant
    let (s_next, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_next, 200);

    // Check player notifications — should have at least one "your_turn" notification
    let (s_list, body) = json_req(&router, "GET", "/api/v1/notifications", Some(&ptok), None).await;
    assert_eq!(s_list, 200);
    let notifs = body.as_array().unwrap();
    assert!(
        notifs.len() >= 1,
        "expected at least 1 notification, got {:?}",
        notifs
    );

    let your_turn = notifs.iter().find(|n| n["kind"] == "combat.your_turn");
    assert!(
        your_turn.is_some(),
        "expected combat.your_turn notification, got {:?}",
        notifs
    );
    assert_eq!(your_turn.unwrap()["title"], "It's your turn!");
}

// =====================================================================
// Mark One Read
// =====================================================================

#[tokio::test]
async fn notifications_mark_read() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    // Generate a notification by starting combat
    let (_, ch) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&ptok),
        Some(json!({ "name": "Bael", "race": "Elf", "level_total": 2 })),
    )
    .await;
    let char_id = ch["id"].as_str().unwrap();

    let (_, enc) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&mtok),
        Some(json!({ "name": "Mark Read" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "character",
            "character_id": char_id,
            "display_name": "Bael",
            "initiative": 15,
            "hp_current": 20,
            "hp_max": 20,
            "ac": 14,
            "initiative_rolled": true
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "npc",
            "display_name": "Bandit",
            "initiative": 8,
            "hp_current": 10,
            "hp_max": 10,
            "ac": 12
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&mtok),
        None,
    )
    .await;

    // Get notification ID
    let (_, list) = json_req(&router, "GET", "/api/v1/notifications", Some(&ptok), None).await;
    let notifs = list.as_array().unwrap();
    assert!(notifs.len() >= 1);
    let nid = notifs[0]["id"].as_str().unwrap();

    // Mark as read
    let (s_mark, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/notifications/{nid}/read"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s_mark, 204);

    // Verify read_at is set
    let (_, list2) = json_req(&router, "GET", "/api/v1/notifications", Some(&ptok), None).await;
    let marked = &list2.as_array().unwrap()[0];
    assert!(!marked["read_at"].is_null(), "read_at should be set");
}

// =====================================================================
// Mark All Read
// =====================================================================

#[tokio::test]
async fn notifications_mark_all_read() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    let (_, ch) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&ptok),
        Some(json!({ "name": "Cael", "race": "Dwarf", "level_total": 2 })),
    )
    .await;
    let char_id = ch["id"].as_str().unwrap();

    let (_, enc) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&mtok),
        Some(json!({ "name": "Mark All" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "character",
            "character_id": char_id,
            "display_name": "Cael",
            "initiative": 20,
            "hp_current": 25,
            "hp_max": 25,
            "ac": 18,
            "initiative_rolled": true
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "npc",
            "display_name": "Orc",
            "initiative": 5,
            "hp_current": 15,
            "hp_max": 15,
            "ac": 13
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&mtok),
        None,
    )
    .await;

    // Advance a couple turns to generate multiple notifications
    let _ = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"),
        Some(&mtok),
        None,
    )
    .await;
    let _ = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"),
        Some(&mtok),
        None,
    )
    .await;

    // Verify we have unread notifications
    let (_, before) = json_req(
        &router,
        "GET",
        "/api/v1/notifications?unread_only=true",
        Some(&ptok),
        None,
    )
    .await;
    assert!(before.as_array().unwrap().len() >= 1);

    // Mark all read
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/notifications/read-all",
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s, 204);

    // Verify none unread
    let (_, after) = json_req(
        &router,
        "GET",
        "/api/v1/notifications?unread_only=true",
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(after.as_array().unwrap().len(), 0);
}

// =====================================================================
// Unread-Only Filtering
// =====================================================================

#[tokio::test]
async fn notifications_unread_only_filter() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    let (_, ch) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&ptok),
        Some(json!({ "name": "Dael", "race": "Tiefling", "level_total": 1 })),
    )
    .await;
    let char_id = ch["id"].as_str().unwrap();

    let (_, enc) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&mtok),
        Some(json!({ "name": "Filter" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "character",
            "character_id": char_id,
            "display_name": "Dael",
            "initiative": 10,
            "hp_current": 15,
            "hp_max": 15,
            "ac": 12,
            "initiative_rolled": true
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&mtok),
        Some(json!({
            "ref_type": "npc",
            "display_name": "Skeleton",
            "initiative": 3,
            "hp_current": 8,
            "hp_max": 8,
            "ac": 10
        })),
    )
    .await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&mtok),
        None,
    )
    .await;

    // All notifications should be unread
    let (_, all) = json_req(&router, "GET", "/api/v1/notifications", Some(&ptok), None).await;
    let total = all.as_array().unwrap().len();

    let (_, unread) = json_req(
        &router,
        "GET",
        "/api/v1/notifications?unread_only=true",
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(unread.as_array().unwrap().len(), total);

    // Mark first as read
    let nid = all[0]["id"].as_str().unwrap();
    json_req(
        &router,
        "POST",
        &format!("/api/v1/notifications/{nid}/read"),
        Some(&ptok),
        None,
    )
    .await;

    let (_, unread2) = json_req(
        &router,
        "GET",
        "/api/v1/notifications?unread_only=true",
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(unread2.as_array().unwrap().len(), total - 1);
}
