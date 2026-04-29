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
