use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser, rbac};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/spells", get(list))
        .route("/spells/{slug}", get(detail))
        .route("/campaigns/{id}/spells", get(list_campaign).post(create_campaign))
        .route("/campaigns/{id}/spells/{slug}", axum::routing::patch(update_campaign).delete(delete_campaign))
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
    pub effects: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct ListQ {
    pub q: Option<String>,
    pub level: Option<i16>,
    pub class: Option<String>,
    /// App-level: merge this campaign's homebrew spells (they override
    /// same-slug SRD entries).
    pub campaign_id: Option<Uuid>,
}

async fn list(
    State(s): State<AppState>,
    _: AuthUser,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Spell>>> {
    let rows: Vec<Spell> = sqlx::query_as::<_, Spell>(
        r#"select slug, name, level, school, casting_time, range_text, components, duration,
                  classes, ritual, concentration, description, higher_levels, source, effects
           from spells
           where ($1::text is null or name ilike '%' || $1 || '%')
             and ($2::smallint is null or level = $2)
             and ($3::text   is null or $3 = any(classes))
           order by level, name"#,
    )
    .bind(q.q.clone())
    .bind(q.level)
    .bind(q.class.clone())
    .fetch_all(&s.db)
    .await?;
    let rows = if let Some(cid) = q.campaign_id {
        let home: Vec<Spell> = sqlx::query_as::<_, Spell>(
            r#"select slug, name, level, school, casting_time, range_text, components, duration,
                      classes, ritual, concentration, description, higher_levels, source, effects
               from campaign_spells
               where campaign_id = $1
                 and ($2::text is null or name ilike '%' || $2 || '%')
                 and ($3::smallint is null or level = $3)
                 and ($4::text is null or $4 = any(classes))"#,
        )
        .bind(cid)
        .bind(q.q)
        .bind(q.level)
        .bind(q.class)
        .fetch_all(&s.db)
        .await?;
        let mut by_slug: std::collections::HashMap<String, Spell> =
            rows.into_iter().map(|s| (s.slug.clone(), s)).collect();
        for h in home {
            by_slug.insert(h.slug.clone(), h);
        }
        let mut merged: Vec<Spell> = by_slug.into_values().collect();
        merged.sort_by(|a, b| a.level.cmp(&b.level).then(a.name.cmp(&b.name)));
        merged
    } else {
        rows
    };
    Ok(Json(rows))
}

async fn detail(
    State(s): State<AppState>,
    _: AuthUser,
    Path(slug): Path<String>,
) -> AppResult<Json<Spell>> {
    let sp: Spell = sqlx::query_as::<_, Spell>(
        r#"select slug, name, level, school, casting_time, range_text, components, duration,
                  classes, ritual, concentration, description, higher_levels, source, effects
           from spells where slug = $1"#,
    )
    .bind(&slug)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(sp))
}


#[derive(Debug, Deserialize, Validate)]
pub struct CampaignSpellCreate {
    #[validate(length(min = 1, max = 64))]
    pub slug: String,
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(range(min = 0, max = 9))]
    pub level: i16,
    #[validate(length(max = 40))]
    pub school: String,
    pub casting_time: Option<String>,
    pub range_text: Option<String>,
    pub components: Option<String>,
    pub duration: Option<String>,
    pub classes: Option<Vec<String>>,
    pub ritual: Option<bool>,
    pub concentration: Option<bool>,
    #[validate(length(max = 4000))]
    pub description: String,
    pub higher_levels: Option<String>,
    pub effects: Option<serde_json::Value>,
}

async fn list_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Spell>>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let rows: Vec<Spell> = sqlx::query_as::<_, Spell>(
        r#"select slug, name, level, school, casting_time, range_text, components, duration,
                  classes, ritual, concentration, description, higher_levels, source, effects
           from campaign_spells where campaign_id = $1 order by level, name"#,
    )
    .bind(cid)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn create_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CampaignSpellCreate>,
) -> AppResult<(StatusCode, Json<Spell>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let sp: Spell = sqlx::query_as::<_, Spell>(
        r#"insert into campaign_spells
           (campaign_id, slug, name, level, school, casting_time, range_text, components, duration,
            classes, ritual, concentration, description, higher_levels, effects)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
           returning slug, name, level, school, casting_time, range_text, components, duration,
                     classes, ritual, concentration, description, higher_levels, source, effects"#,
    )
    .bind(cid)
    .bind(&body.slug)
    .bind(&body.name)
    .bind(body.level)
    .bind(&body.school)
    .bind(&body.casting_time)
    .bind(&body.range_text)
    .bind(&body.components)
    .bind(&body.duration)
    .bind(body.classes.unwrap_or_default())
    .bind(body.ritual.unwrap_or(false))
    .bind(body.concentration.unwrap_or(false))
    .bind(&body.description)
    .bind(&body.higher_levels)
    .bind(body.effects.unwrap_or_else(|| serde_json::json!({})))
    .fetch_one(&s.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("campaign spell slug exists".into())
        }
        other => other.into(),
    })?;
    crate::ws::publish(cid, serde_json::json!({"type": "campaign_spells_updated"}).to_string());
    Ok((StatusCode::CREATED, Json(sp)))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CampaignSpellUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    #[validate(range(min = 0, max = 9))]
    pub level: Option<i16>,
    pub school: Option<String>,
    pub casting_time: Option<String>,
    pub range_text: Option<String>,
    pub components: Option<String>,
    pub duration: Option<String>,
    pub classes: Option<Vec<String>>,
    pub ritual: Option<bool>,
    pub concentration: Option<bool>,
    pub description: Option<String>,
    pub higher_levels: Option<String>,
    pub effects: Option<serde_json::Value>,
}

async fn update_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, slug)): Path<(Uuid, String)>,
    Json(body): Json<CampaignSpellUpdate>,
) -> AppResult<Json<Spell>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let sp: Spell = sqlx::query_as::<_, Spell>(
        r#"update campaign_spells set
             name = coalesce($3, name),
             level = coalesce($4, level),
             school = coalesce($5, school),
             casting_time = coalesce($6, casting_time),
             range_text = coalesce($7, range_text),
             components = coalesce($8, components),
             duration = coalesce($9, duration),
             classes = coalesce($10, classes),
             ritual = coalesce($11, ritual),
             concentration = coalesce($12, concentration),
             description = coalesce($13, description),
             higher_levels = coalesce($14, higher_levels),
             effects = coalesce($15, effects),
             updated_at = now()
           where campaign_id = $1 and slug = $2
           returning slug, name, level, school, casting_time, range_text, components, duration,
                     classes, ritual, concentration, description, higher_levels, source, effects"#,
    )
    .bind(cid)
    .bind(&slug)
    .bind(&body.name)
    .bind(body.level)
    .bind(&body.school)
    .bind(&body.casting_time)
    .bind(&body.range_text)
    .bind(&body.components)
    .bind(&body.duration)
    .bind(body.classes)
    .bind(body.ritual)
    .bind(body.concentration)
    .bind(&body.description)
    .bind(&body.higher_levels)
    .bind(body.effects)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(cid, serde_json::json!({"type": "campaign_spells_updated"}).to_string());
    Ok(Json(sp))
}

async fn delete_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, slug)): Path<(Uuid, String)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, cid).await?;
    let res = sqlx::query("delete from campaign_spells where campaign_id = $1 and slug = $2")
        .bind(cid)
        .bind(&slug)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::ws::publish(cid, serde_json::json!({"type": "campaign_spells_updated"}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
