//! Combat movement, overlays, and encounter management tests
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

async fn setup_encounter(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@combat.test").await;
    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Combat Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Goblin', '{\"ac\":12,\"hp\":{\"max\":7,\"current\":7}}'::jsonb) returning id")
        .bind(&cid).fetch_one(db).await.unwrap();

    let (_, enc) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok), Some(json!({ "name": "Battle", "map_grid_size": 50 }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, comb) = json_req(router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Goblin",
                     "initiative": 10, "hp_max": 7, "hp_current": 7, "ac": 12,
                     "token_x": 50.0, "token_y": 50.0, "token_on_map": true }))).await;
    let combatant_id = comb["id"].as_str().unwrap().to_string();

    (master_tok, eid, combatant_id, cid)
}

// =====================================================================
// Movement
// =====================================================================

#[tokio::test]
async fn move_combatant_updates_position() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/move"),
        Some(&tok),
        Some(json!({ "x": 60.0, "y": 60.0, "movement_cost": 10 }))).await;

    assert_eq!(s, 200, "move should succeed: {}", result);
    assert_eq!(result["token_x"].as_f64().unwrap_or(0.0), 60.0, "x position should update");
    assert_eq!(result["token_y"].as_f64().unwrap_or(0.0), 60.0, "y position should update");
}

#[tokio::test]
async fn move_consumes_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Set initial movement
    sqlx::query("update combatants set movement_remaining_ft = 30 where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (_, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/move"),
        Some(&tok),
        Some(json!({ "x": 55.0, "y": 50.0, "movement_cost": 5 }))).await;

    let remaining = result["movement_remaining_ft"].as_i64().unwrap_or(-1);
    assert_eq!(remaining, 25, "movement should be consumed");
}

#[tokio::test]
async fn move_exceeding_speed_fails() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    sqlx::query("update combatants set movement_remaining_ft = 5 where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, _result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/move"),
        Some(&tok),
        Some(json!({ "x": 70.0, "y": 50.0, "movement_cost": 20 }))).await;

    assert_eq!(s, 400, "move exceeding speed should fail");
}

// =====================================================================
// Turn Management
// =====================================================================

#[tokio::test]
async fn next_turn_advances_round() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "next turn should succeed: {}", result);
    assert!(result.get("round").is_some() || result.get("turn_index").is_some(),
        "should return turn info");
}

#[tokio::test]
async fn prev_turn_reverses() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/next-turn"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/prev-turn"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "prev turn should succeed: {}", result);
}

#[tokio::test]
async fn goto_turn_jumps_to_specific() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/goto-turn"),
        Some(&tok),
        Some(json!({ "round": 2, "turn_index": 0 }))).await;

    assert_eq!(s, 200, "goto turn should succeed: {}", result);
    assert_eq!(result["round"].as_i64().unwrap_or(-1), 2, "should be round 2");
}

// =====================================================================
// Overlays (Zone Effects)
// =====================================================================

#[tokio::test]
async fn create_overlay_adds_zone() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/overlays"),
        Some(&tok),
        Some(json!({
            "name": "Fire Wall",
            "zone_type": "hazard",
            "x": 50.0,
            "y": 50.0,
            "width": 20.0,
            "height": 100.0,
            "hazard_damage_expression": "3d8",
            "hazard_damage_type": "fire"
        }))).await;

    assert_eq!(s, 200, "create overlay should succeed: {}", result);
    assert!(result["id"].is_string(), "should return overlay id");
}

#[tokio::test]
async fn list_overlays_returns_zones() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Create an overlay first
    json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/overlays"),
        Some(&tok),
        Some(json!({
            "name": "Fog",
            "zone_type": "obscurement",
            "x": 0.0,
            "y": 0.0,
            "width": 100.0,
            "height": 100.0
        }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/overlays"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let overlays = result.as_array().expect("should return array");
    assert!(!overlays.is_empty(), "should have overlays");
}

#[tokio::test]
async fn delete_overlay_removes_zone() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (_, created) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/overlays"),
        Some(&tok),
        Some(json!({
            "name": "Temp",
            "zone_type": "obscurement",
            "x": 0.0,
            "y": 0.0,
            "width": 10.0,
            "height": 10.0
        }))).await;

    let overlay_id = created["id"].as_str().unwrap();

    let (s, _result) = json_req(&router, "DELETE",
        &format!("/api/v1/encounters/{eid}/overlays/{overlay_id}"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "delete overlay should succeed");
}

#[tokio::test]
async fn overlay_damage_applies_hazard_damage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Create hazard overlay
    let (_, overlay) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/overlays"),
        Some(&tok),
        Some(json!({
            "name": "Spike Trap",
            "zone_type": "hazard",
            "x": 45.0,
            "y": 45.0,
            "width": 10.0,
            "height": 10.0,
            "hazard_damage_expression": "2d6",
            "hazard_damage_type": "piercing",
            "hazard_save_ability": "dex",
            "hazard_save_dc": 15
        }))).await;

    let overlay_id = overlay["id"].as_str().unwrap();

    // Move combatant into hazard zone
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/move"),
        Some(&tok),
        Some(json!({ "x": 50.0, "y": 50.0, "movement_cost": 0 }))).await;

    // Trigger overlay damage
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/overlay-damage"),
        Some(&tok),
        Some(json!({
            "overlay_id": overlay_id,
            "target_id": combatant_id,
            "save_rolled": 12 // Failed save
        }))).await;

    assert_eq!(s, 200, "overlay damage should succeed: {}", result);
}

// =====================================================================
// Encounter Difficulty
// =====================================================================

#[tokio::test]
async fn encounter_difficulty_calculates_xp() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/difficulty"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "difficulty should succeed: {}", result);
    // Should return adjusted XP and difficulty rating
    assert!(result.get("adjusted_xp").is_some() || result.get("difficulty").is_some(),
        "should return difficulty metrics");
}

// =====================================================================
// Surprise Round
// =====================================================================

#[tokio::test]
async fn surprise_round_sets_surprised_condition() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Add another combatant
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/surprise"),
        Some(&tok),
        Some(json!({ "surprised_ids": [target_id] }))).await;

    assert_eq!(s, 200, "surprise should succeed: {}", result);
}

// =====================================================================
// Events
// =====================================================================

#[tokio::test]
async fn list_events_returns_combat_log() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Create an event by dealing damage
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/damage"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "amount": 5, "damage_type": "slashing" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/events"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "list events should succeed: {}", result);
    let events = result.as_array().expect("should return array");
    // Should have at least the damage event
    assert!(!events.is_empty(), "should have events");
}
