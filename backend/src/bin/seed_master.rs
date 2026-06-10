// Seeds a single master account into an empty users table.
// Usage: DATABASE_URL=... cargo run --bin seed_master -- <email> <password> [display_name]

use dungeonsandapps::auth::hash_password;
use sqlx::postgres::PgPoolOptions;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")?;

    let mut args = std::env::args().skip(1);
    let email = args.next().ok_or_else(|| anyhow::anyhow!("usage: seed_master <email> <password> [display_name]"))?;
    let password = args.next().ok_or_else(|| anyhow::anyhow!("usage: seed_master <email> <password> [display_name]"))?;
    let display_name = args.next().unwrap_or_else(|| email.split('@').next().unwrap_or("master").to_string());

    if password.len() < 8 {
        anyhow::bail!("password must be at least 8 chars");
    }

    let pool = PgPoolOptions::new().max_connections(2).connect(&url).await?;

    let count: i64 = sqlx::query_scalar("select count(*) from users").fetch_one(&pool).await?;
    if count > 0 {
        anyhow::bail!("users table is not empty ({count} rows) — refusing to seed");
    }

    let hash = hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{e:?}")))?;
    let id: uuid::Uuid = sqlx::query_scalar(
        r#"insert into users (email, password_hash, display_name, language, role)
           values ($1, $2, $3, 'en'::language_code, 'admin'::user_role)
           returning id"#,
    )
    .bind(&email)
    .bind(&hash)
    .bind(&display_name)
    .fetch_one(&pool)
    .await?;

    println!("seeded master:");
    println!("  id:           {id}");
    println!("  email:        {email}");
    println!("  display_name: {display_name}");
    println!("  password:     {password}");
    Ok(())
}
