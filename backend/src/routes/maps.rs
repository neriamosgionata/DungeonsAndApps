use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/maps", get(list).post(create))
        .route("/maps/{id}", get(read).patch(update).delete(delete))
        .route("/maps/{id}/pins", get(list_pins).post(create_pin))
        .route("/pins/{id}", axum::routing::patch(update_pin).delete(delete_pin))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Map {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub image_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MapCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub description: Option<String>,
    pub image_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MapUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub image_key: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub visibility: Option<String>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Map>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query instead of format! to prevent SQL injection
    let rows: Vec<Map> = if role == Role::Master {
        sqlx::query_as::<_, Map>(
            "select id, campaign_id, name, description, image_key, width, height,
                    visibility::text as visibility, updated_at
             from maps where campaign_id = $1 order by name")
            .bind(cid)
            .fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Map>(
            "select id, campaign_id, name, description, image_key, width, height,
                    visibility::text as visibility, updated_at
             from maps where campaign_id = $1 and visibility = 'players' order by name")
            .bind(cid)
            .fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<MapCreate>,
) -> AppResult<(StatusCode, Json<Map>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("players");
    let m: Map = sqlx::query_as::<_, Map>(
        "insert into maps (campaign_id, name, description, image_key, width, height, visibility)
         values ($1, $2, $3, $4, $5, $6, $7::visibility)
         returning id, campaign_id, name, description, image_key, width, height,
                   visibility::text as visibility, updated_at")
        .bind(cid).bind(&body.name).bind(&body.description).bind(&body.image_key)
        .bind(body.width).bind(body.height).bind(vis).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"map_created","id":m.id}).to_string());
    Ok((StatusCode::CREATED, Json(m)))
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Map>> {
    let m: Map = sqlx::query_as::<_, Map>(
        "select id, campaign_id, name, description, image_key, width, height,
                visibility::text as visibility, updated_at from maps where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, m.campaign_id).await?;
    if role == Role::Player && m.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(m))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MapUpdate>,
) -> AppResult<Json<Map>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from maps where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let m: Map = sqlx::query_as::<_, Map>(
        "update maps set
           name        = coalesce($2, name),
           description = coalesce($3, description),
           image_key   = coalesce($4, image_key),
           width       = coalesce($5, width),
           height      = coalesce($6, height),
           visibility  = coalesce($7::visibility, visibility)
         where id = $1
         returning id, campaign_id, name, description, image_key, width, height,
                   visibility::text as visibility, updated_at")
        .bind(id).bind(body.name).bind(body.description).bind(body.image_key)
        .bind(body.width).bind(body.height).bind(body.visibility).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"map_updated","id":m.id}).to_string());
    Ok(Json(m))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from maps where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from maps where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(cid, json!({"type":"map_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
pub struct Pin {
    pub id: Uuid,
    pub map_id: Uuid,
    pub label: String,
    pub kind: String,
    pub faction_id: Option<Uuid>,
    pub is_party: bool,
    pub x: f64,
    pub y: f64,
    pub color: Option<String>,
    pub note: Option<String>,
    pub icon_url: Option<String>,
    pub visibility: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PinCreate {
    #[validate(length(min = 1, max = 80))]
    pub label: String,
    #[validate(length(min = 1, max = 40))]
    pub kind: String,
    pub faction_id: Option<Uuid>,
    #[serde(default)]
    pub is_party: bool,
    pub x: f64,
    pub y: f64,
    pub color: Option<String>,
    pub note: Option<String>,
    pub icon_url: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PinUpdate {
    #[validate(length(min = 1, max = 80))]
    pub label: Option<String>,
    pub kind: Option<String>,
    pub faction_id: Option<Uuid>,
    pub is_party: Option<bool>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub color: Option<String>,
    pub note: Option<String>,
    pub icon_url: Option<String>,
    pub visibility: Option<String>,
}

async fn list_pins(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(map_id): Path<Uuid>,
) -> AppResult<Json<Vec<Pin>>> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from maps where id = $1")
        .bind(map_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, cid).await?;
    let rows: Vec<Pin> = if role == Role::Master {
        sqlx::query_as::<_, Pin>(
            "select id, map_id, label, kind, faction_id, is_party, x, y, color, note, icon_url,
                    visibility::text as visibility
             from map_pins where map_id = $1 order by label")
            .bind(map_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Pin>(
            "select id, map_id, label, kind, faction_id, is_party, x, y, color, note, icon_url,
                    visibility::text as visibility
             from map_pins where map_id = $1 and visibility = 'players' order by label")
            .bind(map_id).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create_pin(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(map_id): Path<Uuid>,
    Json(body): Json<PinCreate>,
) -> AppResult<(StatusCode, Json<Pin>)> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from maps where id = $1")
        .bind(map_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("players");
    let p: Pin = sqlx::query_as::<_, Pin>(
        "insert into map_pins (map_id, label, kind, faction_id, is_party, x, y, color, note, icon_url, visibility)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::visibility)
         returning id, map_id, label, kind, faction_id, is_party, x, y, color, note, icon_url,
                   visibility::text as visibility")
        .bind(map_id).bind(&body.label).bind(&body.kind).bind(body.faction_id).bind(body.is_party)
        .bind(body.x).bind(body.y).bind(&body.color).bind(&body.note).bind(&body.icon_url).bind(vis)
        .fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"pin_created","map_id":map_id,"id":p.id}).to_string());
    Ok((StatusCode::CREATED, Json(p)))
}

async fn update_pin(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PinUpdate>,
) -> AppResult<Json<Pin>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar(
        "select m.campaign_id from map_pins p join maps m on m.id = p.map_id where p.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let p: Pin = sqlx::query_as::<_, Pin>(
        "update map_pins set
           label      = coalesce($2, label),
           kind       = coalesce($3, kind),
           faction_id = coalesce($4, faction_id),
           is_party   = coalesce($5, is_party),
           x          = coalesce($6, x),
           y          = coalesce($7, y),
           color      = coalesce($8, color),
           note       = coalesce($9, note),
           icon_url   = coalesce($10, icon_url),
           visibility = coalesce($11::visibility, visibility)
         where id = $1
         returning id, map_id, label, kind, faction_id, is_party, x, y, color, note, icon_url,
                   visibility::text as visibility")
        .bind(id).bind(body.label).bind(body.kind).bind(body.faction_id).bind(body.is_party)
        .bind(body.x).bind(body.y).bind(body.color).bind(body.note).bind(body.icon_url).bind(body.visibility)
        .fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"pin_updated","id":id,"x":p.x,"y":p.y}).to_string());
    Ok(Json(p))
}

async fn delete_pin(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar(
        "select m.campaign_id from map_pins p join maps m on m.id = p.map_id where p.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from map_pins where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(cid, json!({"type":"pin_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
