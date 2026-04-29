use cinghialapp::{AppState, app, config::Config};
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cfg = Config::from_env()?;
    let state = AppState::new(cfg.clone()).await?;

    sqlx::migrate!("../migrations").run(&state.db).await?;
    state.ensure_default_admin().await?;

    let listener = tokio::net::TcpListener::bind(&cfg.bind_addr).await?;
    tracing::info!("cinghialapp listening on {}", cfg.bind_addr);
    axum::serve(listener, app(state)).await?;
    Ok(())
}
