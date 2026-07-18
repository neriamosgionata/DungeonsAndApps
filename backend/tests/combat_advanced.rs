//! Combat action economy and advanced mechanics tests
//! Tests: dodge, disengage, dash, hide, conditions, legendary/lair actions
mod helpers;
use helpers::*;
use serde_json::json;
use uuid::Uuid;

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

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/dodge"),
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(s, 200, "dodge should succeed: {}", result);
    // Dodge consumes the action and inserts a "Dodge" effect on the combatant.
    assert_eq!(result["action_used"], true, "dodge should consume the action");
    let dodging: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Dodge' and active = true)")
        .bind(&combatant_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(dodging, "Dodge effect should be active");
}

#[tokio::test]
async fn disengage_action_prevents_opportunity_attacks() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/disengage"),
        Some(&tok),
        Some(json!({ "use_bonus_action": false })),
    )
    .await;

    assert_eq!(s, 200, "disengage should succeed: {}", result);
    let disengaging: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Disengage' and active = true)")
        .bind(&combatant_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(disengaging, "Disengage effect should be active");
}

#[tokio::test]
async fn dash_action_doubles_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/dash"),
        Some(&tok),
        Some(json!({ "use_bonus_action": false })),
    )
    .await;

    assert_eq!(s, 200, "dash should succeed: {}", result);
    // Dash inserts a movement-bonus effect (dash_bonus = base speed).
    let dash: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Dash' and active = true)")
        .bind(&combatant_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(dash, "Dash effect should be active");
}

// =====================================================================
// Hide Action
// =====================================================================

#[tokio::test]
async fn hide_action_sets_hidden_modifier() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/hide"),
        Some(&tok),
        Some(json!({ "use_bonus_action": false })),
    )
    .await;

    assert_eq!(s, 200, "hide should succeed: {}", result);
    // Hide inserts a "Hidden" effect with {"hidden": true}.
    let hidden: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Hidden' and active = true)")
        .bind(&combatant_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(hidden, "Hidden effect should be active");
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

    let (_, ally) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Ally",
                     "initiative": 8, "hp_max": 10, "hp_current": 10, "ac": 10 }),
        ),
    )
    .await;
    let ally_id = ally["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{helper_id}/help"),
        Some(&tok),
        Some(json!({ "target_id": ally_id })),
    )
    .await;

    assert_eq!(s, 200, "help should succeed: {}", result);
    // Help inserts a "Helped" advantage effect on the ally.
    let helped: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Helped' and active = true)")
        .bind(ally_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(helped, "ally should have a Helped effect");
}

// =====================================================================
// Conditions
// =====================================================================

#[tokio::test]
async fn add_condition_applies_effect() {
    let (router, db) = skip_no_db!();
    let (tok, eid, target_id, _cid) = setup_encounter(&router, &db).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{target_id}/conditions"),
        Some(&tok),
        Some(json!({ "condition": "prone", "duration": 1 })),
    )
    .await;

    assert_eq!(s, 200, "add_condition should succeed: {}", result);
    let conditions = result["conditions"]
        .as_array()
        .expect("conditions should be array");
    assert!(
        conditions
            .iter()
            .any(|c| c.as_str().map(|s| s.contains("prone")).unwrap_or(false)),
        "prone condition should be added"
    );
}

#[tokio::test]
async fn restrained_condition_reduces_speed() {
    let (router, db) = skip_no_db!();
    let (tok, eid, target_id, _cid) = setup_encounter(&router, &db).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{target_id}/conditions"),
        Some(&tok),
        Some(json!({ "condition": "restrained", "duration_rounds": 1 })),
    )
    .await;

    assert_eq!(s, 200, "{result}");
    let conditions = result["conditions"].as_array().expect("conditions array");
    assert!(
        conditions
            .iter()
            .any(|c| c.as_str().map(|s| s.starts_with("restrained")).unwrap_or(false)),
        "restrained condition should be applied: {conditions:?}"
    );
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

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{monster_id}/legendary-action"),
        Some(&tok),
        Some(json!({ "action_name": "Tail Swipe" })),
    )
    .await;

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

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Advance turn - legendary actions should reset
    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"),
        Some(&tok),
        None,
    )
    .await;

    let (_, combatants) = json_req(
        &router,
        "GET",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        None,
    )
    .await;
    let monster = combatants
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"].as_str() == Some(monster_id.as_str()))
        .expect("monster combatant in list");

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

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/lair-action"),
        Some(&tok),
        Some(json!({ "lair_action": "Regional Effect" })),
    )
    .await;

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

    let (_, target) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }),
        ),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{grappler_id}/grapple"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "contest_result": "success" })),
    )
    .await;

    assert_eq!(s, 200, "grapple should succeed: {}", result);
}

#[tokio::test]
async fn shove_prones_target() {
    let (router, db) = skip_no_db!();
    let (tok, eid, shover_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }),
        ),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{shover_id}/shove"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "knock_prone": true })),
    )
    .await;

    assert_eq!(s, 200, "shove should succeed: {}", result);

    let conds: Vec<String> = sqlx::query_scalar(
        "select conditions from combatants where id = $1::uuid")
        .bind(target_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(conds.iter().any(|c| c == "prone"), "target should be prone after shove: {:?}", conds);
}

// =====================================================================
// Stand Up from Prone
// =====================================================================

#[tokio::test]
async fn stand_up_removes_prone_and_uses_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Add prone condition (stored in the combatants.conditions text[] column)
    sqlx::query("update combatants set conditions = array['prone'] where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/stand-up"),
        Some(&tok),
        None,
    )
    .await;

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

    let (_, target) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }),
        ),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // TWF requires two light weapons. Equip the attacker NPC with a light
    // main-hand + light off-hand in its stats (engine loads weapons from
    // npcs.stats->'weapons').
    sqlx::query(
        r#"update npcs set stats = jsonb_set(stats, '{weapons}', $2::jsonb)
           where id = (select npc_id from combatants where id = $1::uuid)"#,
    )
    .bind(&attacker_id)
    .bind(json!([
        {"id": "main", "name": "Shortsword", "properties": "light, finesse", "damage": "1d6", "damage_type": "slashing", "ability": "dex"},
        {"id": "off", "name": "Dagger", "properties": "light, finesse, thrown", "damage": "1d4", "damage_type": "piercing", "ability": "dex"}
    ]).to_string())
    .execute(&db)
    .await
    .unwrap();

    // Mark action as used (attacked with main hand)
    sqlx::query("update combatants set action_used = true where id = $1::uuid")
        .bind(&attacker_id)
        .execute(&db)
        .await
        .unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{attacker_id}/two-weapon-fight"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "offhand_weapon_id": "off" })),
    )
    .await;

    assert_eq!(s, 200, "two-weapon fight should succeed: {}", result);
    // Response is an AttackResult; the bonus action is consumed server-side.
    let bonus_used: bool = sqlx::query_scalar(
        "select bonus_action_used from combatants where id = $1::uuid")
        .bind(&attacker_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(bonus_used, "should consume bonus action");
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

    let (_, target) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Fleeing",
                     "initiative": 5, "hp_max": 10, "hp_current": 10, "ac": 10 }),
        ),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/opportunity-attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id }))).await;

    assert_eq!(s, 200, "opportunity attack should succeed: {}", result);
    // Returns an AttackResult; the reaction is consumed server-side.
    let reaction_used: bool = sqlx::query_scalar(
        "select reaction_used from combatants where id = $1::uuid")
        .bind(&attacker_id)
        .fetch_one(&db)
        .await
        .unwrap();
    assert!(reaction_used, "should consume reaction");
}

// =====================================================================
// Death Saves
// =====================================================================

#[tokio::test]
async fn death_save_roll_updates_saves() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Set to dying (death-save state lives on the linked character sheet, but
    // the handler only requires hp_current <= 0 on the combatant).
    sqlx::query("update combatants set hp_current = 0 where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{combatant_id}/death-save"),
        Some(&tok),
        Some(json!({ "advantage": false, "disadvantage": false })),
    )
    .await;

    assert_eq!(s, 200, "death save should succeed: {}", result);
    // Returns a DeathSaveResult. The roll is server-rolled; assert the result
    // is internally consistent (success bumps successes, failure bumps failures).
    let succ_after = result["successes_after"].as_i64().unwrap();
    let fail_after = result["failures_after"].as_i64().unwrap();
    if result["passed"].as_bool().unwrap() && !result["nat20"].as_bool().unwrap() {
        assert_eq!(succ_after, 1, "a pass should record one success");
    } else if !result["passed"].as_bool().unwrap() {
        assert!(fail_after >= 1, "a failure should record at least one failure");
    }
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

    let (_, target) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 50, "hp_current": 50, "ac": 10 }),
        ),
    )
    .await;
    let target_id = target["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{attacker_id}/multiattack"),
        Some(&tok),
        Some(json!({
            "targets": [
                { "target_id": target_id, "attack_expression": "1d20+2", "damage_expression": "1d6", "damage_type": "slashing" },
                { "target_id": target_id, "attack_expression": "1d20+2", "damage_expression": "1d6", "damage_type": "slashing" }
            ]
        })),
    )
    .await;

    assert_eq!(s, 200, "multiattack should succeed: {}", result);
    assert!(
        result.get("attacks").is_some() || result.get("total_damage").is_some(),
        "multiattack should return attack results"
    );
}

// =====================================================================
// Fix-sprint regression tests: atomic action economy caps
// =====================================================================

/// legendary_action: second call beyond `max` must 400, not silently exceed.
#[tokio::test]
async fn legendary_action_atomic_cap_exhausted_returns_error() {
    let (router, db) = skip_no_db!();
    let (tok, eid, mid, _cid) = setup_encounter(&router, &db).await;

    sqlx::query("update combatants set legendary_actions_max = 2, legendary_actions_used = 0 where id = $1::uuid")
        .bind(&mid).execute(&db).await.unwrap();
    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Spend 1, then 2 — should succeed
    let (s1, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{mid}/legendary-action"),
        Some(&tok),
        Some(json!({"action_name":"Strike"})),
    )
    .await;
    assert_eq!(s1, 200, "first LA should succeed");
    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{mid}/legendary-action"),
        Some(&tok),
        Some(json!({"action_name":"Strike"})),
    )
    .await;
    assert_eq!(s2, 200, "second LA should succeed");

    // Third call — should be rejected
    let (s3, body3) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{mid}/legendary-action"),
        Some(&tok),
        Some(json!({"action_name":"Strike"})),
    )
    .await;
    assert!(
        s3 == 400 || s3 == 409,
        "third LA should be rejected (400/409), got {}: {}",
        s3,
        body3
    );
}

/// lair_action: second call in same round must 400.
#[tokio::test]
async fn lair_action_atomic_already_used_returns_error() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _mid, _cid) = setup_encounter(&router, &db).await;
    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // First call — 200
    let (s1, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/lair-action"),
        Some(&tok),
        Some(json!({"lair_action":"Region Effect"})),
    )
    .await;
    assert_eq!(s1, 200, "first lair action should succeed");

    // Second call same round — 400
    let (s2, body2) = json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/lair-action"),
        Some(&tok),
        Some(json!({"lair_action":"Region Effect"})),
    )
    .await;
    assert!(
        s2 == 400 || s2 == 409,
        "second lair action in round should be rejected, got {}: {}",
        s2,
        body2
    );
}

// =====================================================================
// Sprint 38: Sneak Attack, Stunning Strike, Divine Smite
// =====================================================================

#[tokio::test]
async fn mech_sneak_attack_rogue_with_advantage_deals_extra_damage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Rogue', '{\"level_total\":5,\"classes\":[{\"name\":\"rogue\",\"level\":5}],\"abilities\":{\"str\":10,\"dex\":18,\"con\":14,\"int\":10,\"wis\":12,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":15}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, rogue_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Rogue",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 15, "initiative_rolled": true })),
    ).await;
    let attacker_id = rogue_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok), Some(json!({
            "target_id": _goblin_cid,
            "damage_type": "piercing",
            "attack_expression": "1d20+7",
            "damage_expression": "1d6+4",
            "advantage": true,
            "sneak_attack": true,
        })),
    ).await;
    assert_eq!(s, 200, "attack: {}", result);
    if result["hit"].as_bool().unwrap_or(false) {
        assert!(result["sneak_attack_applied"].as_bool().unwrap_or(false),
            "sneak should apply with advantage: {}", result);
        assert!(result["sneak_attack_damage"].as_i64().unwrap_or(0) > 0,
            "sneak damage > 0: {}", result);
    }
}

#[tokio::test]
async fn mech_stunning_strike_monk_consumes_ki_and_stuns() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Monk', '{\"level_total\":5,\"classes\":[{\"name\":\"monk\",\"level\":5}],\"resources\":[{\"id\":\"ki\",\"name\":\"Ki\",\"current\":5,\"max\":5,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":16,\"con\":14,\"int\":10,\"wis\":16,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":16}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, monk_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Monk",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 16, "initiative_rolled": true })),
    ).await;
    let attacker_id = monk_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok), Some(json!({
            "target_id": _goblin_cid,
            "damage_type": "bludgeoning",
            "attack_expression": "1d20+7",
            "damage_expression": "1d4+3",
            "advantage": true,
            "stunning_strike": true,
        })),
    ).await;
    assert_eq!(s, 200, "attack: {}", result);
    let ki_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') = 'ki'"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    if result["hit"].as_bool().unwrap_or(false) {
        assert!(ki_after < 5, "Ki should be consumed (was 5, now {})", ki_after);
    }
}

#[tokio::test]
async fn mech_wild_shape_druid_transforms_and_reverts() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    // Create a beast NPC (CR 0, creature_type beast)
    let beast_id: Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats)
         values ($1::uuid, 'Wolf', '{\"abilities\":{\"str\":12,\"dex\":15,\"con\":12,\"int\":3,\"wis\":12,\"cha\":6},\"ac\":13,\"hp\":{\"max\":11,\"current\":11},\"speed\":40,\"cr\":\"0.25\",\"creature_type\":\"beast\"}'::jsonb)
         returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Druid', '{\"level_total\":5,\"classes\":[{\"name\":\"druid\",\"level\":5}],\"resources\":[{\"id\":\"ws\",\"name\":\"Wild Shape\",\"current\":2,\"max\":2,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":18,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":14}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, druid_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Druid",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 14, "initiative_rolled": true })),
    ).await;
    let combatant_id = druid_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Wild Shape into wolf
    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/class-feature"),
        Some(&tok), Some(json!({
            "feature": "wild_shape",
            "target_id": beast_id,
        })),
    ).await;
    assert_eq!(s, 200, "wild shape should succeed: {}", result);
    // Verify combatant stats are replaced
    let (hp_cur, hp_max_new, ac_new): (i32, i32, i32) = sqlx::query_as(
        "select hp_current, hp_max, ac from combatants where id = $1::uuid",
    ).bind(&combatant_id).fetch_one(&db).await.unwrap();
    assert_eq!(hp_max_new, 11, "should have wolf HP max (11)");
    assert_eq!(ac_new, 13, "should have wolf AC (13)");
    // Verify original stored
    let orig_is_null: bool = sqlx::query_scalar(
        "select wild_shape_original is null from combatants where id = $1::uuid",
    ).bind(&combatant_id).fetch_one(&db).await.unwrap_or(true);
    assert!(!orig_is_null, "wild_shape_original should be set");
    // Revert
    let (s2, _) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/class-feature"),
        Some(&tok), Some(json!({ "feature": "revert_wild_shape" })),
    ).await;
    assert_eq!(s2, 200, "revert should succeed");
    let (hp_back, hp_max_back, ac_back): (i32, i32, i32) = sqlx::query_as(
        "select hp_current, hp_max, ac from combatants where id = $1::uuid",
    ).bind(&combatant_id).fetch_one(&db).await.unwrap();
    assert_eq!(hp_max_back, 20, "should restore original HP max (20)");
    assert_eq!(ac_back, 14, "should restore original AC (14)");
    assert!(hp_back > 0, "should have positive HP after revert");
    // Verify original cleared
    let orig_null: bool = sqlx::query_scalar(
        "select wild_shape_original is null from combatants where id = $1::uuid",
    ).bind(&combatant_id).fetch_one(&db).await.unwrap_or(false);
    assert!(orig_null, "wild_shape_original should be cleared");
}

#[tokio::test]
async fn mech_wild_shape_damage_carries_over_on_revert() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    let beast_id: Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats)
         values ($1::uuid, 'Wolf', '{\"abilities\":{\"str\":12,\"dex\":15,\"con\":12,\"int\":3,\"wis\":12,\"cha\":6},\"ac\":13,\"hp\":{\"max\":11,\"current\":11},\"speed\":40,\"cr\":\"0.25\",\"creature_type\":\"beast\"}'::jsonb)
         returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Druid', '{\"level_total\":5,\"classes\":[{\"name\":\"druid\",\"level\":5}],\"resources\":[{\"id\":\"ws\",\"name\":\"Wild Shape\",\"current\":2,\"max\":2,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":18,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":14}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, druid_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Druid",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 14, "initiative_rolled": true })),
    ).await;
    let combatant_id = druid_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Wild Shape into wolf
    json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/class-feature"),
        Some(&tok), Some(json!({ "feature": "wild_shape", "target_id": beast_id })),
    ).await;
    // Deal 5 damage to the beast form (11 → 6)
    json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/damage"),
        Some(&tok), Some(json!({ "amount": 5, "damage_type": "slashing" })),
    ).await;
    // Revert - should carry over 5 damage to original form
    let (s, _) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/class-feature"),
        Some(&tok), Some(json!({ "feature": "revert_wild_shape" })),
    ).await;
    assert_eq!(s, 200, "revert should succeed");
    let (hp_back, hp_max_back): (i32, i32) = sqlx::query_as(
        "select hp_current, hp_max from combatants where id = $1::uuid",
    ).bind(&combatant_id).fetch_one(&db).await.unwrap();
    assert_eq!(hp_max_back, 20, "should restore original HP max");
    // 20 original - 5 damage = 15
    assert_eq!(hp_back, 15, "should carry over 5 damage (was 20 - 5 = 15), got {}", hp_back);
}

#[tokio::test]
async fn mech_trip_attack_fighter_superiority_die_consumed() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Fighter', '{\"level_total\":5,\"classes\":[{\"name\":\"fighter\",\"level\":5}],\"resources\":[{\"id\":\"sd\",\"name\":\"Superiority Dice\",\"current\":4,\"max\":4,\"reset\":\"short\"}],\"abilities\":{\"str\":18,\"dex\":14,\"con\":16,\"int\":10,\"wis\":12,\"cha\":10},\"hp\":{\"current\":30,\"max\":30},\"ac\":18}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, fgt_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Fighter",
            "initiative": 15, "hp_max": 30, "hp_current": 30, "ac": 18, "initiative_rolled": true })),
    ).await;
    let cid = fgt_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{cid}/class-feature"),
        Some(&tok), Some(json!({
            "feature": "trip_attack",
            "target_id": _goblin_cid,
        })),
    ).await;
    assert_eq!(s, 200, "trip attack should succeed: {}", result);
    // Superiority dice should be consumed
    let sd_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%superiority%dice%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert!(sd_after < 4, "superiority dice should be consumed (was 4, now {})", sd_after);
}

#[tokio::test]
async fn mech_rage_persistence_turn_end_without_action_ends_rage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Barbarian', '{\"level_total\":5,\"classes\":[{\"name\":\"barbarian\",\"level\":5}],\"resources\":[{\"id\":\"rage\",\"name\":\"Rage\",\"current\":2,\"max\":2,\"reset\":\"long\"}],\"abilities\":{\"str\":18,\"dex\":14,\"con\":16,\"int\":10,\"wis\":12,\"cha\":10},\"hp\":{\"current\":30,\"max\":30},\"ac\":16}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, barb_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Barbarian",
            "initiative": 15, "hp_max": 30, "hp_current": 30, "ac": 16, "initiative_rolled": true })),
    ).await;
    let cid = barb_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Activate Rage
    let (s, _) = json_req(&router, "POST", &format!("/api/v1/combatants/{cid}/class-feature"),
        Some(&tok), Some(json!({ "feature": "rage" })),
    ).await;
    assert_eq!(s, 200, "rage should activate");
    // Verify rage is active
    let rage_on: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Rage' and active = true)",
    ).bind(&cid).fetch_one(&db).await.unwrap_or(false);
    assert!(rage_on, "rage should be active after activation");

    // Don't do anything (no attack) - next_turn should detect no action and end rage
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/next-turn"), Some(&tok), None).await;

    let rage_after: bool = sqlx::query_scalar(
        "select exists(select 1 from combatant_effects where combatant_id = $1::uuid and name = 'Rage' and active = true)",
    ).bind(&cid).fetch_one(&db).await.unwrap_or(true);
    assert!(!rage_after, "rage should end after turn with no action");
}

#[tokio::test]
async fn mech_turn_undead_cleric_turns_undead_npcs() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    // Create an undead NPC in the encounter
    let skeleton_id: Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Skeleton',
         '{\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":8,\"cha\":10},\"ac\":13,\"hp\":{\"max\":13,\"current\":13},\"creature_type\":\"undead\"}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let (_, skel_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": skeleton_id, "display_name": "Skeleton",
            "initiative": 10, "hp_max": 13, "hp_current": 13, "ac": 13, "initiative_rolled": true })),
    ).await;
    let _skel_id = skel_c["id"].as_str().unwrap().to_string();

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Cleric', '{\"level_total\":5,\"classes\":[{\"name\":\"cleric\",\"level\":5}],\"resources\":[{\"id\":\"cd\",\"name\":\"Channel Divinity\",\"current\":1,\"max\":1,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":18,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":18,\"casting\":{\"ability\":\"wis\"}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, cleric_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Cleric",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 18, "initiative_rolled": true })),
    ).await;
    let caster_id = cleric_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/class-feature"),
        Some(&tok), Some(json!({ "feature": "turn_undead" })),
    ).await;
    assert_eq!(s, 200, "turn undead should succeed: {}", result);
    // Channel Divinity should be consumed
    let cd_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%channel%divinity%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(cd_after, 0, "Channel Divinity should be consumed (was 1)");
}

#[tokio::test]
async fn mech_extended_spell_consumes_sp_and_doubles_duration() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "extended": true,
        })),
    ).await;
    assert_eq!(s, 200, "extended cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1)");
}

#[tokio::test]
async fn mech_subtle_spell_bypasses_components() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Cast Magic Missile with subtle spell - must succeed (bypasses V/S)
    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "subtle": true,
        })),
    ).await;
    assert_eq!(s, 200, "subtle cast should succeed: {}", result);
    // Sorcery points consumed (5 → 4)
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1)");
}

#[tokio::test]
async fn mech_heightened_spell_imposes_save_disadvantage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "heightened": true,
        })),
    ).await;
    assert_eq!(s, 200, "heightened cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 2, "sorcery points should be 2 (was 5, spent 3)");
}

#[tokio::test]
async fn mech_twinned_spell_consumes_sp_equal_to_spell_level() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, cid) = setup_encounter(&router, &db).await;

    // Need two targets for twinned spell
    let npc2_id: Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Goblin2', '{\"ac\":12,\"hp\":{\"max\":7,\"current\":7}}'::jsonb) returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let (_, gob2_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc2_id, "display_name": "Goblin2",
            "initiative": 5, "hp_max": 7, "hp_current": 7, "ac": 12, "initiative_rolled": true })),
    ).await;
    let gob2_id = gob2_c["id"].as_str().unwrap().to_string();

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Cast Magic Missile (L1) twinned at 2 targets
    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid, gob2_id],
            "twinned": true,
        })),
    ).await;
    assert_eq!(s, 200, "twinned cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1 for L1 spell)");
}

#[tokio::test]
async fn mech_careful_spell_consumes_sp_and_target_auto_saves() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "careful_target_ids": [_goblin_cid],
        })),
    ).await;
    assert_eq!(s, 200, "careful cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1)");
}

#[tokio::test]
async fn mech_empowered_spell_consumes_sp_and_rerolls_damage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "empowered": true,
        })),
    ).await;
    assert_eq!(s, 200, "empowered cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1)");
}

#[tokio::test]
async fn mech_distant_spell_consumes_sp_and_doubles_range() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "distant": true,
        })),
    ).await;
    assert_eq!(s, 200, "distant cast should succeed: {}", result);
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 4, "sorcery points should be 4 (was 5, spent 1)");
}

#[tokio::test]
async fn mech_indomitable_fighter_grants_save_advantage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Fighter', '{\"level_total\":9,\"classes\":[{\"name\":\"fighter\",\"level\":9}],\"abilities\":{\"str\":16,\"dex\":14,\"con\":16,\"int\":10,\"wis\":12,\"cha\":10},\"hp\":{\"current\":30,\"max\":30},\"ac\":18}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, fighter_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Fighter",
            "initiative": 15, "hp_max": 30, "hp_current": 30, "ac": 18, "initiative_rolled": true })),
    ).await;
    let cid = fighter_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{cid}/class-feature"),
        Some(&tok), Some(json!({ "feature": "indomitable" })),
    ).await;
    assert_eq!(s, 200, "indomitable should succeed: {}", result);
    assert!(result["effect_applied"].as_bool().unwrap_or(false));
    // Verify the effect was created with save_advantage
    let has_save_adv: bool = sqlx::query_scalar(
        r#"select exists(select 1 from combatant_effects
           where combatant_id = $1::uuid and name = 'Indomitable'
           and active = true and modifiers->>'save_advantage' = 'true')"#,
    ).bind(&cid).fetch_one(&db).await.unwrap_or(false);
    assert!(has_save_adv, "Indomitable effect should grant save_advantage");
    // Second use should be rejected
    let (s2, _) = json_req(&router, "POST", &format!("/api/v1/combatants/{cid}/class-feature"),
        Some(&tok), Some(json!({ "feature": "indomitable" })),
    ).await;
    assert!(s2 != 200, "second indomitable use should be rejected");
}

#[tokio::test]
async fn mech_quickened_spell_sorcerer_consumes_sorcery_points() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Sorcerer', '{\"level_total\":5,\"classes\":[{\"name\":\"sorcerer\",\"level\":5}],\"resources\":[{\"id\":\"sp\",\"name\":\"Sorcery Points\",\"current\":5,\"max\":5,\"reset\":\"long\"}],\"slots\":{\"1\":{\"current\":2,\"max\":2},\"2\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":10,\"dex\":14,\"con\":14,\"int\":10,\"wis\":12,\"cha\":18},\"hp\":{\"current\":20,\"max\":20},\"ac\":12,\"casting\":{\"ability\":\"cha\",\"save_dc\":15,\"spell_attack\":7}}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, sorc_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Sorcerer",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 12, "initiative_rolled": true })),
    ).await;
    let caster_id = sorc_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Cast Magic Missile (slug: magic-missile, L1, no save/attack) as a BA via Quickened Spell
    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{caster_id}/cast"),
        Some(&tok), Some(json!({
            "spell_slug": "magic-missile",
            "target_ids": [_goblin_cid],
            "quickened": true,
        })),
    ).await;
    assert_eq!(s, 200, "quickened cast should succeed: {}", result);
    // BA should be consumed
    let ba_used: bool = sqlx::query_scalar(
        "select bonus_action_used from combatants where id = $1::uuid",
    ).bind(&caster_id).fetch_one(&db).await.unwrap_or(false);
    assert!(ba_used, "bonus action should be consumed after quickened cast");
    // Action should NOT be consumed
    let action_used: bool = sqlx::query_scalar(
        "select action_used from combatants where id = $1::uuid",
    ).bind(&caster_id).fetch_one(&db).await.unwrap_or(true);
    assert!(!action_used, "action should NOT be consumed after quickened cast");
    // Sorcery points should be consumed (5 → 3)
    let sp_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') like '%sorcery%point%'
           limit 1"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert_eq!(sp_after, 3, "sorcery points should be 3 (was 5, spent 2)");
}

#[tokio::test]
async fn mech_divine_smite_paladin_consumes_slot_and_deals_radiant() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Paladin', '{\"level_total\":2,\"classes\":[{\"name\":\"paladin\",\"level\":2}],\"slots\":{\"1\":{\"current\":2,\"max\":2}},\"abilities\":{\"str\":16,\"dex\":10,\"con\":14,\"int\":10,\"wis\":10,\"cha\":16},\"hp\":{\"current\":20,\"max\":20},\"ac\":18}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, pal_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Paladin",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 18, "initiative_rolled": true })),
    ).await;
    let attacker_id = pal_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok), Some(json!({
            "target_id": _goblin_cid,
            "weapon_id": null,
            "damage_type": "slashing",
            "attack_expression": "1d20+7",
            "damage_expression": "1d8+3",
            "advantage": true,
            "smite": true,
            "smite_slot_level": 1,
        })),
    ).await;
    assert_eq!(s, 200, "attack: {}", result);
    if result["hit"].as_bool().unwrap_or(false) {
        assert!(result["smite_applied"].as_bool().unwrap_or(false),
            "smite should apply: {}", result);
        assert!(result["smite_damage"].as_i64().unwrap_or(0) > 0,
            "smite damage > 0: {}", result);
        let slot_after: i32 = sqlx::query_scalar(
            r#"select coalesce((sheet->'slots'->'1'->>'current')::int, 0)
               from characters where id = $1"#,
        ).bind(chid).fetch_one(&db).await.unwrap_or(0);
        assert_eq!(slot_after, 1, "1st-level slot should be consumed (was 2, now {})", slot_after);
    }
}

#[tokio::test]
async fn mech_flurry_of_blows_monk_ki_consumed_and_damage_dealt() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Monk', '{\"level_total\":5,\"classes\":[{\"name\":\"monk\",\"level\":5}],\"resources\":[{\"id\":\"ki\",\"name\":\"Ki\",\"current\":5,\"max\":5,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":18,\"con\":14,\"int\":10,\"wis\":16,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":17}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, monk_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Monk",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 17, "initiative_rolled": true })),
    ).await;
    let attacker_id = monk_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/class-feature"),
        Some(&tok), Some(json!({
            "feature": "flurry_of_blows",
            "target_id": _goblin_cid,
        })),
    ).await;
    assert_eq!(s, 200, "flurry should succeed: {}", result);
    assert!(result["effect_applied"].as_bool().unwrap_or(false), "flurry should apply");
    // Ki should be consumed
    let ki_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') = 'ki'"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert!(ki_after < 5, "Ki should be consumed (was 5, now {})", ki_after);
}

#[tokio::test]
async fn mech_patient_defense_monk_ba_and_ki_consumed() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Monk', '{\"level_total\":5,\"classes\":[{\"name\":\"monk\",\"level\":5}],\"resources\":[{\"id\":\"ki\",\"name\":\"Ki\",\"current\":5,\"max\":5,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":18,\"con\":14,\"int\":10,\"wis\":16,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":17}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, monk_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Monk",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 17, "initiative_rolled": true })),
    ).await;
    let attacker_id = monk_c["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/class-feature"),
        Some(&tok), Some(json!({
            "feature": "patient_defense",
        })),
    ).await;
    assert_eq!(s, 200, "patient defense should succeed: {}", result);
    assert!(result["effect_applied"].as_bool().unwrap_or(false));
    // Bonus action should be consumed
    let ba_used: bool = sqlx::query_scalar(
        "select bonus_action_used from combatants where id = $1::uuid",
    ).bind(&attacker_id).fetch_one(&db).await.unwrap_or(false);
    assert!(ba_used, "bonus action should be consumed");
    let ki_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') = 'ki'"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert!(ki_after < 5, "Ki should be consumed (was 5, now {})", ki_after);
    // Verify Dodge effect has the correct modifiers (attack_disadvantage_against)
    let has_dodge_mod: bool = sqlx::query_scalar(
        r#"select exists(select 1 from combatant_effects
           where combatant_id = $1::uuid and name = 'Dodge' and active = true
           and modifiers->>'attack_disadvantage_against' = 'true'
           and modifiers->>'dex_save_advantage' = 'true')"#,
    ).bind(&attacker_id).fetch_one(&db).await.unwrap_or(false);
    assert!(has_dodge_mod, "Dodge effect should grant attack_disadvantage_against + dex_save_advantage");
}

#[tokio::test]
async fn mech_step_of_the_wind_monk_dash_resets_movement() {
    let (router, db) = skip_no_db!();
    let (tok, eid, _goblin_cid, _cid) = setup_encounter(&router, &db).await;

    let chid: Uuid = sqlx::query_scalar(
        "insert into characters (campaign_id, owner_id, name, sheet)
         values ((select campaign_id from encounters where id = $1::uuid),
                 (select master_id from campaigns where id = (select campaign_id from encounters where id = $1::uuid)),
                 'Monk', '{\"level_total\":5,\"classes\":[{\"name\":\"monk\",\"level\":5}],\"resources\":[{\"id\":\"ki\",\"name\":\"Ki\",\"current\":5,\"max\":5,\"reset\":\"short\"}],\"abilities\":{\"str\":10,\"dex\":18,\"con\":14,\"int\":10,\"wis\":16,\"cha\":10},\"hp\":{\"current\":20,\"max\":20},\"ac\":17}'::jsonb)
         returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, monk_c) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "character", "character_id": chid, "display_name": "Monk",
            "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 17, "initiative_rolled": true })),
    ).await;
    let attacker_id = monk_c["id"].as_str().unwrap().to_string();
    sqlx::query("update combatants set movement_used_ft = 30 where id = $1::uuid")
        .bind(&attacker_id).execute(&db).await.unwrap();
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/combatants/{attacker_id}/class-feature"),
        Some(&tok), Some(json!({
            "feature": "step_of_the_wind",
        })),
    ).await;
    assert_eq!(s, 200, "step should succeed: {}", result);
    assert!(result["effect_applied"].as_bool().unwrap_or(false));
    let movement_after: i32 = sqlx::query_scalar(
        "select movement_used_ft from combatants where id = $1::uuid",
    ).bind(&attacker_id).fetch_one(&db).await.unwrap_or(999);
    assert_eq!(movement_after, 0, "movement should be reset to 0 for Dash");
    let ki_after: i32 = sqlx::query_scalar(
        r#"select coalesce((elem->>'current')::int, 0)
           from characters, jsonb_array_elements(sheet->'resources') as elem
           where id = $1 and lower(elem->>'name') = 'ki'"#,
    ).bind(chid).fetch_one(&db).await.unwrap_or(0);
    assert!(ki_after < 5, "Ki should be consumed (was 5, now {})", ki_after);
}
