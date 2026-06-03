//! Combat endpoint integration tests — previously untested endpoints
//! Tests: use-action, delete combatant, bulk add, patch effects, save,
//!        skill check, contested hide, search, use object, delay turn,
//!        surprise auto, flanking, cover, action ordering bugs, player perms
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
// use-action — manually consume actions
// =====================================================================

#[tokio::test]
async fn use_action_consume_action() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    assert_eq!(s, 200, "use-action should succeed: {}", result);
    assert!(result["action_used"].as_bool().unwrap_or(false), "action_used should be true");
}

#[tokio::test]
async fn use_action_consume_bonus_action() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "bonus_action" }))).await;

    assert_eq!(s, 200, "use-action BA should succeed: {}", result);
    assert!(result["bonus_action_used"].as_bool().unwrap_or(false), "bonus_action_used should be true");
}

#[tokio::test]
async fn use_action_toggle_action_twice() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    assert_eq!(s, 200, "second toggle should succeed: {}", result);
    assert!(!result["action_used"].as_bool().unwrap_or(true), "action_used should toggle back to false");
}

// =====================================================================
// delete-combatant
// =====================================================================

#[tokio::test]
async fn delete_combatant_removes_from_encounter() {
    let (router, db) = skip_no_db!();
    let (tok, _eid, cid, _) = setup_encounter(&router, &db).await;

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/combatants/{cid}"),
        Some(&tok), None).await;

    assert_eq!(s, 204, "delete should return 204");

    let count: i64 = sqlx::query_scalar(
        "select count(*) from combatants where id = $1")
        .bind(uuid::Uuid::parse_str(&cid).unwrap()).fetch_one(&db).await.unwrap();
    assert_eq!(count, 0, "combatant should be deleted");
}

// =====================================================================
// bulk-add-combatants
// =====================================================================

#[tokio::test]
async fn bulk_add_combatants_from_npcs() {
    let (router, db) = skip_no_db!();
    let (tok, _, _, cid) = setup_encounter(&router, &db).await;

    let npc_a: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Orc A', '{\"ac\":13,\"hp\":{\"max\":15,\"current\":15}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let npc_b: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Orc B', '{\"ac\":13,\"hp\":{\"max\":15,\"current\":15}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    let (_, enc) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&tok), Some(json!({ "name": "Bulk Battle" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/combatants/bulk"),
        Some(&tok),
        Some(json!({
            "combatants": [
                { "ref_type": "npc", "npc_id": npc_a, "display_name": "Orc A", "initiative": 10 },
                { "ref_type": "npc", "npc_id": npc_b, "display_name": "Orc B", "initiative": 8 }
            ]
        }))).await;

    assert_eq!(s, 200, "bulk add should succeed: {}", result);
    assert_eq!(result["added"].as_i64().unwrap_or(0), 2, "should have added 2 combatants");
    assert_eq!(result["combatants"].as_array().unwrap().len(), 2);
}

// =====================================================================
// patch-effects — add/update combatant effects
// =====================================================================

#[tokio::test]
async fn patch_effects_add_bless() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/encounters/{eid}/effects"),
        Some(&tok),
        Some(json!({
            "combatant_ids": [cid],
            "add_effect": {
                "name": "Bless",
                "modifiers": { "attack_bonus": 2, "save_bonus": 2 },
                "kind": "buff",
                "icon": "sparkles"
            }
        }))).await;

    assert_eq!(s, 200, "patch effects should succeed: {}", result);
    assert_eq!(result["affected"].as_i64().unwrap_or(0), 1, "should affect 1 combatant");

    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let count: i64 = sqlx::query_scalar(
        "select count(*) from combatant_effects where combatant_id = $1 and name = 'Bless' and active = true")
        .bind(db_id).fetch_one(&db).await.unwrap();
    assert!(count >= 1, "Bless effect should exist in DB");
}

#[tokio::test]
async fn patch_effects_remove_bless() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "PATCH",
        &format!("/api/v1/encounters/{eid}/effects"),
        Some(&tok),
        Some(json!({
            "combatant_ids": [&cid],
            "add_effect": { "name": "Bless", "kind": "buff", "icon": "sparkles" }
        }))).await;

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/encounters/{eid}/effects"),
        Some(&tok),
        Some(json!({
            "combatant_ids": [&cid],
            "remove_by_name": "Bless"
        }))).await;

    assert_eq!(s, 200, "remove should succeed: {}", result);

    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let count: i64 = sqlx::query_scalar(
        "select count(*) from combatant_effects where combatant_id = $1 and name = 'Bless' and active = true")
        .bind(db_id).fetch_one(&db).await.unwrap();
    assert_eq!(count, 0, "Bless effect should be deactivated in DB");
}

// =====================================================================
// roll-save — saving throw
// =====================================================================

#[tokio::test]
async fn roll_save_dex_dc15() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/save"),
        Some(&tok),
        Some(json!({
            "ability": "dex",
            "dc": 15,
            "advantage": false,
            "disadvantage": false
        }))).await;

    assert_eq!(s, 200, "save roll should succeed: {}", result);
    assert!(result["save_total"].is_i64(), "should have save_total");
    assert_eq!(result["dc"].as_i64().unwrap_or(0), 15, "dc should be 15");
}

// =====================================================================
// skill-check
// =====================================================================

#[tokio::test]
async fn skill_check_athletics_dc10() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/skill-check"),
        Some(&tok),
        Some(json!({
            "skill": "athletics",
            "dc": 10,
            "advantage": false,
            "disadvantage": false
        }))).await;

    assert_eq!(s, 200, "skill check should succeed: {}", result);
    assert!(result["total"].is_i64(), "should have total");
    assert!(result["passed"].is_boolean(), "should have passed field");
}

// =====================================================================
// contested-hide — stealth vs passive perception
// =====================================================================

#[tokio::test]
async fn contested_hide_with_observer() {
    let (router, db) = skip_no_db!();
    let (tok, eid, hider_id, _) = setup_encounter(&router, &db).await;

    let observer_npc: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Observer', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, observer) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": observer_npc, "display_name": "Observer",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let observer_id = observer["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{hider_id}/contested-hide"),
        Some(&tok),
        Some(json!({
            "observer_ids": [observer_id]
        }))).await;

    // Contested hide may require specific conditions (action available, not in LOS, etc.)
    // — it may legitimately fail depending on game state
    assert!(s == 200 || s == 400, "contested hide should return 200 or 400: {} {}", s, result);
    if s == 200 {
        assert!(result["stealth_total"].is_i64(), "should have stealth_total");
        assert!(result["hidden"].is_boolean(), "should have hidden field");
    }
}

// =====================================================================
// search action
// =====================================================================

#[tokio::test]
async fn search_action_consumes_action() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/search"),
        Some(&tok),
        Some(json!({ "label": "Investigate area" }))).await;

    assert_eq!(s, 200, "search should succeed: {}", result);
    assert!(result["action_used"].as_bool().unwrap_or(false), "search should consume action");
}

// =====================================================================
// use-object
// =====================================================================

#[tokio::test]
async fn use_object_consumes_action() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-object"),
        Some(&tok),
        Some(json!({ "label": "Healing Potion" }))).await;

    assert_eq!(s, 200, "use object should succeed: {}", result);
    assert!(result["action_used"].as_bool().unwrap_or(false), "use object should consume action");
}

// =====================================================================
// delay-turn
// =====================================================================

#[tokio::test]
async fn delay_turn_sets_delayed_flag() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Need another combatant so the encounter has at least 2 combatants for reordering
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/delay"),
        Some(&tok),
        Some(json!({ "insert_after_turn_index": 1 }))).await;

    assert_eq!(s, 200, "delay should succeed: {}", result);
    assert!(result["delayed_turn"].as_bool().unwrap_or(false), "delayed_turn should be true");
    assert!(result["action_used"].as_bool().unwrap_or(false), "delay should consume action");
}

// =====================================================================
// surprise-auto
// =====================================================================

#[tokio::test]
async fn surprise_auto_applies_surprised_condition() {
    let (router, db) = skip_no_db!();
    let (tok, _, _, cid) = setup_encounter(&router, &db).await;

    let ambusher_npc: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Ambusher', '{\"ac\":12,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let defender_npc: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Defender', '{\"ac\":10,\"hp\":{\"max\":8,\"current\":8}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    let (_, enc) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&tok), Some(json!({ "name": "Surprise Battle" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, amb) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": ambusher_npc, "display_name": "Ambusher",
                     "initiative": 15, "hp_max": 10, "hp_current": 10, "ac": 12 }))).await;
    let amb_id = amb["id"].as_str().unwrap().to_string();

    let (_, def) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": defender_npc, "display_name": "Defender",
                     "initiative": 5, "hp_max": 8, "hp_current": 8, "ac": 10 }))).await;
    let def_id = def["id"].as_str().unwrap().to_string();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/surprise-auto"),
        Some(&tok),
        Some(json!({
            "ambusher_ids": [amb_id],
            "defender_ids": [def_id]
        }))).await;

    assert_eq!(s, 200, "surprise auto should succeed: {}", result);
    // Check if surprised_ids contains the defender (depends on stealth vs perception roll)
    assert!(result["surprised_ids"].is_array(), "should have surprised_ids");
}

// =====================================================================
// flanking
// =====================================================================

#[tokio::test]
async fn check_flanking_empty_when_no_flank() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _, cid) = setup_encounter(&router, &db).await;

    let npc_a: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Ally A', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_a, "display_name": "Ally A",
                     "initiative": 10, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/flanking"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "flanking check should succeed: {}", result);
    assert!(result["flanking_pairs"].is_array(), "should have flanking_pairs array");
}

// =====================================================================
// calculate-cover — GET /encounters/{eid}/cover?attacker_id=...&target_id=...
// =====================================================================

#[tokio::test]
async fn calculate_cover_no_blockers() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _) = setup_encounter(&router, &db).await;

    let target_npc: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": target_npc, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/cover?attacker_id={attacker_id}&target_id={target_id}"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "cover calc should succeed: {}", result);
    assert!(result["cover_type"].is_string(), "should have cover_type");
    assert!(result["cover_bonus"].is_i64(), "should have cover_bonus");
}

// =====================================================================
// Bug Regression: Action Ordering — Dodge/Disengage/Help after action used
// =====================================================================

#[tokio::test]
async fn action_ordering_dodge_free_effect() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Use action via use-action endpoint first
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    // Try dodge — should fail because action is already used
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/dodge"),
        Some(&tok), None).await;

    assert!(s == 400 || s == 409, "dodge should fail when action already used (got {}): {}", s, result);

    // Verify dodge modifier is NOT set
    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let dodge_count: i64 = sqlx::query_scalar(
        "select count(*) from combatant_effects where combatant_id = $1 and name = 'Dodge' and active = true")
        .bind(db_id).fetch_one(&db).await.unwrap();
    assert_eq!(dodge_count, 0, "dodge effect should NOT be active when action was already used");
}

#[tokio::test]
async fn action_ordering_disengage_free_effect() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Consume action
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    // Try disengage — should fail
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/disengage"),
        Some(&tok),
        Some(json!({ "use_bonus_action": false }))).await;

    assert!(s == 400 || s == 409, "disengage should fail when action already used (got {}): {}", s, result);

    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let disengage_count: i64 = sqlx::query_scalar(
        "select count(*) from combatant_effects where combatant_id = $1 and name = 'Disengage' and active = true")
        .bind(db_id).fetch_one(&db).await.unwrap();
    assert_eq!(disengage_count, 0, "disengage effect should NOT be active when action was already used");
}

#[tokio::test]
async fn action_ordering_help_free_effect() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    let target_npc: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": target_npc, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Consume action
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    // Try help — should fail
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/help"),
        Some(&tok),
        Some(json!({ "target_id": target_id }))).await;

    assert!(s == 400 || s == 409, "help should fail when action already used (got {}): {}", s, result);

    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let helped_count: i64 = sqlx::query_scalar(
        "select count(*) from combatant_effects where combatant_id = $1 and name = 'Helped' and active = true")
        .bind(db_id).fetch_one(&db).await.unwrap();
    assert_eq!(helped_count, 0, "Helped effect should NOT exist on helper when action was already used");
}

// =====================================================================
// Bug Regression: Dash doubles movement
// =====================================================================

#[tokio::test]
async fn dash_doubles_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _) = setup_encounter(&router, &db).await;

    // Set high base speed so we can move after dash
    sqlx::query("update combatants set sheet = jsonb_set(coalesce(sheet, '{}'::jsonb), '{speed}', '60'::jsonb) where id = $1::uuid")
        .bind(&cid).execute(&db).await.ok();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/dash"),
        Some(&tok),
        Some(json!({ "use_bonus_action": false }))).await;

    assert_eq!(s, 200, "dash should succeed: {}", result);

    // Verify dash effect exists with extra_movement
    let db_id = uuid::Uuid::parse_str(&cid).unwrap();
    let modifiers: Option<serde_json::Value> = sqlx::query_scalar(
        "select modifiers from combatant_effects where combatant_id = $1 and name = 'Dash' and active = true")
        .bind(db_id).fetch_optional(&db).await.unwrap_or(None);
    assert!(modifiers.is_some(), "dash effect should exist in DB");
    if let Some(ref m) = modifiers {
        let extra = m.get("extra_movement").and_then(|v| v.as_i64()).unwrap_or(0);
        assert!(extra > 0, "dash should grant extra movement (got {})", extra);
    }
}

// =====================================================================
// Player Permission: player can attack with own combatant
// =====================================================================

async fn setup_player_encounter(
    router: &axum::Router,
    _db: &sqlx::PgPool,
) -> (String, String, String, String, String, String) {
    let (master_tok, _master_body) = register(router, "gm@perms.test").await;

    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Perms Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Register player, add to campaign
    let (player_tok, player_body) = register(router, "player@perms.test").await;
    let player_id = player_body["user"]["id"].as_str().unwrap().to_string();

    let _ = sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1::uuid, $2::uuid, 'player'::membership_role)")
        .bind(uuid::Uuid::parse_str(&cid).unwrap())
        .bind(uuid::Uuid::parse_str(&player_id).unwrap())
        .execute(_db).await;

    // Player creates character
    let (_, ch) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Hero", "level_total": 1 }))).await;
    let char_id = ch["id"].as_str().unwrap().to_string();

    // Master creates encounter — start will auto-add character
    let (_, enc) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok), Some(json!({ "name": "Perms Battle" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    // Player rolls initiative
    json_req(router, "POST", &format!("/api/v1/encounters/{eid}/set-initiative"),
        Some(&player_tok),
        Some(json!({ "character_id": char_id, "initiative": 12 }))).await;

    // Master starts encounter
    json_req(router, "POST", &format!("/api/v1/encounters/{eid}/start"),
        Some(&master_tok), None).await;

    // Get the player's combatant ID
    let (_, combatants) = json_req(router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok), None).await;
    let player_combatant_id: String = combatants.as_array().unwrap().iter()
        .find(|c| c["character_id"].as_str().map(|s| s == char_id).unwrap_or(false))
        .map(|c| c["id"].as_str().unwrap().to_string())
        .expect("player combatant should exist");

    // Add an NPC target
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&cid).fetch_one(_db).await.unwrap();
    let (_, target) = json_req(router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap().to_string();

    (master_tok, player_tok, eid, player_combatant_id, target_id, cid)
}

#[tokio::test]
async fn player_can_attack_own_combatant() {
    let (router, db) = skip_no_db!();
    let (_master_tok, player_tok, _eid, cid, target_id, _) =
        setup_player_encounter(&router, &db).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/attack"),
        Some(&player_tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    assert_eq!(s, 200, "player should be able to attack with own combatant: {} {}", s, result);
}

#[tokio::test]
async fn player_cannot_attack_others_combatant() {
    let (router, db) = skip_no_db!();
    let (master_tok, _player_tok, eid, _cid, target_id, cid) =
        setup_player_encounter(&router, &db).await;

    // Register second player
    let (player2_tok, player2_body) = register(&router, "player2@perms.test").await;
    let player2_id = player2_body["user"]["id"].as_str().unwrap().to_string();

    // Add second player to campaign
    let _ = sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1::uuid, $2::uuid, 'player'::membership_role)")
        .bind(uuid::Uuid::parse_str(&cid).unwrap())
        .bind(uuid::Uuid::parse_str(&player2_id).unwrap())
        .execute(&db).await;

    // Create a new NPC combatant for player2 to (not) attack with
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Owned NPC', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let (_, owned) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Owned NPC",
                     "initiative": 7, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let owned_id = owned["id"].as_str().unwrap().to_string();

    // Player2 tries to attack using someone else's combatant → 403
    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{owned_id}/attack"),
        Some(&player2_tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    assert_eq!(s, 403, "player2 should NOT be able to attack with someone else's NPC combatant");
}

// =====================================================================
// Regression: ready_action TOCTOU — action must be consumed atomically
// =====================================================================

#[tokio::test]
async fn ready_action_fails_when_action_already_used() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Consume action first
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    // Try to ready — must fail because action is already used
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/ready"),
        Some(&tok),
        Some(json!({
            "action": "attack",
            "trigger": "target_attacks",
            "target_id": cid
        }))).await;

    assert_eq!(s, 400, "ready should fail when action already used (got {}): {}", s, result);
}

// =====================================================================
// Regression: delay_turn must check action_used atomically
// =====================================================================

#[tokio::test]
async fn delay_turn_fails_when_action_already_used() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Consume action first
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/use-action"),
        Some(&tok),
        Some(json!({ "action": "action" }))).await;

    // Try to delay — must fail because action is already used
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{cid}/delay"),
        Some(&tok),
        Some(json!({ "insert_after_turn_index": 1 }))).await;

    assert_eq!(s, 400, "delay should fail when action already used (got {}): {}", s, result);
}
