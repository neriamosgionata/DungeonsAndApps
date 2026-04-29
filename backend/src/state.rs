use crate::{auth::hash_password, config::Config};
use sqlx::{PgPool, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct AppState {
    pub cfg: Config,
    pub db: PgPool,
}

impl AppState {
    pub async fn new(cfg: Config) -> anyhow::Result<Self> {
        let db = PgPoolOptions::new()
            .max_connections(16)
            .connect(&cfg.database_url)
            .await?;
        Ok(Self { cfg, db })
    }

    /// Create the default admin account if no admin exists. Call after migrations.
    /// Email + password come from env (ADMIN_EMAIL / ADMIN_PASSWORD) or use built-in defaults.
    pub async fn ensure_default_admin(&self) -> anyhow::Result<()> {
        let admin_count: i64 = sqlx::query_scalar("select count(*) from users where role = 'admin'")
            .fetch_one(&self.db).await?;
        if admin_count > 0 { return Ok(()); }

        let email = std::env::var("ADMIN_EMAIL").unwrap_or_else(|_| "admin@cinghialapp.local".into());
        let password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin-change-me".into());
        let hash = hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{e:?}")))?;

        sqlx::query(
            r#"insert into users (email, password_hash, display_name, language, role)
               values ($1, $2, 'Admin', 'en', 'admin'::user_role)
               on conflict (email) do update set role = 'admin'"#,
        )
        .bind(&email).bind(&hash)
        .execute(&self.db).await?;

        tracing::info!(email = %email, "default admin ensured");
        Ok(())
    }
}
