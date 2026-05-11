use dungeonsandapps::{AppState, app, config::Config, ws};
use serde::Deserialize;
use tracing_subscriber::{EnvFilter, fmt};

#[derive(Debug, Deserialize)]
struct SpellFile {
    spells: Vec<SrdSpell>,
}

#[derive(Debug, Deserialize)]
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

async fn seed_spells_if_empty(db: &sqlx::PgPool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("select count(*) from spells").fetch_one(db).await?;
    if count > 0 {
        return Ok(());
    }

    // Try multiple possible paths for the spells file
    let paths = [
        "../shared/spells-srd.json".to_string(),
        "./shared/spells-srd.json".to_string(),
        "/app/shared/spells-srd.json".to_string(),
        std::env::var("SPELLS_JSON_PATH").unwrap_or_default(),
    ];

    let mut raw = None;
    for path in &paths {
        if path.is_empty() {
            continue;
        }
        match std::fs::read_to_string(path) {
            Ok(content) => {
                tracing::info!("Loading spells from {}", path);
                raw = Some(content);
                break;
            }
            Err(e) => {
                tracing::debug!("Could not read spells from {}: {}", path, e);
            }
        }
    }

    let raw = raw.ok_or_else(|| anyhow::anyhow!("spells-srd.json not found in any known location"))?;
    let file: SpellFile = serde_json::from_str(&raw)?;

    let mut seeded = 0;
    for s in &file.spells {
        sqlx::query(
            r#"insert into spells
               (slug, name, level, school, casting_time, range_text, components, duration,
                classes, ritual, concentration, description, higher_levels, source)
               values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
               on conflict (slug) do update set
                 name=excluded.name, level=excluded.level, school=excluded.school,
                 casting_time=excluded.casting_time, range_text=excluded.range_text,
                 components=excluded.components, duration=excluded.duration,
                 classes=excluded.classes, ritual=excluded.ritual, concentration=excluded.concentration,
                 description=excluded.description, higher_levels=excluded.higher_levels, source=excluded.source"#,
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
        seeded += 1;
    }
    tracing::info!("Seeded {} spells", seeded);
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cfg = Config::from_env()?;
    let state = AppState::new(cfg.clone()).await?;

    sqlx::migrate!("../migrations").run(&state.db).await?;
    seed_spells_if_empty(&state.db).await?;
    state.ensure_default_admin().await?;

    // Start WebSocket channel cleanup task to prevent memory leaks
    ws::start_cleanup_task();

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("DungeonsAndApps listening on {}", cfg.bind_addr);
    axum::serve(
        listener,
        app(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await?;
    Ok(())
}
