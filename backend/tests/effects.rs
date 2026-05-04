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

/// Creates campaign, encounter, adds one NPC combatant.
/// Returns (master_tok, encounter_id, combatant_id, campaign_id).
async fn setup_encounter(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@eff.test").await;
    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Effects Camp" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name) values ($1::uuid, 'Goblin') returning id")
        .bind(&cid).fetch_one(db).await.unwrap();

    let (_, enc) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok), Some(json!({ "name": "Fight" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, comb) = json_req(router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Goblin",
                     "initiative": 10, "hp_max": 10, "hp_current": 10, "ac": 12 }))).await;
    let combatant_id = comb["id"].as_str().unwrap().to_string();

    (master_tok, eid, combatant_id, cid)
}

#[tokio::test]
async fn apply_manual_effect_master_only() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, cid) = setup_encounter(&router, &db).await;
    let (player_tok, _) = register_with(&router, "pl@eff.test", Some(&master_tok)).await;
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
        Some(json!({ "email": "pl@eff.test", "role": "player" }))).await;

    let (s_forbid, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&player_tok),
        Some(json!({ "name": "Blessed", "kind": "buff", "duration_unit": "rounds", "duration_value": 3 }))).await;
    assert_eq!(s_forbid, 403);

    let (s, effect) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "Blessed", "kind": "buff", "duration_unit": "rounds", "duration_value": 3 }))).await;
    assert_eq!(s, 201, "{effect}");
    assert_eq!(effect["name"], "Blessed");
    assert_eq!(effect["kind"], "buff");
    assert!(effect["id"].is_string());
}

#[tokio::test]
async fn list_effects_on_encounter() {
    let (router, db) = skip_no_db!();
    let (master_tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "Stunned", "kind": "condition", "duration_unit": "rounds" }))).await;
    json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "Slowed", "kind": "debuff", "duration_unit": "rounds" }))).await;

    let (s, effects) = json_req(&router, "GET", &format!("/api/v1/encounters/{eid}/effects"),
        Some(&master_tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(effects.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn list_effects_on_combatant() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "Hexed", "kind": "debuff", "duration_unit": "hours" }))).await;

    let (s, effects) = json_req(&router, "GET", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok), None).await;
    assert_eq!(s, 200);
    assert_eq!(effects.as_array().unwrap().len(), 1);
    assert_eq!(effects[0]["name"], "Hexed");
}

#[tokio::test]
async fn update_effect_name_and_remaining() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    let (_, eff) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "OldName", "kind": "buff", "duration_unit": "rounds", "duration_value": 5, "remaining": 5 }))).await;
    let eff_id = eff["id"].as_str().unwrap();

    let (s, updated) = json_req(&router, "PATCH",
        &format!("/api/v1/combatants/{combatant_id}/effects/{eff_id}"),
        Some(&master_tok),
        Some(json!({ "name": "NewName", "remaining": 3 }))).await;
    assert_eq!(s, 200, "{updated}");
    assert_eq!(updated["name"], "NewName");
    assert_eq!(updated["remaining"], 3);
}

#[tokio::test]
async fn remove_effect() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    let (_, eff) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({ "name": "Temp", "kind": "neutral", "duration_unit": "permanent" }))).await;
    let eff_id = eff["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/combatants/{combatant_id}/effects/{eff_id}"),
        Some(&master_tok), None).await;
    assert_eq!(s, 204);

    let (_, effects) = json_req(&router, "GET", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok), None).await;
    assert_eq!(effects.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn concentration_second_effect_deactivates_first() {
    let (router, db) = skip_no_db!();
    let (master_tok, eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    let (_, enc_combs) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&master_tok), None).await;
    let caster_id = enc_combs[0]["id"].as_str().unwrap();

    let (_, eff1) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({
            "name": "Hold Person",
            "kind": "condition",
            "duration_unit": "rounds",
            "duration_value": 10,
            "concentration": true,
            "caster_combatant_id": caster_id
        }))).await;
    assert_eq!(eff1["concentration"], true);
    let eff1_id = eff1["id"].as_str().unwrap();

    let (s2, _eff2) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&master_tok),
        Some(json!({
            "name": "Hypnotic Pattern",
            "kind": "condition",
            "duration_unit": "rounds",
            "duration_value": 10,
            "concentration": true,
            "caster_combatant_id": caster_id
        }))).await;
    assert_eq!(s2, 201);

    // Hold Person should now be inactive
    let (_, effects) = json_req(&router, "GET", &format!("/api/v1/encounters/{eid}/effects"),
        Some(&master_tok), None).await;
    let arr = effects.as_array().unwrap();
    let hold = arr.iter().find(|e| e["id"].as_str() == Some(eff1_id)).unwrap();
    assert_eq!(hold["active"], false, "first concentration effect should be deactivated");
}

#[tokio::test]
async fn apply_spell_effect_requires_active_encounter_combatant() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, _cid) = setup_encounter(&router, &db).await;

    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source, effects)
         values ('bless', 'Bless', 1, 'Enchantment', array['Cleric'], 'desc', 'SRD',
                 '[{\"name\":\"Blessed\",\"kind\":\"buff\",\"icon\":\"star\",\"duration_unit\":\"rounds\",\"duration_value\":10,\"tick_trigger\":\"round_end\",\"concentration\":true,\"modifiers\":{\"attack_bonus\":4}}]'::jsonb)")
        .execute(&db).await.unwrap();

    let (s, effects) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{combatant_id}/effects/apply-spell"),
        Some(&master_tok),
        Some(json!({ "spell_slug": "bless" }))).await;
    assert_eq!(s, 201, "{effects}");
    assert!(effects.as_array().unwrap().len() >= 1);
    assert_eq!(effects[0]["source_spell_slug"], "bless");
}

#[tokio::test]
async fn non_member_cannot_apply_effects() {
    let (router, db) = skip_no_db!();
    let (master_tok, _eid, combatant_id, _cid) = setup_encounter(&router, &db).await;
    let (outsider_tok, _) = register_with(&router, "none@eff.test", Some(&master_tok)).await;

    let (s, _) = json_req(&router, "POST", &format!("/api/v1/combatants/{combatant_id}/effects"),
        Some(&outsider_tok),
        Some(json!({ "name": "Illegal", "kind": "buff", "duration_unit": "permanent" }))).await;
    assert_eq!(s, 403);
}
