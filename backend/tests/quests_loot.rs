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
    let (master_tok, _) = register(router, "gm@ql.test").await;
    let (player_tok, _) = register_with(router, "pl@ql.test", Some(&master_tok)).await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "QL Camp" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();
    add_member_via_invite(
        router,
        &master_tok,
        &player_tok,
        "pl@ql.test",
        &cid,
        "player",
    )
    .await;
    (master_tok, player_tok, cid)
}

// =====================================================================
// Quests CRUD
// =====================================================================

#[tokio::test]
async fn quest_create_and_list() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (s1, q1) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Save the town", "visibility": "players" })),
    )
    .await;
    assert_eq!(s1, 201);
    assert_eq!(q1["title"], "Save the town");
    assert_eq!(q1["status"], "active");

    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Find the relic", "status": "active", "visibility": "master" })),
    )
    .await;
    assert_eq!(s2, 201);

    let (_, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        None,
    )
    .await;
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 2);
    let titles: Vec<&str> = arr.iter().map(|q| q["title"].as_str().unwrap()).collect();
    assert!(titles.contains(&"Save the town"));
    assert!(titles.contains(&"Find the relic"));
}

#[tokio::test]
async fn quest_read_update_delete() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Rescue the wizard", "description": "He is in the tower." })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();

    let (s_read, detail) = json_req(
        &router,
        "GET",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_read, 200);
    assert_eq!(detail["title"], "Rescue the wizard");
    assert_eq!(detail["status"], "active");

    let (s_upd, updated) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        Some(json!({ "status": "completed" })),
    )
    .await;
    assert_eq!(s_upd, 200);
    assert_eq!(updated["status"], "completed");

    let (s_upd2, failed) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        Some(json!({ "status": "failed" })),
    )
    .await;
    assert_eq!(s_upd2, 200);
    assert_eq!(failed["status"], "failed");

    let (s_del, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_del, 204);

    let (s_gone, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_gone, 404);
}

#[tokio::test]
async fn quest_player_cannot_create() {
    let (router, _) = skip_no_db!();
    let (_mtok, ptok, cid) = setup(&router).await;

    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&ptok),
        Some(json!({ "title": "Nope" })),
    )
    .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn quest_player_cannot_delete() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Master-only delete" })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/quests/{qid}"),
        Some(&ptok),
        None,
    )
    .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn quest_player_cannot_update() {
    let (router, _) = skip_no_db!();
    let (mtok, ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Read-only" })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();

    let (s, _) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&ptok),
        Some(json!({ "status": "completed" })),
    )
    .await;
    assert_eq!(s, 403);
}

#[tokio::test]
async fn quest_link_npc() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "NPC quest" })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();

    let (_, n) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&mtok),
        Some(json!({ "name": "Quest Giver", "visibility": "players" })),
    )
    .await;
    let nid = n["id"].as_str().unwrap();

    let (s_link, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/quests/{qid}/npcs"),
        Some(&mtok),
        Some(json!({ "npc_id": nid, "role": "quest giver" })),
    )
    .await;
    assert_eq!(s_link, 204);

    let (_, detail) = json_req(
        &router,
        "GET",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        None,
    )
    .await;
    let npcs = detail["npcs"].as_array().unwrap();
    assert_eq!(npcs.len(), 1);
    assert_eq!(npcs[0]["name"], "Quest Giver");
    assert_eq!(npcs[0]["npc_id"], nid);
}

#[tokio::test]
async fn quest_unlink_npc() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Unlink quest" })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();

    let (_, n) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/npcs"),
        Some(&mtok),
        Some(json!({ "name": "Temporary NPC", "visibility": "players" })),
    )
    .await;
    let nid = n["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/quests/{qid}/npcs"),
        Some(&mtok),
        Some(json!({ "npc_id": nid })),
    )
    .await;

    let (s_unlink, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/quests/{qid}/npcs/{nid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_unlink, 204);

    let (_, detail) = json_req(
        &router,
        "GET",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(detail["npcs"].as_array().unwrap().len(), 0);
}

// =====================================================================
// Loot CRUD
// =====================================================================

#[tokio::test]
async fn loot_create_and_list() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (s1, l1) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&mtok),
        Some(json!({ "name": "Sword +1", "quantity": 1, "value_gp": 1000.0 })),
    )
    .await;
    assert_eq!(s1, 201);
    assert_eq!(l1["name"], "Sword +1");

    let (s2, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&mtok),
        Some(json!({ "name": "Potion of Healing", "quantity": 3 })),
    )
    .await;
    assert_eq!(s2, 201);

    let (_, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&mtok),
        None,
    )
    .await;
    let arr = list.as_array().unwrap();
    assert_eq!(arr.len(), 2);
}

#[tokio::test]
async fn loot_update_delete() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (_, l) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&mtok),
        Some(json!({ "name": "Ring", "quantity": 1, "value_gp": 50.0 })),
    )
    .await;
    let lid = l["id"].as_str().unwrap();

    let (s_upd, updated) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/loot/{lid}"),
        Some(&mtok),
        Some(json!({ "quantity": 5, "value_gp": 250.0 })),
    )
    .await;
    assert_eq!(s_upd, 200);
    assert_eq!(updated["quantity"], 5);

    let (s_del, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/loot/{lid}"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(s_del, 204);

    let (_, list) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&mtok),
        None,
    )
    .await;
    assert_eq!(list.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn loot_player_can_create() {
    let (router, _) = skip_no_db!();
    let (_mtok, ptok, cid) = setup(&router).await;

    let (s, _) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&ptok),
        Some(json!({ "name": "Player loot" })),
    )
    .await;
    assert_eq!(s, 201, "player should be allowed to create loot");
}

#[tokio::test]
async fn loot_non_member_cannot_access() {
    let (router, _) = skip_no_db!();
    let (mtok, _, cid) = setup(&router).await;
    let (outsider_tok, _) = register_with(&router, "out@ql.test", Some(&mtok)).await;

    let (s, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}/loot"),
        Some(&outsider_tok),
        None,
    )
    .await;
    assert_eq!(s, 403);
}

// =====================================================================
// Quest Status Transitions
// =====================================================================

#[tokio::test]
async fn quest_status_transitions() {
    let (router, _) = skip_no_db!();
    let (mtok, _ptok, cid) = setup(&router).await;

    let (_, q) = json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/quests"),
        Some(&mtok),
        Some(json!({ "title": "Transitions" })),
    )
    .await;
    let qid = q["id"].as_str().unwrap();
    assert_eq!(q["status"], "active");

    let (_, c) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        Some(json!({ "status": "completed" })),
    )
    .await;
    assert_eq!(c["status"], "completed");

    let (_, f) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        Some(json!({ "status": "failed" })),
    )
    .await;
    assert_eq!(f["status"], "failed");

    let (_, a) = json_req(
        &router,
        "PATCH",
        &format!("/api/v1/quests/{qid}"),
        Some(&mtok),
        Some(json!({ "status": "active" })),
    )
    .await;
    assert_eq!(a["status"], "active");
}
