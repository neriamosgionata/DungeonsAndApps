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
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();
    let url = test_db_url()?;
    eprintln!("make_app: got url={}", url);
    // Each test gets its own schema so concurrent tests across binaries don't
    // race on a shared `public` schema. The search_path is pinned via the
    // `options` URL parameter, so every new connection in the pool uses it.
    let schema = format!(
        "t_{}",
        uuid::Uuid::new_v4().simple().to_string()[..12].to_lowercase()
    );
    let schema_url = if url.contains('?') {
        format!("{url}&options=-c%20search_path%3D{schema}")
    } else {
        format!("{url}?options=-c%20search_path%3D{schema}")
    };
    let cfg = Config {
        database_url: schema_url,
        jwt_secret: "test-secret".into(),
        bind_addr: "127.0.0.1:0".into(),
        cors_origin: "*".into(),
        s3: None,
    };
    let state = AppState::new(cfg).await.ok()?;
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&state.db)
        .await
        .ok()?;
    sqlx::migrate!("../migrations")
        .run(&state.db)
        .await
        .ok()?;
    seed_spells(&state.db).await.ok()?;
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
        .header(header::CONTENT_TYPE, "application/json")
        .extension(axum::extract::ConnectInfo(
            std::net::SocketAddr::from(([127, 0, 0, 1], 0)),
        ));
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

/// Seed SRD spells into the test DB. make_app resets the schema on every run,
/// so this must run after `migrate!` to populate spells used by combat tests.
async fn seed_spells(db: &PgPool) -> anyhow::Result<()> {
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct SpellFile {
        spells: Vec<SrdSpell>,
    }
    #[derive(Deserialize)]
    struct SrdSpell {
        slug: String,
        name: String,
        level: i16,
        school: String,
        casting_time: Option<String>,
        range: Option<String>,
        components: Option<String>,
        duration: Option<String>,
        classes: Vec<String>,
        ritual: bool,
        concentration: bool,
        description: String,
        higher_levels: Option<String>,
        source: String,
    }

    let path = std::env::var("SPELLS_SRD_PATH")
        .unwrap_or_else(|_| "../shared/spells-srd.json".into());
    let raw = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("read {}: {}", path, e))?;
    let file: SpellFile = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("parse {}: {}", path, e))?;

    for s in &file.spells {
        sqlx::query(
            r#"insert into spells
               (slug, name, level, school, casting_time, range_text, components, duration,
                classes, ritual, concentration, description, higher_levels, source)
               values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
               on conflict (slug) do update set
                 name=excluded.name, level=excluded.level, school=excluded.school"#,
        )
        .bind(&s.slug)
        .bind(&s.name)
        .bind(s.level)
        .bind(&s.school)
        .bind(&s.casting_time)
        .bind(&s.range)
        .bind(&s.components)
        .bind(&s.duration)
        .bind(&s.classes)
        .bind(s.ritual)
        .bind(s.concentration)
        .bind(&s.description)
        .bind(&s.higher_levels)
        .bind(&s.source)
        .execute(db)
        .await?;
    }
    Ok(())
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
