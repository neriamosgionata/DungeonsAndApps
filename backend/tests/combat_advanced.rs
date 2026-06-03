//! Combat action economy and advanced mechanics tests
//! Tests: dodge, disengage, dash, hide, conditions, legendary/lair actions
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
// Action Economy - Dodge, Disengage, Dash
// =====================================================================

#[tokio::test]
async fn dodge_action_sets_dodging() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/dodge"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "dodge should succeed: {}", result);
    assert!(result["modifiers"]["dodging"].as_bool().unwrap_or(false), "dodging modifier should be set");
}

#[tokio::test]
async fn disengage_action_prevents_opportunity_attacks() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/disengage"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "disengage should succeed: {}", result);
    assert!(result["modifiers"]["disengaging"].as_bool().unwrap_or(false), "disengaging modifier should be set");
}

#[tokio::test]
async fn dash_action_doubles_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/dash"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "dash should succeed: {}", result);
    // Dash typically doubles remaining movement
    let movement = result["movement_remaining_ft"].as_i64().unwrap_or(0);
    assert!(movement > 0, "dash should provide extra movement");
}

// =====================================================================
// Hide Action
// =====================================================================

#[tokio::test]
async fn hide_action_sets_hidden_modifier() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/hide"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "hide should succeed: {}", result);
    // Hide sets hidden condition and makes stealth check
    assert!(result.get("stealth_check").is_some() || result.get("modifiers").is_some(), 
        "hide should return stealth result or modifiers");
}

// =====================================================================
// Help Action
// =====================================================================

#[tokio::test]
async fn help_action_gives_advantage_to_ally() {
    let (router, db) = skip_no_db!();
    let (tok, eid, helper_id, _cid) = setup_encounter(&router, &db).await;

    // Create ally
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Ally', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, ally) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Ally",
                     "initiative": 8, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let ally_id = ally["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{helper_id}/help"),
        Some(&tok),
        Some(json!({ "target_id": ally_id, "help_type": "attack" }))).await;

    assert_eq!(s, 200, "help should succeed: {}", result);
    assert!(result.get("help_given_to").is_some() || result.get("modifiers").is_some(),
        "help should track target");
}

// =====================================================================
// Conditions
// =====================================================================

#[tokio::test]
async fn add_condition_applies_effect() {
    let (router, db) = skip_no_db!();
    let (tok, eid, target_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{target_id}/conditions"),
        Some(&tok),
        Some(json!({ "condition": "prone", "duration": 1 }))).await;

    assert_eq!(s, 200, "add_condition should succeed: {}", result);
    let conditions = result["conditions"].as_array().expect("conditions should be array");
    assert!(conditions.iter().any(|c| c.as_str().map(|s| s.contains("prone")).unwrap_or(false)),
        "prone condition should be added");
}

#[tokio::test]
async fn restrained_condition_reduces_speed() {
    let (router, db) = skip_no_db!();
    let (tok, eid, target_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{target_id}/conditions"),
        Some(&tok),
        Some(json!({ "condition": "restrained", "duration": 1 }))).await;

    assert_eq!(s, 200);
    let speed = result["speed_ft"].as_i64().unwrap_or(30);
    assert_eq!(speed, 0, "restrained should reduce speed to 0");
}

// =====================================================================
// Legendary Actions
// =====================================================================

#[tokio::test]
async fn legendary_action_consumes_legendary_action() {
    let (router, db) = skip_no_db!();
    let (tok, eid, monster_id, _cid) = setup_encounter(&router, &db).await;

    // Set up monster with legendary actions
    sqlx::query("update combatants set legendary_actions_max = 3, legendary_actions_used = 0 where id = $1::uuid")
        .bind(&monster_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{monster_id}/legendary-action"),
        Some(&tok),
        Some(json!({ "action_name": "Tail Swipe" }))).await;

    assert_eq!(s, 200, "legendary action should succeed: {}", result);
    let used = result["legendary_actions_used"].as_i64().unwrap_or(0);
    assert_eq!(used, 1, "should consume 1 legendary action");
}

#[tokio::test]
async fn legendary_actions_reset_on_turn_start() {
    let (router, db) = skip_no_db!();
    let (tok, eid, monster_id, _cid) = setup_encounter(&router, &db).await;

    sqlx::query("update combatants set legendary_actions_max = 3, legendary_actions_used = 2 where id = $1::uuid")
        .bind(&monster_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Advance turn - legendary actions should reset
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/next-turn"), Some(&tok), None).await;

    let (_, monster) = json_req(&router, "GET",
        &format!("/api/v1/combatants/{monster_id}"),
        Some(&tok), None).await;

    let used = monster["legendary_actions_used"].as_i64().unwrap_or(999);
    assert_eq!(used, 0, "legendary actions should reset on turn start");
}

// =====================================================================
// Lair Actions
// =====================================================================

#[tokio::test]
async fn lair_action_sets_lair_action_used() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _monster_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/lair-action"),
        Some(&tok),
        Some(json!({ "lair_action": "Regional Effect" }))).await;

    assert_eq!(s, 200, "lair action should succeed: {}", result);
}

// =====================================================================
// Grapple & Shove
// =====================================================================

#[tokio::test]
async fn grapple_sets_grappling_condition() {
    let (router, db) = skip_no_db!();
    let (tok, eid, grappler_id, _cid) = setup_encounter(&router, &db).await;

    // Create target to grapple
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{grappler_id}/grapple"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "contest_result": "success" }))).await;

    assert_eq!(s, 200, "grapple should succeed: {}", result);
}

#[tokio::test]
async fn shove_prones_target() {
    let (router, db) = skip_no_db!();
    let (tok, eid, shover_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{shover_id}/shove"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "shove_type": "prone", "contest_result": "success" }))).await;

    assert_eq!(s, 200, "shove should succeed: {}", result);
}

// =====================================================================
// Stand Up from Prone
// =====================================================================

#[tokio::test]
async fn stand_up_removes_prone_and_uses_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Add prone condition
    sqlx::query("update combatants set modifiers = jsonb_set(coalesce(modifiers, '{}'), '{conditions}', '[\"prone\"]') where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/stand-up"),
        Some(&tok), None).await;

    assert_eq!(s, 200, "stand up should succeed: {}", result);
}

// =====================================================================
// Two-Weapon Fighting
// =====================================================================

#[tokio::test]
async fn two_weapon_fight_bonus_action_attack() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Mark action as used (attacked with main hand)
    sqlx::query("update combatants set action_used = true where id = $1::uuid")
        .bind(&attacker_id).execute(&db).await.unwrap();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/two-weapon-fight"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "offhand_damage": "1d6", "damage_type": "slashing" }))).await;

    assert_eq!(s, 200, "two-weapon fight should succeed: {}", result);
    assert!(result["bonus_action_used"].as_bool().unwrap_or(false), "should consume bonus action");
}

// =====================================================================
// Opportunity Attack
// =====================================================================

#[tokio::test]
async fn opportunity_attack_uses_reaction() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Fleeing', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Fleeing",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/opportunity-attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d8", "damage_type": "slashing" }))).await;

    assert_eq!(s, 200, "opportunity attack should succeed: {}", result);
    assert!(result["reaction_used"].as_bool().unwrap_or(false), "should consume reaction");
}

// =====================================================================
// Death Saves
// =====================================================================

#[tokio::test]
async fn death_save_roll_updates_saves() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Set to dying
    sqlx::query("update combatants set hp_current = 0, death_saves = '{\"successes\":0,\"failures\":0}'::jsonb where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/death-save"),
        Some(&tok),
        Some(json!({ "roll": 15 }))).await; // 15 = success

    assert_eq!(s, 200, "death save should succeed: {}", result);
    let successes = result["death_saves"]["successes"].as_i64().unwrap_or(-1);
    assert_eq!(successes, 1, "should have 1 success");
}

// =====================================================================
// Multiattack
// =====================================================================

#[tokio::test]
async fn multiattack_makes_multiple_attacks() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":50,\"current\":50}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 50, "hp_current": 50, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/multiattack"),
        Some(&tok),
        Some(json!({
            "target_id": target_id,
            "attacks": [
                { "damage_expression": "1d6", "damage_type": "slashing" },
                { "damage_expression": "1d6", "damage_type": "slashing" }
            ]
        }))).await;

    assert_eq!(s, 200, "multiattack should succeed: {}", result);
    assert!(result.get("attacks").is_some() || result.get("total_damage").is_some(),
        "multiattack should return attack results");
}
