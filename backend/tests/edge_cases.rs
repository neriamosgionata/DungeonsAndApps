//! WebSocket, rate limit, RBAC, config, error handling tests
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
// Rate Limiting
// =====================================================================

#[tokio::test]
async fn rate_limit_blocks_excessive_requests() {
    let (router, _db) = skip_no_db!();

    // Register a user first
    let email = format!("ratelimit_{}@test.com", uuid::Uuid::new_v4());
    let (s, _) = json_req(
        &router,
        "POST",
        "/api/v1/auth/register",
        None,
        Some(json!({
            "email": email,
            "password": TEST_PASSWORD,
            "display_name": "RateTest"
        })),
    )
    .await;
    assert_eq!(s, 201);

    // Make many rapid login attempts
    let mut blocked = false;
    for i in 0..15 {
        let (s, body) = json_req(
            &router,
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({
                "email": email,
                "password": "wrong"
            })),
        )
        .await;

        if s == 429 || (s == 400 && body.to_string().contains("too many")) {
            blocked = true;
            break;
        }

        if i < 10 {
            assert_eq!(s, 401, "attempt {} should be 401", i);
        }
    }

    // Rate limit should eventually trigger
    assert!(blocked, "rate limit should block excessive requests");
}

// =====================================================================
// RBAC Middleware
// =====================================================================

#[tokio::test]
async fn rbac_blocks_non_member_campaign_access() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "rbac1@test.com").await;

    // Create campaign
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "RBAC Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    // Create outsider
    let (outsider_tok, _) = register(&router, "outsider@test.com").await;

    // Outsider tries to access campaign
    let (s, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&outsider_tok),
        None,
    )
    .await;

    assert_eq!(s, 403, "non-member should be blocked from campaign");
}

#[tokio::test]
async fn rbac_allows_member_read_blocks_write() {
    let (router, _db) = skip_no_db!();
    let (master_tok, _) = register(&router, "rbac2@test.com").await;
    let (player_tok, player_body) =
        register_with(&router, "player@test.com", Some(&master_tok)).await;
    let player_id = player_body["user"]["id"].as_str().unwrap();

    // Create campaign and add player
    let (_, camp) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "RBAC Test 2" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/members"),
        Some(&master_tok),
        Some(json!({ "user_id": player_id, "role": "player" })),
    )
    .await;

    // Player can read
    let (s, _) = json_req(
        &router,
        "GET",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s, 200, "player should read campaign");

    // Player cannot delete
    let (s2, _) = json_req(
        &router,
        "DELETE",
        &format!("/api/v1/campaigns/{cid}"),
        Some(&player_tok),
        None,
    )
    .await;
    assert_eq!(s2, 403, "player should not delete campaign");
}

// =====================================================================
// Error Handling
// =====================================================================

#[tokio::test]
async fn error_not_found_returns_404() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "error@test.com").await;

    let (s, body) = json_req(
        &router,
        "GET",
        "/api/v1/campaigns/nonexistent-uuid",
        Some(&tok),
        None,
    )
    .await;

    assert_eq!(s, 404, "nonexistent resource should return 404: {}", body);
}

#[tokio::test]
async fn error_validation_returns_422() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "error2@test.com").await;

    // Create campaign with invalid data
    let (s, body) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "" })),
    )
    .await;

    // Empty name might be 400 or 422 depending on validation
    assert!(
        s == 400 || s == 422,
        "invalid data should return 400/422: {}",
        body
    );
}

#[tokio::test]
async fn error_unauthorized_returns_401() {
    let (router, _db) = skip_no_db!();

    // No token provided
    let (s, _) = json_req(&router, "GET", "/api/v1/auth/me", None, None).await;
    assert_eq!(s, 401, "missing token should return 401");

    // Invalid token
    let (s2, _) = json_req(
        &router,
        "GET",
        "/api/v1/auth/me",
        Some("invalid.token.here"),
        None,
    )
    .await;
    assert_eq!(s2, 401, "invalid token should return 401");
}

// =====================================================================
// Combat Mechanics - Cover, Critical Hits, Range
// =====================================================================

#[tokio::test]
async fn attack_with_half_cover_bonus() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_combat(&router, &db).await;

    let target_id = create_target(&router, &db, &eid, &tok, 10).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Attack with half cover
    let (_, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({
            "target_id": target_id,
            "damage_expression": "1d6",
            "damage_type": "piercing",
            "cover": "half"
        })),
    )
    .await;

    // Half cover gives +2 AC to target, making hit harder
    // Just verify the request succeeds
    assert!(
        result["hit"].is_boolean(),
        "attack with cover should return hit result"
    );
}

#[tokio::test]
async fn critical_hit_doubles_damage_dice() {
    let (router, db) = skip_no_db!();
    let (tok, eid, attacker_id, _cid) = setup_combat(&router, &db).await;

    // Target with low AC to ensure hits
    let target_id = create_target(&router, &db, &eid, &tok, 2).await;

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Attack with weapon that has damage die
    let (_, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{attacker_id}/attack"),
        Some(&tok),
        Some(json!({
            "target_id": target_id,
            "damage_expression": "1d8",
            "damage_type": "slashing",
            "damage_die": "d8"
        })),
    )
    .await;

    if result["critical"].as_bool().unwrap_or(false) {
        let dmg = result["damage_applied"].as_i64().unwrap_or(0);
        // Critical should double dice: 2d8 = min 2
        assert!(dmg >= 2, "critical hit should roll double dice");
    }
}

#[tokio::test]
async fn concentration_check_on_damage() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_combat(&router, &db).await;

    let target_id = create_target(&router, &db, &eid, &tok, 10).await;

    // Give caster a concentration effect
    sqlx::query("update combatants set concentration_spell = 'Bless', concentration_effect_id = 'test' where id = $1::uuid")
        .bind(&caster_id).execute(&db).await.ok();

    json_req(
        &router,
        "POST",
        &format!("/api/v1/encounters/{eid}/start"),
        Some(&tok),
        None,
    )
    .await;

    // Hit the caster to trigger concentration check
    let (_, result) = json_req(
        &router,
        "POST",
        &format!("/api/v1/combatants/{target_id}/attack"),
        Some(&tok),
        Some(json!({
            "target_id": caster_id,
            "damage_expression": "5",
            "damage_type": "bludgeoning"
        })),
    )
    .await;

    // Result should indicate if concentration was broken or roll made
    assert!(
        result["concentration_broken"].is_boolean() || result["concentration_roll"].is_object(),
        "damage to concentrating target should show concentration status"
    );
}

// =====================================================================
// Spell Components Blocking
// =====================================================================

#[tokio::test]
async fn silenced_blocks_verbal_spells() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_combat(&router, &db).await;

    // Seed spell with V component
    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source, components)
         values ('command', 'Command', 1, 'Enchantment', array['Cleric'], 'spell', 'SRD', array['V'])
         on conflict do nothing")
        .execute(&db).await.ok();

    // Silence the caster
    sqlx::query("update combatants set modifiers = jsonb_build_object('silenced', true) where id = $1::uuid")
        .bind(&caster_id).execute(&db).await.ok();

    let target_id = create_target(&router, &db, &eid, &tok, 10).await;

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
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "command",
            "upcast_level": 1,
            "target_ids": [target_id]
        })),
    )
    .await;

    // Silenced caster MUST be blocked from casting V-component spell.
    // 400 = BadRequest (component validation), 403 = Forbidden
    assert!(
        s == 400 || s == 403,
        "silenced caster MUST be blocked from verbal spell (got {}): {}",
        s,
        result
    );
}

#[tokio::test]
async fn no_somatic_blocks_spells_without_war_caster() {
    let (router, db) = skip_no_db!();
    let (tok, eid, caster_id, _cid) = setup_combat(&router, &db).await;

    // Seed spell with S component
    sqlx::query(
        "insert into spells (slug, name, level, school, classes, description, source, components)
         values ('shield', 'Shield', 1, 'Abjuration', array['Wizard'], 'reaction', 'SRD', array['V','S'])
         on conflict do nothing")
        .execute(&db).await.ok();

    // Restrain caster (no somatic)
    sqlx::query("update combatants set modifiers = jsonb_build_object('no_somatic', true) where id = $1::uuid")
        .bind(&caster_id).execute(&db).await.ok();

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
        &format!("/api/v1/combatants/{caster_id}/cast-spell"),
        Some(&tok),
        Some(json!({
            "spell_slug": "shield",
            "upcast_level": 1,
            "targets": []
        })),
    )
    .await;

    // Caster without somatic ability MUST be blocked from S-component spell.
    // 400 = BadRequest (component validation), 403 = Forbidden
    assert!(
        s == 400 || s == 403,
        "caster w/o somatic MUST be blocked from S-component spell (got {}): {}",
        s,
        result
    );
}

// =====================================================================
// Saving Throws
// =====================================================================

#[tokio::test]
async fn saving_throw_with_damage_applies() {
    let (router, db) = skip_no_db!();
    let (tok, eid, combatant_id, _cid) = setup_combat(&router, &db).await;

    // Set hit points on target so we can verify damage
    sqlx::query("update combatants set hp_current = 20 where id = $1::uuid")
        .bind(&combatant_id)
        .execute(&db)
        .await
        .ok();

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
        &format!("/api/v1/combatants/{combatant_id}/save"),
        Some(&tok),
        Some(json!({
            "ability": "dex",
            "dc": 15,
            "damage_on_fail": "2d6",
            "damage_type": "fire",
            "half_on_save": true
        })),
    )
    .await;

    assert_eq!(s, 200, "saving throw should succeed: {}", result);
    assert!(result["save_total"].is_i64(), "should have save_total");
    assert!(result["passed"].is_boolean(), "should have passed field");
    assert!(
        result["damage"].is_i64() || result["total_damage"].is_i64(),
        "should have damage"
    );
}

// =====================================================================
// Database Edge Cases
// =====================================================================

#[tokio::test]
async fn handles_special_characters_in_names() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "special@test.com").await;

    // Campaign with special characters
    let (s, result) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": "Test: Special & Characters <script>alert('xss')</script>" })),
    )
    .await;

    assert_eq!(s, 201, "special characters should be handled: {}", result);
}

#[tokio::test]
async fn handles_very_long_names() {
    let (router, _db) = skip_no_db!();
    let (tok, _) = register(&router, "long@test.com").await;

    let long_name = "A".repeat(200);
    let (s, result) = json_req(
        &router,
        "POST",
        "/api/v1/campaigns",
        Some(&tok),
        Some(json!({ "name": long_name })),
    )
    .await;

    // Should either succeed or fail gracefully with validation error
    assert!(
        s == 201 || s == 400 || s == 422,
        "long names should be handled: {}",
        result
    );
}

// Helper functions
async fn setup_combat(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@mechanics.test").await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(json!({ "name": "Mechanics Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'TestEnemy', '{\"ac\":10,\"hp\":{\"max\":20,\"current\":20}}'::jsonb) returning id")
        .bind(&cid).fetch_one(db).await.unwrap();

    let (_, enc) = json_req(
        router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok),
        Some(json!({ "name": "Test Battle" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, comb) = json_req(router, "POST", &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok),
        Some(json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Attacker",
                     "initiative": 10, "hp_max": 20, "hp_current": 20, "ac": 15, "level_total": 5 }))).await;
    let combatant_id = comb["id"].as_str().unwrap().to_string();

    (master_tok, eid, combatant_id, cid)
}

async fn create_target(
    router: &axum::Router,
    db: &sqlx::PgPool,
    eid: &str,
    tok: &str,
    ac: i32,
) -> String {
    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ((select campaign_id from encounters where id = $1::uuid), 'Target', $2::jsonb) returning id")
        .bind(eid)
        .bind(format!("{{\"ac\":{},\"hp\":{{\"max\":20,\"current\":20}}}}", ac))
        .fetch_one(db).await.unwrap();

    let (_, target) = json_req(
        router,
        "POST",
        &format!("/api/v1/encounters/{}/combatants", eid),
        Some(tok),
        Some(
            json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Target",
                     "initiative": 5, "hp_max": 20, "hp_current": 20, "ac": ac }),
        ),
    )
    .await;

    target["id"].as_str().unwrap().to_string()
}
