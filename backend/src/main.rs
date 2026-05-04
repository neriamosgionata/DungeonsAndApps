use dungeonsandapps::{AppState, app, config::Config, ws};
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
