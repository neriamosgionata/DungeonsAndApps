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

#[tokio::test]
async fn auth_flow_register_login_me() {
    let (router, _db) = skip_no_db!();

    let (s, body) = json_req(&router, "POST", "/api/v1/auth/register", None, Some(json!({
        "email": "alice@ex.com", "password": "password123", "display_name": "Alice",
    }))).await;
    assert_eq!(s, 201, "register: {body}");
    let token = body["token"].as_str().unwrap().to_string();

    // duplicate -> conflict (alice is master, has token)
    let (s2, _) = json_req(&router, "POST", "/api/v1/auth/register", Some(&token), Some(json!({
        "email": "alice@ex.com", "password": "password123", "display_name": "Alice",
    }))).await;
    assert_eq!(s2, 409);

    // non-master caller cannot register a 3rd user: first register bob via alice,
    // then attempt to register eve as bob -> 403
    let (_, bob) = json_req(&router, "POST", "/api/v1/auth/register", Some(&token), Some(json!({
        "email": "bob@ex.com", "password": "password123", "display_name": "Bob",
    }))).await;
    let bob_tok = bob["token"].as_str().unwrap();
    let (s_bob, _) = json_req(&router, "POST", "/api/v1/auth/register", Some(bob_tok), Some(json!({
        "email": "eve@ex.com", "password": "password123", "display_name": "Eve",
    }))).await;
    assert_eq!(s_bob, 403);

    // no token + already-bootstrapped -> 401
    let (s_anon, _) = json_req(&router, "POST", "/api/v1/auth/register", None, Some(json!({
        "email": "mal@ex.com", "password": "password123", "display_name": "Mal",
    }))).await;
    assert_eq!(s_anon, 401);

    // login OK
    let (s3, body3) = json_req(&router, "POST", "/api/v1/auth/login", None, Some(json!({
        "email": "alice@ex.com", "password": "password123",
    }))).await;
    assert_eq!(s3, 200);
    assert!(body3["token"].is_string());

    // login bad pw -> 401
    let (s4, _) = json_req(&router, "POST", "/api/v1/auth/login", None, Some(json!({
        "email": "alice@ex.com", "password": "wrong",
    }))).await;
    assert_eq!(s4, 401);

    // me
    let (s5, body5) = json_req(&router, "GET", "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(s5, 200);
    assert_eq!(body5["email"], "alice@ex.com");

    // no token -> 401
    let (s6, _) = json_req(&router, "GET", "/api/v1/auth/me", None, None).await;
    assert_eq!(s6, 401);
}

#[tokio::test]
async fn campaigns_and_characters_roles() {
    let (router, _db) = skip_no_db!();
    let (master_tok, master_id, player_tok, _player_id) =
        bootstrap_two(&router, "m@e.com", "p@e.com").await;
    let m = serde_json::json!({ "user": { "id": master_id.clone() } });
    let master_id = m["user"]["id"].as_str().unwrap().to_string();
    let _ = player_tok;

    // master creates campaign
    let (s, c) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Curse of Strahd" }))).await;
    assert_eq!(s, 201);
    let campaign_id = c["id"].as_str().unwrap().to_string();
    assert_eq!(c["master_id"], master_id);

    // player not yet a member -> 403
    let (s2, _) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{campaign_id}"), Some(&player_tok), None).await;
    assert_eq!(s2, 403);

    // master adds player
    let (s3, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{campaign_id}/members"), Some(&master_tok),
        Some(json!({ "email": "p@e.com", "role": "player" }))).await;
    assert_eq!(s3, 201);

    // player now sees campaign
    let (s4, _) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{campaign_id}"), Some(&player_tok), None).await;
    assert_eq!(s4, 200);

    // player creates own character
    let (s5, char_body) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{campaign_id}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Thorgrim", "race": "Dwarf", "class_primary": "Fighter", "level_total": 3 }))).await;
    assert_eq!(s5, 201, "create char: {char_body}");
    let char_id = char_body["id"].as_str().unwrap().to_string();
    assert_eq!(char_body["level_total"], 3);

    // player updates own sheet
    let (s6, upd) = json_req(&router, "PATCH",
        &format!("/api/v1/characters/{char_id}"),
        Some(&player_tok),
        Some(json!({ "sheet": { "hp": { "max": 28, "current": 28 }, "abilities": { "str": 16 } } }))).await;
    assert_eq!(s6, 200);
    assert_eq!(upd["sheet"]["hp"]["max"], 28);

    // player cannot delete master's campaign
    let (s7, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{campaign_id}"), Some(&player_tok), None).await;
    assert_eq!(s7, 403);

    // list characters: player sees only own
    let (s8, list) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{campaign_id}/characters"),
        Some(&player_tok), None).await;
    assert_eq!(s8, 200);
    assert_eq!(list.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn user_management_master_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, master_id, player_tok, player_id) =
        bootstrap_two(&router, "gm@um.com", "pl@um.com").await;

    // player cannot list users
    let (s0, _) = json_req(&router, "GET", "/api/v1/users", Some(&player_tok), None).await;
    assert_eq!(s0, 403);

    // master lists users
    let (s1, list) = json_req(&router, "GET", "/api/v1/users", Some(&master_tok), None).await;
    assert_eq!(s1, 200);
    assert_eq!(list.as_array().unwrap().len(), 2);

    // player cannot delete anyone
    let (s2, _) = json_req(&router, "DELETE",
        &format!("/api/v1/users/{master_id}"), Some(&player_tok), None).await;
    assert_eq!(s2, 403);

    // master cannot delete themselves
    let (s3, _) = json_req(&router, "DELETE",
        &format!("/api/v1/users/{master_id}"), Some(&master_tok), None).await;
    assert_eq!(s3, 400);

    // cannot demote the sole master (player is role=user, so this targets master)
    let (s_demote, _) = json_req(&router, "PATCH",
        &format!("/api/v1/users/{master_id}"), Some(&master_tok),
        Some(json!({ "role": "user" }))).await;
    assert_eq!(s_demote, 400);

    // master resets player password
    let (s4, _) = json_req(&router, "POST",
        &format!("/api/v1/users/{player_id}/reset-password"), Some(&master_tok),
        Some(json!({ "new_password": "brand-new-pw-1" }))).await;
    assert_eq!(s4, 204);

    // player's old password no longer logs in
    let (s_old, _) = json_req(&router, "POST", "/api/v1/auth/login", None,
        Some(json!({ "email": "pl@um.com", "password": "password123" }))).await;
    assert_eq!(s_old, 401);

    // new password works
    let (s_new, _) = json_req(&router, "POST", "/api/v1/auth/login", None,
        Some(json!({ "email": "pl@um.com", "password": "brand-new-pw-1" }))).await;
    assert_eq!(s_new, 200);

    // master deletes player
    let (s5, _) = json_req(&router, "DELETE",
        &format!("/api/v1/users/{player_id}"), Some(&master_tok), None).await;
    assert_eq!(s5, 204);

    // idempotent: deleting again → 404
    let (s6, _) = json_req(&router, "DELETE",
        &format!("/api/v1/users/{player_id}"), Some(&master_tok), None).await;
    assert_eq!(s6, 404);
}

#[tokio::test]
async fn member_management_master_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _, player_tok, player_id) =
        bootstrap_two(&router, "gm@mm.com", "pl@mm.com").await;
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Roster" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // player isn't in campaign yet -> 403 listing members
    let (s0, _) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/members"), Some(&player_tok), None).await;
    assert_eq!(s0, 403);

    // master adds player
    let (s1, _) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok), Some(json!({ "email": "pl@mm.com", "role": "player" }))).await;
    assert_eq!(s1, 201);

    // player can now see members
    let (s2, list) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/members"), Some(&player_tok), None).await;
    assert_eq!(s2, 200);
    assert_eq!(list.as_array().unwrap().len(), 2);

    // player cannot remove anyone
    let (s3, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"), Some(&player_tok), None).await;
    assert_eq!(s3, 403);

    // master removes player -> 204
    let (s4, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/members/{player_id}"), Some(&master_tok), None).await;
    assert_eq!(s4, 204);

    // cannot remove campaign master themselves
    let (_, me) = json_req(&router, "GET", "/api/v1/auth/me", Some(&master_tok), None).await;
    let mid = me["id"].as_str().unwrap();
    let (s5, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/members/{mid}"), Some(&master_tok), None).await;
    assert_eq!(s5, 400);
}

#[tokio::test]
async fn multiple_characters_per_player_allowed() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _mid, player_tok, _pid) =
        bootstrap_two(&router, "gm@ch.com", "pl@ch.com").await;
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "multi-char" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
        Some(json!({ "email": "pl@ch.com", "role": "player" }))).await;

    // first character OK
    let (s1, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Hero", "level_total": 1 }))).await;
    assert_eq!(s1, 201);

    // second character (original died, player re-enters) also OK
    let (s2, body) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok),
        Some(json!({ "name": "Heir", "level_total": 1 }))).await;
    assert_eq!(s2, 201, "expected second char to be allowed, got: {body}");

    // player sees both of their characters
    let (_, list) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok), None).await;
    assert_eq!(list.as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn only_master_can_create_campaigns() {
    let (router, _db) = skip_no_db!();
    let (_master_tok, _master_id, player_tok, _player_id) =
        bootstrap_two(&router, "m@c.com", "p@c.com").await;

    // player (role=user) tries to create a campaign → 403
    let (s, _) = json_req(&router, "POST", "/api/v1/campaigns", Some(&player_tok),
        Some(json!({ "name": "Nope" }))).await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn dice_clear_master_only() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _master_id, player_tok, _player_id) =
        bootstrap_two(&router, "gm@d.com", "pl@d.com").await;
    // master creates campaign + adds player
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Dice Clear" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
        Some(json!({ "email": "pl@d.com", "role": "player" }))).await;

    // both roll
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/dice"), Some(&master_tok),
        Some(json!({ "expression": "1d20" }))).await;
    json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/dice"), Some(&player_tok),
        Some(json!({ "expression": "1d6" }))).await;

    // player cannot clear -> 403
    let (s_forbid, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&player_tok), None).await;
    assert_eq!(s_forbid, 403);

    // master clears -> 204
    let (s_ok, _) = json_req(&router, "DELETE",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&master_tok), None).await;
    assert_eq!(s_ok, 204);

    // history now empty
    let (_, hist) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&master_tok), None).await;
    assert_eq!(hist.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn dice_rolls_persist_and_scope() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "roller@e.com").await;
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&tok),
        Some(json!({ "name": "Dice Test" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // valid roll
    let (s, body) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&tok),
        Some(json!({ "expression": "1d20+5", "label": "attack" }))).await;
    assert_eq!(s, 201, "dice: {body}");
    let total = body["total"].as_i64().unwrap();
    assert!(total >= 6 && total <= 25);

    // invalid expression
    let (s2, _) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&tok),
        Some(json!({ "expression": "gibberish" }))).await;
    assert_eq!(s2, 400);

    // history
    let (s3, hist) = json_req(&router, "GET",
        &format!("/api/v1/campaigns/{cid}/dice"), Some(&tok), None).await;
    assert_eq!(s3, 200);
    assert_eq!(hist.as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn combat_full_flow() {
    let (router, db) = skip_no_db!();
    let (tok, _) = register(&router, "gm@e.com").await;
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&tok),
        Some(json!({ "name": "Battle" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // create an npc directly (no npc endpoint yet, use raw sql)
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name) values ($1::uuid, 'Goblin') returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();

    // create encounter
    let (s, enc) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"), Some(&tok),
        Some(json!({ "name": "Ambush" }))).await;
    assert_eq!(s, 201);
    let eid = enc["id"].as_str().unwrap().to_string();
    assert_eq!(enc["status"], "planned");

    // add combatants
    let (s2, _) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Goblin 1",
                     "initiative": 14, "hp_max": 7, "hp_current": 7, "ac": 15 }))).await;
    assert_eq!(s2, 201);
    let (s3, _) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Goblin 2",
                     "initiative": 8, "hp_max": 7, "hp_current": 7, "ac": 15 }))).await;
    assert_eq!(s3, 201);

    // start encounter -> turn_order assigned by initiative desc
    let (s4, started) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/start"), Some(&tok), None).await;
    assert_eq!(s4, 200);
    assert_eq!(started["status"], "active");
    assert_eq!(started["round"], 1);

    // list combatants ordered
    let (_, combs) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&tok), None).await;
    let arr = combs.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    assert!(arr[0]["initiative"].as_i64().unwrap() >= arr[1]["initiative"].as_i64().unwrap());

    // next turn x2 -> new round
    let (_, t1) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"), Some(&tok), None).await;
    assert_eq!(t1["turn_index"], 1);
    let (_, t2) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/next-turn"), Some(&tok), None).await;
    assert_eq!(t2["turn_index"], 0);
    assert_eq!(t2["round"], 2);

    // end
    let (_, ended) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/end"), Some(&tok), None).await;
    assert_eq!(ended["status"], "ended");
}

// Cannot start encounter while a player character hasn't rolled initiative.
#[tokio::test]
async fn combat_start_blocked_until_all_rolled() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "gm2@e.com").await;
    let (player_tok, player) = register(&router, "p@e.com").await;
    let player_id = player["user"]["id"].as_str().unwrap().to_string();
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Blocked" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();
    let _ = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok), Some(json!({ "user_id": player_id, "role": "player" }))).await;

    // Player creates a character → auto-added to encounter as pending
    let (_, ch) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/characters"),
        Some(&player_tok), Some(json!({ "name": "Hero" }))).await;
    let char_id = ch["id"].as_str().unwrap().to_string();

    let (_, enc) = json_req(&router, "POST", &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok), Some(json!({ "name": "Fight" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();

    // Attempt to start before player rolled → 400
    let (s_blocked, _) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/start"), Some(&master_tok), None).await;
    assert_eq!(s_blocked, 400);

    // Player rolls initiative
    let _ = json_req(&router, "POST", &format!("/api/v1/encounters/{eid}/set-initiative"),
        Some(&player_tok), Some(json!({ "character_id": char_id, "initiative": 12 }))).await;

    // Now master can start
    let (s_ok, started) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/start"), Some(&master_tok), None).await;
    assert_eq!(s_ok, 200, "should start after all rolled: {started}");
    assert_eq!(started["status"], "active");
}

// Battle map: master uploads map image, master and players move tokens,
// a non-owner player cannot move someone else's token.
#[tokio::test]
async fn combat_battle_map_tokens() {
    let (router, db) = skip_no_db!();
    let (master_tok, master) = register(&router, "dm@map.e").await;
    let master_id = master["user"]["id"].as_str().unwrap().to_string();
    let (_, camp) = json_req(&router, "POST", "/api/v1/campaigns", Some(&master_tok),
        Some(json!({ "name": "Map Battle" }))).await;
    let cid = camp["id"].as_str().unwrap().to_string();

    // Two players join
    let (alice_tok, alice) = register(&router, "alice@map.e").await;
    let alice_id = alice["user"]["id"].as_str().unwrap().to_string();
    let (bob_tok, bob)     = register(&router, "bob@map.e").await;
    let bob_id   = bob["user"]["id"].as_str().unwrap().to_string();
    for uid in [&alice_id, &bob_id] {
        let (_, _) = json_req(&router, "POST",
            &format!("/api/v1/campaigns/{cid}/members"), Some(&master_tok),
            Some(json!({ "user_id": uid, "role": "player" }))).await;
    }

    // Each player creates a character
    let (_, alice_ch) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"), Some(&alice_tok),
        Some(json!({ "name": "Alice PC" }))).await;
    let alice_char = alice_ch["id"].as_str().unwrap().to_string();
    let (_, bob_ch) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/characters"), Some(&bob_tok),
        Some(json!({ "name": "Bob PC" }))).await;
    let bob_char = bob_ch["id"].as_str().unwrap().to_string();

    // Create encounter (auto-adds both PCs as pending combatants)
    let (_, enc) = json_req(&router, "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"), Some(&master_tok),
        Some(json!({ "name": "Skirmish" }))).await;
    let eid = enc["id"].as_str().unwrap().to_string();
    assert_eq!(enc["map_grid_size"], 50);
    assert!(enc["map_image"].is_null());

    // Master uploads a battle map image and changes grid size
    let (s, updated) = json_req(&router, "PATCH",
        &format!("/api/v1/encounters/{eid}"), Some(&master_tok),
        Some(json!({ "map_image": "https://example/map.png", "map_grid_size": 64 }))).await;
    assert_eq!(s, 200);
    assert_eq!(updated["map_image"], "https://example/map.png");
    assert_eq!(updated["map_grid_size"], 64);

    // Find each PC's combatant row
    let (_, combs) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&master_tok), None).await;
    let arr = combs.as_array().unwrap();
    let alice_c = arr.iter().find(|c| c["character_id"].as_str() == Some(&alice_char)).unwrap()["id"].as_str().unwrap().to_string();
    let bob_c   = arr.iter().find(|c| c["character_id"].as_str() == Some(&bob_char)).unwrap()["id"].as_str().unwrap().to_string();

    // Alice moves her own token
    let (s1, moved) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{alice_c}/move"), Some(&alice_tok),
        Some(json!({ "x": 12.5, "y": 30.0 }))).await;
    assert_eq!(s1, 200, "alice move own: {moved}");
    assert!((moved["token_x"].as_f64().unwrap() - 12.5).abs() < 0.01);
    assert_eq!(moved["token_on_map"], true);

    // Alice tries to move Bob's token -> forbidden
    let (s2, _body) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{bob_c}/move"), Some(&alice_tok),
        Some(json!({ "x": 50.0, "y": 50.0 }))).await;
    assert_eq!(s2, 403);

    // Master can move anybody's token
    let (s3, moved3) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{bob_c}/move"), Some(&master_tok),
        Some(json!({ "x": 77.0, "y": 88.0 }))).await;
    assert_eq!(s3, 200);
    assert!((moved3["token_x"].as_f64().unwrap() - 77.0).abs() < 0.01);

    // Out-of-range coords clamp
    let (_, clamped) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{alice_c}/move"), Some(&alice_tok),
        Some(json!({ "x": 200.0, "y": -5.0 }))).await;
    assert_eq!(clamped["token_x"], 100.0);
    assert_eq!(clamped["token_y"], 0.0);

    // Add an NPC combatant; player cannot move it
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name) values ($1::uuid, 'Gob') returning id")
        .bind(&cid).fetch_one(&db).await.unwrap();
    let (_, npc_c) = json_req(&router, "POST",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&master_tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Gob" }))).await;
    let npc_cid = npc_c["id"].as_str().unwrap().to_string();
    let (s4, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{npc_cid}/move"), Some(&alice_tok),
        Some(json!({ "x": 10.0, "y": 10.0 }))).await;
    assert_eq!(s4, 403);

    // Master also sets a custom token color via PATCH
    let (s5, colored) = json_req(&router, "PATCH",
        &format!("/api/v1/combatants/{npc_cid}"), Some(&master_tok),
        Some(json!({ "token_color": "#8b1a1a", "token_on_map": true }))).await;
    assert_eq!(s5, 200);
    assert_eq!(colored["token_color"], "#8b1a1a");

    // Non-member cannot move anyone
    let (outsider_tok, _) = register(&router, "outsider@map.e").await;
    let (s6, _) = json_req(&router, "POST",
        &format!("/api/v1/combatants/{alice_c}/move"), Some(&outsider_tok),
        Some(json!({ "x": 1.0, "y": 1.0 }))).await;
    assert_eq!(s6, 403);

    // Master cannot change HP on a character-linked combatant — the player's
    // sheet owns HP. Non-HP patches (display_name, conditions) still work.
    let (s_hp, _body) = json_req(&router, "PATCH",
        &format!("/api/v1/combatants/{alice_c}"), Some(&master_tok),
        Some(json!({ "hp_current": 1 }))).await;
    assert_eq!(s_hp, 400);
    let (s_ok, _ok) = json_req(&router, "PATCH",
        &format!("/api/v1/combatants/{alice_c}"), Some(&master_tok),
        Some(json!({ "conditions": ["stunned"] }))).await;
    assert_eq!(s_ok, 200);
    // NPC HP still editable by master.
    let (s_npc, _npc) = json_req(&router, "PATCH",
        &format!("/api/v1/combatants/{npc_cid}"), Some(&master_tok),
        Some(json!({ "hp_current": 3 }))).await;
    assert_eq!(s_npc, 200);

    // Player listing: Alice sees full HP for her own combatant but zeros for
    // Bob's character and the NPC. Master listing still shows full HP.
    let (_, alice_view) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&alice_tok), None).await;
    let alice_arr = alice_view.as_array().unwrap();
    let alice_own = alice_arr.iter().find(|c| c["id"].as_str() == Some(&alice_c)).unwrap();
    let bob_view  = alice_arr.iter().find(|c| c["id"].as_str() == Some(&bob_c)).unwrap();
    let npc_view  = alice_arr.iter().find(|c| c["id"].as_str() == Some(&npc_cid)).unwrap();
    assert!(alice_own["hp_max"].as_i64().unwrap() > 0, "alice sees own HP");
    assert_eq!(bob_view["hp_current"], 0, "alice cannot see bob hp");
    assert_eq!(bob_view["hp_max"], 0);
    assert_eq!(bob_view["ac"], 0);
    assert_eq!(npc_view["hp_current"], 0);
    assert_eq!(npc_view["ac"], 0);
    // Master still sees the NPC's real HP we just set above.
    let (_, master_view) = json_req(&router, "GET",
        &format!("/api/v1/encounters/{eid}/combatants"), Some(&master_tok), None).await;
    let m_npc = master_view.as_array().unwrap().iter()
        .find(|c| c["id"].as_str() == Some(&npc_cid)).unwrap();
    assert_eq!(m_npc["hp_current"], 3);
}

#[tokio::test]
async fn spells_list_and_detail() {
    let (router, db) = skip_no_db!();
    let (tok, _) = register(&router, "wiz@e.com").await;

    // seed one spell directly
    sqlx::query(
        r#"insert into spells (slug, name, level, school, classes, description, source)
           values ('fire-bolt', 'Fire Bolt', 0, 'Evocation', array['Sorcerer','Wizard'], 'cantrip', 'SRD 5.1')"#)
        .execute(&db).await.unwrap();

    let (s, list) = json_req(&router, "GET", "/api/v1/spells", Some(&tok), None).await;
    assert_eq!(s, 200);
    assert!(list.as_array().unwrap().len() >= 1);

    let (s2, one) = json_req(&router, "GET", "/api/v1/spells/fire-bolt", Some(&tok), None).await;
    assert_eq!(s2, 200);
    assert_eq!(one["slug"], "fire-bolt");

    // filter by class
    let (s3, filt) = json_req(&router, "GET", "/api/v1/spells?class=Wizard", Some(&tok), None).await;
    assert_eq!(s3, 200);
    assert_eq!(filt.as_array().unwrap().len(), 1);
}
