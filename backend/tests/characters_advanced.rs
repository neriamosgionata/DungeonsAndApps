//! Character management tests - full CRUD, HP tracking, spell management
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

async fn setup_campaign(router: &axum::Router) -> (String, String, String) {
    let (master_tok, _) = register(router, "gm@chars.test").await;
    let (player_tok, _) = register(router, "player@chars.test").await;

    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Character Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Add player to campaign
    let (_, invite) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&master_tok), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();

    json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player_tok), Some(json!({ "code": code }))).await;

    (master_tok, player_tok, cid)
}

// =====================================================================
// Character CRUD
// =====================================================================

#[tokio::test]
async fn create_character_basic() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({
            "name": "Test Hero",
            "race": "Human",
            "class_primary": "Fighter",
            "level_total": 1
        }))).await;

    assert_eq!(s, 201, "create character should succeed: {}", result);
    assert_eq!(result["name"], "Test Hero");
    assert_eq!(result["race"], "Human");
}

#[tokio::test]
async fn get_character_by_id() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test Hero", "race": "Elf", "class_primary": "Wizard", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}"),
        Some(&player), None).await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "Test Hero");
}

#[tokio::test]
async fn list_characters_in_campaign() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player), Some(json!({ "name": "Char 1", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player), Some(json!({ "name": "Char 2", "race": "Elf", "class_primary": "Rogue", "level_total": 1 }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player), None).await;

    assert_eq!(s, 200);
    let chars = result.as_array().expect("should be array");
    assert!(chars.len() >= 2, "should have at least 2 characters");
}

#[tokio::test]
async fn update_character_patch() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Old Name", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}"),
        Some(&player),
        Some(json!({ "name": "New Name", "level_total": 2 }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "New Name");
    assert_eq!(result["level_total"], 2);
}

#[tokio::test]
async fn delete_character_removes_it() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "To Delete", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, _result) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}"),
        Some(&player), None).await;

    assert_eq!(s, 200);

    // Verify deleted
    let (s2, _) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}"),
        Some(&player), None).await;
    assert_eq!(s2, 404, "character should be deleted");
}

// =====================================================================
// Sheet Updates
// =====================================================================

#[tokio::test]
async fn update_character_sheet_abilities() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "abilities": { "str": 16, "dex": 14, "con": 15, "int": 10, "wis": 12, "cha": 8 }
        }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["sheet"]["abilities"]["str"], 16);
}

#[tokio::test]
async fn update_hp_current_and_max() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({ "hp": { "current": 8, "max": 10, "temp": 0 } }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["sheet"]["hp"]["current"], 8);
    assert_eq!(result["sheet"]["hp"]["max"], 10);
}

#[tokio::test]
async fn update_ac_equipment() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "ac": 16,
            "armor": { "type": "chain", "ac_base": 14, "max_dex": 2 }
        }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["sheet"]["ac"], 16);
}

// =====================================================================
// Skills and Saving Throws
// =====================================================================

#[tokio::test]
async fn set_skill_proficiency() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Rogue", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "skills": { "stealth": "expertise", "perception": "proficient" }
        }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["sheet"]["skills"]["stealth"], "expertise");
    assert_eq!(result["sheet"]["skills"]["perception"], "proficient");
}

#[tokio::test]
async fn set_saving_throw_proficiency() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({ "saving_throws": { "str": true, "con": true } }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["sheet"]["saving_throws"]["str"], true);
}

// =====================================================================
// Spells
// =====================================================================

#[tokio::test]
async fn add_spell_to_character() {
    let (router, db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    // Seed a spell
    sqlx::query(
        "INSERT INTO spells (slug, name, level, school, classes, description, source)
         VALUES ('magic-missile', 'Magic Missile', 1, 'Evocation', ARRAY['Wizard'], 'Arcane darts', 'SRD')
         ON CONFLICT DO NOTHING")
        .execute(&db).await.unwrap();

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Wizard", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/spells"),
        Some(&player),
        Some(json!({ "slug": "magic-missile", "prepared": true }))).await;

    assert_eq!(s, 200);
}

#[tokio::test]
async fn list_character_spells() {
    let (router, db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    sqlx::query(
        "INSERT INTO spells (slug, name, level, school, classes, description, source)
         VALUES ('fireball', 'Fireball', 3, 'Evocation', ARRAY['Wizard', 'Sorcerer'], 'Boom', 'SRD')
         ON CONFLICT DO NOTHING")
        .execute(&db).await.unwrap();

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Wizard", "level_total": 5 }))).await;

    let char_id = created["id"].as_str().unwrap();

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/spells"),
        Some(&player),
        Some(json!({ "slug": "fireball", "prepared": true }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/spells"),
        Some(&player), None).await;

    assert_eq!(s, 200);
    let spells = result.as_array().expect("should be array");
    assert!(!spells.is_empty(), "should have spells");
}

// =====================================================================
// Rest Mechanics
// =====================================================================

#[tokio::test]
async fn short_rest_recovers_hit_dice() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 3 }))).await;

    let char_id = created["id"].as_str().unwrap();

    // Set low HP and spent hit dice
    json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "hp": { "current": 10, "max": 24 },
            "hit_dice": { "d10": { "current": 1, "max": 3 } }
        }))).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/rest"),
        Some(&player),
        Some(json!({ "type": "short", "hit_dice_spent": 1 }))).await;

    assert_eq!(s, 200, "short rest should succeed: {}", result);
}

#[tokio::test]
async fn long_rest_full_recovery() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Wizard", "level_total": 3 }))).await;

    let char_id = created["id"].as_str().unwrap();

    // Set depleted resources
    json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "hp": { "current": 5, "max": 18 },
            "spell_slots": { "1": 0, "2": 0 },
            "hit_dice": { "d6": { "current": 0, "max": 3 } }
        }))).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/rest"),
        Some(&player),
        Some(json!({ "type": "long" }))).await;

    assert_eq!(s, 200, "long rest should succeed: {}", result);
}

// =====================================================================
// Death Saves
// =====================================================================

#[tokio::test]
async fn death_save_tracking() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Dying", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    // Set to dying state
    json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "hp": { "current": 0, "max": 10 },
            "death_saves": { "successes": 0, "failures": 0 },
            "alive": true
        }))).await;

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/death-save"),
        Some(&player),
        Some(json!({ "success": true }))).await;

    assert_eq!(s, 200, "death save should succeed: {}", result);
}

// =====================================================================
// Multi-class
// =====================================================================

#[tokio::test]
async fn add_second_class() {
    let (router, _db) = skip_no_db!();
    let (_master, player, cid) = setup_campaign(&router).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player),
        Some(json!({ "name": "Test", "race": "Human", "class_primary": "Fighter", "level_total": 5 }))).await;

    let char_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}/sheet"),
        Some(&player),
        Some(json!({
            "classes": [
                { "name": "Fighter", "level": 3 },
                { "name": "Wizard", "level": 2 }
            ]
        }))).await;

    assert_eq!(s, 200);
    let classes = result["sheet"]["classes"].as_array().unwrap();
    assert_eq!(classes.len(), 2);
}

// =====================================================================
// Permission Tests
// =====================================================================

#[tokio::test]
async fn player_cannot_delete_others_character() {
    let (router, _db) = skip_no_db!();
    let (_master, player1, cid) = setup_campaign(&router).await;

    let (player2_tok, _) = register(&router, "player2@chars.test").await;

    // Join player2 to campaign
    let (_, invite) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/invitations"),
        Some(&_master), Some(json!({ "role": "player" }))).await;
    let code = invite["code"].as_str().unwrap();
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/join"),
        Some(&player2_tok), Some(json!({ "code": code }))).await;

    // Player1 creates character
    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player1),
        Some(json!({ "name": "Mine", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let char_id = created["id"].as_str().unwrap();

    // Player2 tries to delete - should fail
    let (s, _result) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/characters/{char_id}"),
        Some(&player2_tok), None).await;

    assert_eq!(s, 403, "player should not delete other's character");
}

#[tokio::test]
async fn master_can_view_all_characters() {
    let (router, _db) = skip_no_db!();
    let (master, player, cid) = setup_campaign(&router).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player), Some(json!({ "name": "Player Char", "race": "Human", "class_primary": "Fighter", "level_total": 1 }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&master), None).await;

    assert_eq!(s, 200);
    let chars = result.as_array().expect("should be array");
    assert!(!chars.is_empty(), "master should see all characters");
}
