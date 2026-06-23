use crate::{auth::hash_password, config::Config};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Config,
    pub db: PgPool,
}

impl AppState {
    pub async fn new(cfg: Config) -> anyhow::Result<Self> {
        // I-P1: bumped from 16 to 32. The audit found 16 was tight for
        // 4 PCs + 1 master + background tasks (migrations, scheduled
        // jobs, etc.). 32 gives ~2x headroom without a meaningful memory
        // increase on the postgres server. Override with the
        // DATABASE_MAX_CONNECTIONS env var for tuning.
        let max_connections: u32 = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(32);
        let db = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(&cfg.database_url)
            .await?;
        Ok(Self { cfg, db })
    }

    /// Create the default admin account if no admin exists. Call after migrations.
    /// **Both** ADMIN_EMAIL and ADMIN_PASSWORD must be set; fails hard otherwise
    /// so the operator is forced to choose a secure password.
    pub async fn ensure_default_admin(&self) -> anyhow::Result<()> {
        let admin_count: i64 =
            sqlx::query_scalar("select count(*) from users where role = 'admin'")
                .fetch_one(&self.db)
                .await?;
        if admin_count > 0 {
            return Ok(());
        }

        let email = std::env::var("ADMIN_EMAIL")
            .map_err(|_| anyhow::anyhow!("ADMIN_EMAIL must be set to bootstrap the first admin"))?;
        let password = std::env::var("ADMIN_PASSWORD").map_err(|_| {
            anyhow::anyhow!("ADMIN_PASSWORD must be set to bootstrap the first admin")
        })?;
        if password.len() < 12 {
            anyhow::bail!("ADMIN_PASSWORD must be at least 12 characters");
        }
        let hash = hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{e:?}")))?;

        sqlx::query(
            r#"insert into users (email, password_hash, display_name, language, role)
               values ($1, $2, 'Admin', 'en', 'admin'::user_role)
               on conflict (email) do update set role = 'admin'"#,
        )
        .bind(&email)
        .bind(&hash)
        .execute(&self.db)
        .await?;

        tracing::info!(email = %email, "default admin ensured");
        Ok(())
    }
}
