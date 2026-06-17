#![allow(dead_code)]

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use dungeonsandapps::{AppState, app, config::Config};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub fn test_db_url() -> Option<String> {
    // Load .env if present so local runs work without prefixing DATABASE_URL=
    let _ = dotenvy::dotenv();
    std::env::var("TEST_DATABASE_URL")
        .ok()
        .or_else(|| std::env::var("DATABASE_URL").ok())
}

pub async fn make_app() -> Option<(axum::Router, PgPool)> {
    let url = test_db_url()?;
    let cfg = Config {
        database_url: url.clone(),
        jwt_secret: "test-secret".into(),
        bind_addr: "127.0.0.1:0".into(),
        cors_origin: "*".into(),
        s3: None,
    };
    let state = AppState::new(cfg).await.ok()?;
    // reset schema
    sqlx::query("drop schema public cascade; create schema public;")
        .execute(&state.db)
        .await
        .ok()?;
    sqlx::migrate!("../migrations").run(&state.db).await.ok()?;
    let router = app(state.clone());
    Some((router, state.db))
}

pub async fn json_req(
    router: &axum::Router,
    method: &str,
    path: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(path)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(t) = token {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    let body = match body {
        Some(v) => Body::from(serde_json::to_vec(&v).unwrap()),
        None => Body::empty(),
    };
    let res = router.clone().oneshot(b.body(body).unwrap()).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let json: Value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, json)
}

pub async fn register(router: &axum::Router, email: &str) -> (String, Value) {
    register_with(router, email, None).await
}

pub const TEST_PASSWORD: &str = "Test123!Pass"; // Meets strong password requirements

pub async fn register_with(
    router: &axum::Router,
    email: &str,
    master_token: Option<&str>,
) -> (String, Value) {
    let (_, body) = json_req(
        router,
        "POST",
        "/api/v1/auth/register",
        master_token,
        Some(serde_json::json!({
            "email": email,
            "password": TEST_PASSWORD,
            "display_name": email.split('@').next().unwrap(),
        })),
    )
    .await;
    (body["token"].as_str().unwrap_or_default().to_string(), body)
}

/// Create a campaign, encounter, NPC, and one combatant. Returns (token, eid, combatant_id, cid).
pub async fn setup_encounter(
    router: &axum::Router,
    db: &sqlx::PgPool,
) -> (String, String, String, String) {
    let (master_tok, _) = register(router, "gm@setup.test").await;
    let (_, camp) = json_req(
        router,
        "POST",
        "/api/v1/campaigns",
        Some(&master_tok),
        Some(serde_json::json!({ "name": "Combat Test" })),
    )
    .await;
    let cid = camp["id"].as_str().unwrap().to_string();

    let npc_id: uuid::Uuid = sqlx::query_scalar(
        "insert into npcs (campaign_id, name, stats) values ($1::uuid, 'Goblin', '{\"ac\":12,\"hp\":{\"max\":7,\"current\":7}}'::jsonb) returning id")
        .bind(&cid).fetch_one(db).await.unwrap();

    let (_, enc) = json_req(
        router,
        "POST",
        &format!("/api/v1/campaigns/{cid}/encounters"),
        Some(&master_tok),
        Some(serde_json::json!({ "name": "Battle" })),
    )
    .await;
    let eid = enc["id"].as_str().unwrap().to_string();

    let (_, comb) = json_req(
        router,
        "POST",
        &format!("/api/v1/encounters/{eid}/combatants"),
        Some(&master_tok),
        Some(
            serde_json::json!({ "ref_type": "npc", "npc_id": npc_id, "display_name": "Goblin",
                     "initiative": 10, "hp_max": 7, "hp_current": 7, "ac": 12 }),
        ),
    )
    .await;
    let combatant_id = comb["id"].as_str().unwrap().to_string();

    (master_tok, eid, combatant_id, cid)
}

/// Bootstrap first master then register an extra user, returning master token, master user id,
/// user token, user user id. Use when a test needs two accounts.
pub async fn bootstrap_two(
    router: &axum::Router,
    master_email: &str,
    user_email: &str,
) -> (String, String, String, String) {
    let (master_tok, master_body) = register(router, master_email).await;
    let master_id = master_body["user"]["id"].as_str().unwrap().to_string();
    let (user_tok, user_body) = register_with(router, user_email, Some(&master_tok)).await;
    let user_id = user_body["user"]["id"].as_str().unwrap().to_string();
    (master_tok, master_id, user_tok, user_id)
}
