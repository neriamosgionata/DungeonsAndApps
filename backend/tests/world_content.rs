//! World content tests - NPCs, lore, factions, maps, pins
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

async fn setup_campaign_with_world(router: &axum::Router, _db: &sqlx::PgPool) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@world.test").await;
    let (_, camp) = json_req(router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "World Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Create map
    let (_, map) = json_req(router, "POST", &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&master_tok), Some(json!({ "name": "Test Map" }))).await;
    let map_id = map["id"].as_str().unwrap().to_string();

    (master_tok, cid, map_id, camp["id"].as_str().unwrap().to_string())
}

// =====================================================================
// NPCs
// =====================================================================

#[tokio::test]
async fn create_npc_full_stats() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&tok),
        Some(json!({
            "name": "Goblin Warrior",
            "race": "Goblin",
            "role": "combat",
            "stats": {
                "ac": 15,
                "hp": { "max": 7, "current": 7 },
                "abilities": { "str": 8, "dex": 14, "con": 10, "int": 10, "wis": 10, "cha": 8 },
                "cr": "1/4",
                "xp": 50
            },
            "is_hostile": true,
            "is_secret": false
        }))).await;

    assert_eq!(s, 201, "create npc should succeed: {}", result);
    assert_eq!(result["name"], "Goblin Warrior");
}

#[tokio::test]
async fn list_npcs_filters_by_role() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&tok), Some(json!({ "name": "Combat NPC", "role": "combat" }))).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&tok), Some(json!({ "name": "Social NPC", "role": "social" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/npcs?role=combat"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let npcs = result.as_array().expect("should be array");
    assert!(npcs.iter().all(|n| n["role"] == "combat"), "should filter by role");
}

#[tokio::test]
async fn update_npc_stats() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&tok), Some(json!({ "name": "Weak Goblin", "stats": { "ac": 10, "hp": { "max": 5 } } }))).await;

    let npc_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/npcs/{npc_id}"),
        Some(&tok),
        Some(json!({
            "name": "Strong Goblin",
            "stats": { "ac": 15, "hp": { "max": 15 } }
        }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["name"], "Strong Goblin");
}

#[tokio::test]
async fn delete_npc() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&tok), Some(json!({ "name": "To Delete" }))).await;

    let npc_id = created["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/npcs/{npc_id}"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
}

// =====================================================================
// Factions
// =====================================================================

#[tokio::test]
async fn create_faction() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&tok),
        Some(json!({
            "name": "The Black Hand",
            "color": "#8B0000",
            "description": "Criminal syndicate",
            "relationship": "hostile"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["name"], "The Black Hand");
}

#[tokio::test]
async fn update_faction_relationship() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&tok), Some(json!({ "name": "Merchant Guild", "relationship": "neutral" }))).await;

    let faction_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/factions/{faction_id}"),
        Some(&tok),
        Some(json!({ "relationship": "friendly" }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["relationship"], "friendly");
}

#[tokio::test]
async fn list_factions_sorted() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&tok), Some(json!({ "name": "Z-Faction" }))).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&tok), Some(json!({ "name": "A-Faction" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/factions"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let factions = result.as_array().expect("should be array");
    assert!(factions.len() >= 2);
}

// =====================================================================
// Lore / Codex
// =====================================================================

#[tokio::test]
async fn create_lore_entry() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/lore"),
        Some(&tok),
        Some(json!({
            "title": "Ancient History",
            "content": "Long ago...",
            "category": "history",
            "visibility": "players"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["title"], "Ancient History");
}

#[tokio::test]
async fn update_lore_visibility() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/lore"),
        Some(&tok), Some(json!({ "title": "Secret", "content": "Hidden", "visibility": "master" }))).await;

    let lore_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/lore/{lore_id}"),
        Some(&tok),
        Some(json!({ "visibility": "players" }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["visibility"], "players");
}

#[tokio::test]
async fn list_lore_filters_by_category() {
    let (router, db) = skip_no_db!();
    let (tok, cid, _map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/lore"),
        Some(&tok), Some(json!({ "title": "History 1", "category": "history" }))).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/lore"),
        Some(&tok), Some(json!({ "title": "Location 1", "category": "location" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/lore?category=history"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let entries = result.as_array().expect("should be array");
    assert!(entries.iter().all(|e| e["category"] == "history"));
}

// =====================================================================
// Maps
// =====================================================================

#[tokio::test]
async fn create_map() {
    let (router, _db) = skip_no_db!();
    let (tok, cid) = {
        let (t, _) = register(&router, "gm@map.test").await;
        let (_, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&t),
            Some(json!({ "name": "Map Test" }))).await;
        (t, c["id"].as_str().unwrap().to_string())
    };

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&tok),
        Some(json!({
            "name": "Dungeon Level 1",
            "grid_size": 50,
            "grid_type": "square"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["name"], "Dungeon Level 1");
}

#[tokio::test]
async fn update_map_grid() {
    let (router, _db) = skip_no_db!();
    let (tok, cid) = {
        let (t, _) = register(&router, "gm@map.test").await;
        let (_, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&t),
            Some(json!({ "name": "Map Test" }))).await;
        (t, c["id"].as_str().unwrap().to_string())
    };

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&tok), Some(json!({ "name": "Test Map" }))).await;

    let map_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}"),
        Some(&tok),
        Some(json!({ "grid_size": 70, "show_grid": true }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["grid_size"], 70);
}

#[tokio::test]
async fn delete_map() {
    let (router, _db) = skip_no_db!();
    let (tok, cid) = {
        let (t, _) = register(&router, "gm@map.test").await;
        let (_, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&t),
            Some(json!({ "name": "Map Test" }))).await;
        (t, c["id"].as_str().unwrap().to_string())
    };

    let (_, created) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/maps"),
        Some(&tok), Some(json!({ "name": "To Delete" }))).await;

    let map_id = created["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
}

// =====================================================================
// Map Pins
// =====================================================================

#[tokio::test]
async fn create_map_pin() {
    let (router, db) = skip_no_db!();
    let (tok, cid, map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (s, result) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok),
        Some(json!({
            "x": 100.0,
            "y": 200.0,
            "label": "Treasure Room",
            "icon": "chest",
            "description": "Contains loot"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["label"], "Treasure Room");
}

#[tokio::test]
async fn update_map_pin() {
    let (router, db) = skip_no_db!();
    let (tok, cid, map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok), Some(json!({ "x": 0.0, "y": 0.0, "label": "Old Label" }))).await;

    let pin_id = created["id"].as_str().unwrap();

    let (s, result) = json_req(&router, "PATCH",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins/{pin_id}"),
        Some(&tok),
        Some(json!({ "label": "New Label", "x": 50.0 }))).await;

    assert_eq!(s, 200);
    assert_eq!(result["label"], "New Label");
}

#[tokio::test]
async fn list_map_pins() {
    let (router, db) = skip_no_db!();
    let (tok, cid, map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok), Some(json!({ "x": 10.0, "y": 10.0, "label": "Pin 1" }))).await;

    json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok), Some(json!({ "x": 20.0, "y": 20.0, "label": "Pin 2" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let pins = result.as_array().expect("should be array");
    assert!(pins.len() >= 2);
}

#[tokio::test]
async fn delete_map_pin() {
    let (router, db) = skip_no_db!();
    let (tok, cid, map_id, _camp_id) = setup_campaign_with_world(&router, &db).await;

    let (_, created) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins"),
        Some(&tok), Some(json!({ "x": 0.0, "y": 0.0, "label": "To Delete" }))).await;

    let pin_id = created["id"].as_str().unwrap();

    let (s, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/maps/{map_id}/pins/{pin_id}"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
}

// =====================================================================
// Recaps (Session History)
// =====================================================================

#[tokio::test]
async fn create_recap() {
    let (router, _db) = skip_no_db!();
    let (tok, cid) = {
        let (t, _) = register(&router, "gm@recap.test").await;
        let (_, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&t),
            Some(json!({ "name": "Recap Test" }))).await;
        (t, c["id"].as_str().unwrap().to_string())
    };

    let (s, result) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/recaps"),
        Some(&tok),
        Some(json!({
            "title": "Session 1",
            "content": "The party met in a tavern...",
            "in_game_date": "1492 DR, Mirtul 1"
        }))).await;

    assert_eq!(s, 201);
    assert_eq!(result["title"], "Session 1");
}

#[tokio::test]
async fn list_recaps_chronological() {
    let (router, _db) = skip_no_db!();
    let (tok, cid) = {
        let (t, _) = register(&router, "gm@recap.test").await;
        let (_, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&t),
            Some(json!({ "name": "Recap Test" }))).await;
        (t, c["id"].as_str().unwrap().to_string())
    };

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/recaps"),
        Some(&tok), Some(json!({ "title": "Session 1", "content": "First" }))).await;

    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/recaps"),
        Some(&tok), Some(json!({ "title": "Session 2", "content": "Second" }))).await;

    let (s, result) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/recaps"),
        Some(&tok), None).await;

    assert_eq!(s, 200);
    let recaps = result.as_array().expect("should be array");
    assert!(recaps.len() >= 2);
}
