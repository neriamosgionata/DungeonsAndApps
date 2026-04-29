use crate::{AppState, error::AppResult, extract::AuthUser};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/spells", get(list))
        .route("/spells/{slug}", get(detail))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Spell {
    pub slug: String,
    pub name: String,
    pub level: i16,
    pub school: String,
    pub casting_time: Option<String>,
    pub range_text: Option<String>,
    pub components: Option<String>,
    pub duration: Option<String>,
    pub classes: Vec<String>,
    pub ritual: bool,
    pub concentration: bool,
    pub description: String,
    pub higher_levels: Option<String>,
    pub source: String,
}

#[derive(Debug, Deserialize)]
pub struct ListQ {
    pub q: Option<String>,
    pub level: Option<i16>,
    pub class: Option<String>,
}

async fn list(
    State(s): State<AppState>,
    _: AuthUser,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Spell>>> {
    let rows: Vec<Spell> = sqlx::query_as::<_, Spell>(
        r#"select slug, name, level, school, casting_time, range_text, components, duration,
                  classes, ritual, concentration, description, higher_levels, source
           from spells
           where ($1::text is null or name ilike '%' || $1 || '%')
             and ($2::smallint is null or level = $2)
             and ($3::text   is null or $3 = any(classes))
           order by level, name"#,
    )
    .bind(q.q)
    .bind(q.level)
    .bind(q.class)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn detail(
    State(s): State<AppState>,
    _: AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Json<Spell>> {
    let sp: Spell = sqlx::query_as::<_, Spell>(
        r#"select slug, name, level, school, casting_time, range_text, components, duration,
                  classes, ritual, concentration, description, higher_levels, source
           from spells where slug = $1"#,
    )
    .bind(&slug)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(sp))
}
