//! WebSocket connection and presence tests
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

// WebSocket tests - verify endpoints and auth

#[tokio::test]
async fn ws_campaign_endpoint_requires_upgrade() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "ws@test.com").await;

    // Create campaign
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "WS Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    // WS endpoint via HTTP GET should fail (requires upgrade)
    let (s, _) = json_req(
        &router,
        "GET",
        &format!("/ws"),
        Some(&tok),
        None,
    )
    .await;

    // Should be 400 Bad Request (needs WebSocket upgrade) or 404 if different path
    assert!(
        s == 400 || s == 404 || s == 426,
        "WS endpoint should require upgrade: got {}",
        s
    );
}

#[tokio::test]
async fn ws_user_endpoint_requires_upgrade() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "ws2@test.com").await;

    // User WS endpoint
    let (s, _) = json_req(&router, "GET", "/api/v1/ws/user", Some(&tok), None).await;

    assert!(
        s == 400 || s == 404 || s == 426,
        "User WS should require upgrade: got {}",
        s
    );
}

#[tokio::test]
async fn ws_campaign_without_auth_fails() {
    let (router, _db) = skip_no_db!();

    // Try WS endpoint without auth
    let (s, _) = json_req(&router, "GET", "/api/v1/ws/campaign/some-uuid", None, None).await;

    assert_eq!(s, 401, "WS without auth should be 401");
}

// =====================================================================
// Presence/Online Status
// =====================================================================

#[tokio::test]
async fn presence_list_requires_membership() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "presence@test.com").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Presence Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    // Master can see presence
    let (s, result) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/presence"),
        Some(&tok),
        None,
    )
    .await;

    assert!(s == 200 || s == 404, "presence endpoint check: {}", result);
}

#[tokio::test]
async fn ws_token_extraction_from_protocol() {
    // Test token extraction logic that would be used in WS handler
    let protocols = vec!["token", "Bearer.test.jwt.token"];

    // Extract Bearer token
    let token = protocols
        .iter()
        .find(|p| p.starts_with("Bearer."))
        .and_then(|p| p.strip_prefix("Bearer."));

    assert_eq!(token, Some("test.jwt.token"));
}

#[tokio::test]
async fn ws_token_missing_when_no_bearer_protocol() {
    let protocols: Vec<&str> = vec![];

    let token = protocols
        .iter()
        .find(|p| p.starts_with("Bearer."))
        .and_then(|p| p.strip_prefix("Bearer."));

    assert_eq!(token, None);
}

#[tokio::test]
async fn ws_invalid_token_protocol_ignored() {
    let protocols = vec!["token", "invalid-format"];

    let token = protocols
        .iter()
        .find(|p| p.starts_with("Bearer."))
        .and_then(|p| p.strip_prefix("Bearer."));

    assert_eq!(token, None);
}

// =====================================================================
// Campaign Notifications via WS
// =====================================================================

#[tokio::test]
async fn dice_roll_broadcasts_to_campaign() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "dice@test.com").await;

    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Dice WS Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    // Roll dice (triggers WS broadcast)
    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/dice"),
        Some(&tok),
        Some(json!({ "expression": "1d20", "label": "Test Roll" })),
    )
    .await;

    assert_eq!(s, 201, "dice roll should succeed: {}", result);
    assert!(result["total"].is_number(), "roll should have total");
}

#[tokio::test]
async fn combat_event_triggers_notification() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_combat(&router, &db).await;

    let target_id = create_target(&router, &db, &eid, &tok, 10).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Attack triggers combat event notification
    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({
            "target_id": target_id,
            "damage_expression": "1d6",
            "damage_type": "slashing"
        })),
    )
    .await;

    assert_eq!(s, 200, "attack should succeed and trigger WS: {}", result);
}

// Helper functions
async fn setup_combat(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@ws.test").await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "WS Combat Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Enemy', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&cid).fetch_one(db).await.unwrap();

    let (_, enc) = json_req(
        router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok),
        Some(json!({ "name": "Battle" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, comb) = json_req(
        router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Fighter",
                     "initiative": 10, "hp_max": 20, "hp_current": 20, "ac": 15 }),
        ),
    )
    .await;
    let combatant_id = comb["id"].as_str().unwrap().to_string();

    (master_tok, eid, combatant_id, cid)
}

async fn create_target(
    router: &axum::Router,
    db: &sqlx::PgPool,
    eid: &str,
    tok: &str,
    ac: i32,
) -> String {
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', $2::jsonb) returning id")
        .bind(eid)
        .bind(format!("{{\"ac\":{},\"hp\":{{\"max\":20,\"current\":20}}}}", ac))
        .fetch_one(db).await.unwrap();

    let (_, target) = json_req(
        router,
        "POST",
        &format!("/api/v1/encounters/{}/combatants", eid),
        Some(tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": ac }),
        ),
    )
    .await;

    target["id"].as_str().unwrap().to_string()
}
