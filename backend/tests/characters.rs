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
    let (master_tok, _) = register(router, "gm@char.test").await;
    let (player_tok, _) = register_with(router, "pl@char.test", Some(&master_tok)).await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "CharCamp" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();
    json_req(
        router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok),
        Some(json!({ "email": "pl@char.test", "role": "player" })),
    )
    .await;
    (master_tok, player_tok, cid)
}

#[tokio::test]
async fn create_character_in_own_campaign() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    let (s, body) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Aela", "race": "Human", "level_total": 3 })),
    )
    .await;
    assert_eq!(s, 201, "{body}");
    assert_eq!(body["name"], "Aela");
    assert_eq!(body["level_total"], 3);
    assert!(body["id"].is_string());

    let (s2, body2) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&master_tok),
        Some(json!({ "name": "Npc Hero", "level_total": 5 })),
    )
    .await;
    assert_eq!(s2, 201, "{body2}");
}

#[tokio::test]
async fn create_character_not_member_403() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _, cid) = setup(&router).await;
    let (outsider_tok, _) = register_with(&router, "out@char.test", Some(&master_tok)).await;

    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&outsider_tok),
        Some(json!({ "name": "Ghost" })),
    )
    .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn list_characters_player_sees_own_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Player PC" })),
    )
    .await;
    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&master_tok),
        Some(json!({ "name": "Master NPC" })),
    )
    .await;

    let (s, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(list.as_array().unwrap().len(), 1);
    assert_eq!(list[0]["name"], "Player PC");

    let (s2, master_list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&master_tok),
        None,
    )
    .await;
    assert_eq!(s2, 200);
    assert_eq!(master_list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn read_character_sheet() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, created) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Bilbo", "level_total": 2 })),
    )
    .await;
    let cid_char = created["id"].as_str().unwrap();

    let (s, body) = json_req(
        &router,
        "GET",
        &format!("/api/v1/characters/{cid_char}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(body["name"], "Bilbo");
    assert!(body["sheet"].is_object());
}

#[tokio::test]
async fn update_sheet_owner_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Rogue" })),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, updated) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/characters/{char_id}"),
        Some(&player_tok),
        Some(json!({ "sheet": { "hp": { "max": 20, "current": 20 } } })),
    )
    .await;
    assert_eq!(s, 200);
    assert_eq!(updated["sheet"]["hp"]["max"], 20);

    let (s2, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/characters/{char_id}"),
        Some(&master_tok),
        Some(json!({ "sheet": { "hp": { "max": 99 } } })),
    )
    .await;
    assert_eq!(s2, 403);
}

#[tokio::test]
async fn delete_character_owner_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Doomed" })),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s_forbid, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/characters/{char_id}"),
        Some(&master_tok),
        None,
    )
    .await;
    assert_eq!(s_forbid, 403);

    let (s_ok, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/characters/{char_id}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s_ok, 204);

    let (s_gone, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/characters/{char_id}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s_gone, 404);
}

#[tokio::test]
async fn short_rest_rolls_hit_dice_and_recovers_hp() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Fighter", "sheet": {
            "hp": { "max": 30, "current": 10 },
            "hit_dice": { "die": "d10", "max": 5, "current": 5 },
            "abilities": { "con": 14 }
        }})),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/short-rest"),
        Some(&player_tok),
        Some(json!({ "hit_dice_spent": 2 })),
    )
    .await;
    assert_eq!(s, 200, "{result}");
    assert_eq!(result["hp_before"], 10);
    assert_eq!(result["hit_dice_before"], 5);
    assert_eq!(result["hit_dice_after"], 3);
    assert!(result["hp_after"].as_i64().unwrap() >= 10);
    assert!(result["hp_after"].as_i64().unwrap() <= 30);
    assert!(result["roll_total"].as_i64().unwrap() >= 2);
    assert_eq!(result["con_mod"], 2);
}

#[tokio::test]
async fn short_rest_cannot_exceed_hp_max() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Paladin", "sheet": {
            "hp": { "max": 20, "current": 19 },
            "hit_dice": { "die": "d10", "max": 5, "current": 3 },
            "abilities": { "con": 18 }
        }})),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/short-rest"),
        Some(&player_tok),
        Some(json!({ "hit_dice_spent": 2 })),
    )
    .await;
    assert_eq!(s, 200, "{result}");
    assert!(result["hp_after"].as_i64().unwrap() <= 20);
    assert_eq!(result["hp_max"], 20);
}

#[tokio::test]
async fn short_rest_cannot_spend_more_hit_dice_than_available() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Barbarian", "sheet": {
            "hp": { "max": 40, "current": 5 },
            "hit_dice": { "die": "d12", "max": 5, "current": 1 }
        }})),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, body) = json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/short-rest"),
        Some(&player_tok),
        Some(json!({ "hit_dice_spent": 3 })),
    )
    .await;
    assert_eq!(s, 400, "{body}");
}

#[tokio::test]
async fn long_rest_restores_hp_and_recovers_half_hit_dice() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Monk", "sheet": {
            "hp": { "max": 25, "current": 5 },
            "hit_dice": { "die": "d8", "max": 6, "current": 2 },
            "exhaustion": 2,
            "slots": {
                "1": { "max": 4, "current": 0 },
                "2": { "max": 3, "current": 1 }
            }
        }})),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/long-rest"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s, 200, "{result}");
    assert_eq!(result["hp_after"], 25);
    assert_eq!(result["hp_before"], 5);
    assert_eq!(result["exhaustion_before"], 2);
    assert_eq!(result["exhaustion_after"], 1);
    assert_eq!(result["hit_dice_max"], 6);
    // ceil(6/2)=3; 2+3=5, capped at 6
    assert_eq!(result["hit_dice_after"], 5);
}

#[tokio::test]
async fn long_rest_resets_spell_slots() {
    let (router, _db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Wizard", "sheet": {
            "hp": { "max": 15, "current": 15 },
            "hit_dice": { "die": "d6", "max": 3, "current": 3 },
            "slots": {
                "1": { "max": 4, "current": 0 },
                "2": { "max": 3, "current": 0 }
            }
        }})),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/long-rest"),
        Some(&player_tok),
        None,
    )
    .await;

    let (_, updated) = json_req(
        &router,
        "GET",
        &format!("/api/v1/characters/{char_id}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(updated["sheet"]["slots"]["1"]["current"], 4);
    assert_eq!(updated["sheet"]["slots"]["2"]["current"], 3);
}

#[tokio::test]
async fn award_xp_master_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Hero", "level_total": 1 })),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s_forbid, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/award-xp"),
        Some(&player_tok),
        Some(json!({ "character_ids": [char_id], "xp_each": 300 })),
    )
    .await;
    assert_eq!(s_forbid, 403);

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/award-xp"),
        Some(&master_tok),
        Some(json!({ "character_ids": [char_id], "xp_each": 300, "reason": "Slew goblins" })),
    )
    .await;
    assert_eq!(s, 200, "{result}");
    let awarded = &result["characters_awarded"][0];
    assert_eq!(awarded["xp_gained"], 300);
    assert_eq!(awarded["xp_after"], 300);
    assert_eq!(awarded["leveled_up"], true);
}

#[tokio::test]
async fn award_xp_level_up_triggers() {
    let (router, _db) = skip_no_db!();
    let (master_tok, player_tok, cid) = setup(&router).await;

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Levelup", "level_total": 1, "sheet": { "xp": 250 } })),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/award-xp"),
        Some(&master_tok),
        Some(json!({ "character_ids": [char_id], "xp_each": 100 })),
    )
    .await;
    assert_eq!(s, 200, "{result}");
    let entry = &result["characters_awarded"][0];
    assert_eq!(entry["leveled_up"], true);
    assert_eq!(entry["new_level"], 2);
}

#[tokio::test]
async fn spell_list_crud() {
    let (router, db) = skip_no_db!();
    let (_, player_tok, cid) = setup(&router).await;

    let spell_id: uuid::Uuid = sqlx::query_scalar(
        "insert into spells (slug, name, level, school, classes, description, source)
         values ('mage-hand', 'Mage Hand', 0, 'Conjuration', array['Wizard'], 'cantrip', 'SRD')
         on conflict (slug) do nothing
         returning id",
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let (_, c) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Spellbook" })),
    )
    .await;
    let char_id = c["id"].as_str().unwrap();

    let (s_add, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/characters/{char_id}/spells"),
        Some(&player_tok),
        Some(json!({ "spell_id": spell_id, "prepared": true })),
    )
    .await;
    assert_eq!(s_add, 204);

    let (s_list, spells) = json_req(
        &router,
        "GET",
        &format!("/api/v1/characters/{char_id}/spells"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s_list, 200);
    assert_eq!(spells.as_array().unwrap().len(), 1);
    assert_eq!(spells[0]["slug"], "mage-hand");
    assert_eq!(spells[0]["prepared"], true);

    let (s_patch, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/characters/{char_id}/spells/{spell_id}"),
        Some(&player_tok),
        Some(json!({ "prepared": false, "notes": Some("save for emergencies") })),
    )
    .await;
    assert_eq!(s_patch, 204);

    let (s_del, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/characters/{char_id}/spells/{spell_id}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s_del, 204);

    let (_, after_del) = json_req(
        &router,
        "GET",
        &format!("/api/v1/characters/{char_id}/spells"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(after_del.as_array().unwrap().len(), 0);
}
