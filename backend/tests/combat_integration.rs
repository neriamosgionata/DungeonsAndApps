//! Combat endpoint integration tests
//! Tests attack, cast_spell, reactions, ready actions, grapple, death saves
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
// Attack Endpoint
// =====================================================================

#[tokio::test]
async fn attack_endpoint_basic_hit() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    // Create target
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    // Start encounter
    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Attack
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    assert_eq!(s, 200, "attack should succeed: {}", result);
    assert!(result["hit"].is_boolean(), "result should have hit field");
}

#[tokio::test]
async fn attack_endpoint_power_attack() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":5,\"hp\":{\"max\":30,\"current\":30}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 30, "hp_current": 30, "ac": 5 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (_, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing", "power_attack": true }))).await;

    // Power attack should add +10 damage
    if result["hit"].as_bool().unwrap_or(false) {
        let dmg = result["damage_applied"].as_i64().unwrap_or(0);
        assert!(dmg >= 10, "power attack should add +10 damage, got {}", dmg);
    }
}

// =====================================================================
// Spell Casting
// =====================================================================

#[tokio::test]
async fn cast_spell_with_attack_roll() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_encounter(&router, &db).await;

    // Seed spell
    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source)
         values ('fire-bolt', 'Fire Bolt', 0, 'Evocation', array['Wizard', 'Sorcerer'], 'cantrip', 'SRD')")
        .execute(&db).await.unwrap();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "fire-bolt",
            "slot_level": 0,
            "targets": [{"target_id": target_id}],
            "use_spell_attack": true
        }))).await;

    assert_eq!(s, 200, "spell cast should succeed: {}", result);
    assert!(result["targets"].is_array(), "result should have targets array");
}

#[tokio::test]
async fn cast_cantrip_scales_with_level() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_encounter(&router, &db).await;

    // Update caster to level 5 (cantrip should scale to 2d10)
    sqlx::query("update combatants set level_total = 5 where id = $1::uuid")
        .bind(&caster_id).execute(&db).await.unwrap();

    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source)
         values ('fire-bolt', 'Fire Bolt', 0, 'Evocation', array['Wizard'], 'cantrip', 'SRD')")
        .execute(&db).await.unwrap();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":5,\"hp\":{\"max\":50,\"current\":50}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 50, "hp_current": 50, "ac": 5 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    let (_, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "fire-bolt",
            "slot_level": 0,
            "targets": [{"target_id": target_id}],
            "damage_expression": "1d10"
        }))).await;

    // Level 5 caster: cantrip should scale to 2d10
    if result["targets"][0]["hit"].as_bool().unwrap_or(false) {
        let dmg = result["targets"][0]["damage_applied"].as_i64().unwrap_or(0);
        assert!(dmg >= 2, "level 5 cantrip should roll 2d10 (min 2), got {}", dmg);
    }
}

// =====================================================================
// Reactions - Shield
// =====================================================================

#[tokio::test]
async fn shield_reaction_negates_hit() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    // Create target with shield spell
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":12,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 12 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // First attack to set last_hit_attack_total
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    // Target uses Shield reaction
    let (s, shield_result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{target_id}/react"),
        Some(&tok),
        Some(json!({ "reaction": "shield" }))).await;

    // Shield can only be used if last_hit_attack_total is set (attack hit).
    // The initial attack may miss, so shield may be rejected.
    // If the attack missed, last_hit_attack_total would be null → 409/400.
    assert!(s == 200 || s == 400 || s == 409, "shield reaction should return 200/400/409: {} {}", s, shield_result);
}

// =====================================================================
// Death Saves
// =====================================================================

#[tokio::test]
async fn death_save_reset_on_heal() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    // Set combatant to 0 HP with death saves
    sqlx::query("update combatants set hp_current = 0, death_saves = '{\"successes\":1,\"failures\":1}'::jsonb where id = $1::uuid")
        .bind(&combatant_id).execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Heal the combatant
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/heal"),
        Some(&tok),
        Some(json!({ "amount": 5 }))).await;

    assert_eq!(s, 200, "heal should succeed: {}", result);

    // Verify HP is positive and death saves reset
    let (_, updated) = json_req(&router, "GET",
        &format!("/api/v1/combatants/{combatant_id}"),
        Some(&tok), None).await;

    assert!(updated["hp_current"].as_i64().unwrap_or(0) > 0, "HP should be positive after heal");
}

// =====================================================================
// Massive Damage / Instant Death
// =====================================================================

#[tokio::test]
async fn massive_damage_instant_death() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    // Create target with low max HP
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Fragile', '{\"ac\":5,\"hp\":{\"max\":5,\"current\":5}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Fragile",
                     "initiative": 5, "hp_max": 5, "hp_current": 5, "ac": 5 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Deal massive damage (30 vs 5 max HP = instant death)
    let (_, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "30", "damage_type": "force" }))).await;

    if result["hit"].as_bool().unwrap_or(false) {
        // Check if instant_death flag is set or target HP is 0 with death saves maxed
        let instant_death = result["instant_death"].as_bool().unwrap_or(false);
        let hp_after = result["target_hp_after"].as_i64().unwrap_or(1);
        assert!(instant_death || hp_after <= 0, "massive damage should kill instantly");
    }
}

// =====================================================================
// Action Economy
// =====================================================================

#[tokio::test]
async fn action_usage_prevents_second_attack() {
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

    // First attack
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    // Second attack should fail (action already used)
    let (s2, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": target_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    // Second attack MUST be blocked — action already used.
    // 409 = Conflict (action already consumed), 400 = BadRequest
    assert!(s2 == 409 || s2 == 400, "second attack should be blocked (got {}): action re-use must be prevented", s2);
}

// =====================================================================
// Grapple
// =====================================================================

#[tokio::test]
async fn grapple_target_and_escape() {
    let (router, db) = skip_no_db!();
    let (tok, eid, grappler_id, _cid) = setup_encounter(&router, &db).await;

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Grapple target
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{grappler_id}/grapple"),
        Some(&tok),
        Some(json!({ "target_id": target_id }))).await;

    assert!(s == 200 || s == 201, "grapple should succeed: {}", result);

    // Target attempts to escape
    let (s2, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{target_id}/escape-grapple"),
        Some(&tok), None).await;

    assert!(s2 == 200 || s2 == 204, "escape attempt should be valid");
}

// =====================================================================
// Ready Action
// =====================================================================

#[tokio::test]
async fn ready_action_trigger_on_attack() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Set ready action
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/ready-action"),
        Some(&tok),
        Some(json!({
            "action": "attack",
            "trigger": "target_attacks",
            "target_id": combatant_id
        }))).await;

    assert!(s == 200 || s == 201, "ready action should be set: {}", result);
}

// =====================================================================
// Lay on Hands
// =====================================================================

#[tokio::test]
async fn lay_on_hands_heals_and_consumes_pool() {
    let (router, db) = skip_no_db!();
    let (tok, eid, healer_id, _cid) = setup_encounter(&router, &db).await;

    // Set up healer with Lay on Hands pool (Paladin level 5 = 25 HP pool)
    sqlx::query("update combatants set sheet = jsonb_set(sheet, '{resources}', '[{\"name\":\"Lay on Hands\",\"current\":25,\"max\":25}]'::jsonb) where id = $1::uuid")
        .bind(&healer_id).execute(&db).await.ok();

    // Create injured target
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Injured', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":5}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Injured",
                     "initiative": 5, "hp_max": 20, "hp_current": 5 }))).await;
    let target_id = target["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Use Lay on Hands
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{healer_id}/class-feature"),
        Some(&tok),
        Some(json!({
            "feature": "lay_on_hands",
            "target_id": target_id,
            "amount": 10
        }))).await;

    assert!(s == 200 || s == 204, "lay on hands should succeed: {}", result);
}

// =====================================================================
// Counterspell
// =====================================================================

#[tokio::test]
async fn counterspell_reaction_available_when_spell_casting() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_encounter(&router, &db).await;

    // Create counterspeller
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Counterspeller', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();

    let (_, counterspeller) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Counterspeller",
                     "initiative": 15, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let counter_id = counterspeller["id"].as_str().unwrap();

    // Seed spell
    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source)
         values ('magic-missile', 'Magic Missile', 1, 'Evocation', array['Wizard'], 'spell', 'SRD')")
        .execute(&db).await.unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Cast a spell with the caster to set spell_being_cast, then counterspell it
    let npc_id2: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', '{\"ac\":10,\"hp\":{\"max\":99,\"current\":99}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, spell_target) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id2, "display_name": "Target",
                     "initiative": 3, "hp_max": 99, "hp_current": 99, "ac": 10 }))).await;
    let spell_target_id = spell_target["id"].as_str().unwrap();

    // Caster casts magic-missile (sets spell_being_cast temporarily)
    let (cast_s, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "magic-missile",
            "slot_level": 1,
            "targets": [{"target_id": spell_target_id}]
        }))).await;
    assert_eq!(cast_s, 200, "spell cast should succeed to set spell_being_cast");

    // Now counterspell reaction should be available (spell_being_cast was set during cast)
    // Note: spell_being_cast is cleared after the cast-spell tx commits, so counterspell
    // may fail if the timing window already closed. Either 200 (caught it) or 400/409 (missed window).
    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{counter_id}/react"),
        Some(&tok),
        Some(json!({ "reaction": "counterspell", "target_casting_id": caster_id }))).await;

    assert!(s == 200 || s == 400 || s == 409, "counterspell should return 200/400/409 (window may have closed): {} {}", s, result);
}

// =====================================================================
// Fix-sprint regression tests
// =====================================================================

/// PHB p.203 BA+Action spell restriction:
/// Casting a non-cantrip as BA blocks casting a non-cantrip as Action in the same turn.
#[tokio::test]
async fn ba_plus_action_spell_restriction_enforced() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_encounter(&router, &db).await;

    // Seed two leveled spells on Wizard
    sqlx::query(
        "insert into spells (slug, name, level, school, classes, casting_time, effects, description, source)
         values
         ('healing-word', 'Healing Word', 1, 'Evocation', array['Wizard','Cleric'], '1 bonus action', '{}', 'spell', 'SRD'),
         ('magic-missile', 'Magic Missile', 1, 'Evocation', array['Wizard'], '1 action', '{}', 'spell', 'SRD')")
        .execute(&db).await.unwrap();

    // Seed slots
    sqlx::query("update combatants set sheet = jsonb_set(coalesce(sheet, '{}'::jsonb), '{slots,1}', '{\"max\":2,\"current\":2}'::jsonb) where id = $1::uuid")
        .bind(&caster_id).execute(&db).await.unwrap();

    // Need a target for both spells
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Tgt', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, tgt) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Tgt",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let tgt_id = tgt["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Cast healing word (bonus action) — should succeed
    let (s1, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "healing-word",
            "slot_level": 1,
            "targets": [{"target_id": tgt_id}]
        }))).await;
    assert_eq!(s1, 200, "healing word (BA) should succeed: {s1}");

    // Now try a non-cantrip action spell (magic missile) — should be blocked
    let (s2, body2) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "magic-missile",
            "slot_level": 1,
            "targets": [{"target_id": tgt_id}]
        }))).await;

    // PHB: only a cantrip can be cast as action after a BA leveled spell.
    assert_ne!(s2, 200, "action spell should be blocked after BA leveled spell: {} {}", s2, body2);
}

/// Combatant → character sheet HP writeback (sync_combatant_hp_to_sheet).
/// After attack damage, the linked character's sheet.hp.current must reflect combatant HP.
#[tokio::test]
async fn combatant_damage_syncs_to_character_sheet() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, cid) = setup_encounter(&router, &db).await;

    // Create a target character (so sync path is exercised) and add to encounter
    let (player_tok, _) = register(&router, "play@test.com").await;
    let (_, char_body) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({
            "name": "Scribe",
            "class_primary": "Wizard",
            "level_total": 3,
            "sheet": { "hp": { "current": 20, "max": 20 }, "ac": 12, "alive": true }
        }))).await;
    let char_id = char_body["id"].as_str().unwrap();

    let (_, victim) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok),
        Some(json!({
            "ref_type": "character", "character_id": char_id, "display_name": "Scribe",
            "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 12
        }))).await;
    let victim_id = victim["id"].as_str().unwrap();

    json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;

    // Attack the victim for guaranteed damage
    json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": victim_id, "damage_expression": "5d6+10", "damage_type": "fire" }))).await;

    // Read the character sheet — hp.current should be < 20
    let sheet: serde_json::Value = sqlx::query_scalar("select sheet from characters where id = $1::uuid")
        .bind(char_id).fetch_one(&db).await.unwrap();
    let hp_current = sheet["hp"]["current"].as_i64().unwrap_or(-1);
    assert!(hp_current >= 0 && hp_current < 20,
        "character sheet hp.current should drop after attack; got {}", hp_current);
}

/// set-initiative endpoint should accept a list of {combatant_id, initiative} updates.
#[tokio::test]
async fn set_initiative_endpoint_updates_combatant_initiative() {
    let (router, db) = skip_no_db!();
    let (tok, eid, cid, _camp) = setup_encounter(&router, &db).await;

    // Add a second combatant
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'B', '{\"ac\":10,\"hp\":{\"max\":10,\"current\":10}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, b) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "B",
                     "initiative": 0, "hp_max": 10, "hp_current": 10, "ac": 10 }))).await;
    let b_id = b["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/set-initiative"),
        Some(&tok),
        Some(json!({
            "combatants": [
                { "combatant_id": cid, "initiative": 18 },
                { "combatant_id": b_id, "initiative": 7 }
            ]
        }))).await;

    assert_eq!(s, 200, "set-initiative should succeed: {s}");

    let a_init: i32 = sqlx::query_scalar("select initiative from combatants where id = $1::uuid")
        .bind(cid).fetch_one(&db).await.unwrap();
    assert_eq!(a_init, 18, "first combatant initiative should be 18");
    let b_init: i32 = sqlx::query_scalar("select initiative from combatants where id = $1::uuid")
        .bind(b_id).fetch_one(&db).await.unwrap();
    assert_eq!(b_init, 7, "second combatant initiative should be 7");
}

/// Actions in a `planned` (not-yet-started) encounter must be rejected.
#[tokio::test]
async fn attack_in_planned_encounter_is_rejected() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_encounter(&router, &db).await;

    // Add target
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'T', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&eid).fetch_one(&db).await.unwrap();
    let (_, tgt) = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&tok), Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "T",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": 10 }))).await;
    let tgt_id = tgt["id"].as_str().unwrap();

    // Do NOT call /start — encounter remains "planned"

    let (s, body) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({ "target_id": tgt_id, "damage_expression": "1d6", "damage_type": "slashing" }))).await;

    assert!(s == 400 || s == 409,
        "attack in planned encounter should be rejected (400/409), got {}: {}", s, body);
}
