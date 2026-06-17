use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let url = std::env::var("DATABASE_URL")?;
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&url)
        .await?;

    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../shared/spells-srd.json".into());
    let raw = std::fs::read_to_string(&path)?;
    let file: SpellFile = serde_json::from_str(&raw)?;

    let mut count = 0;
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
        .execute(&pool)
        .await?;
        count += 1;
    }
    println!("seeded {count} spells from {path}");
    Ok(())
}
