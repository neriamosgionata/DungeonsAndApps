use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use serde_json::json;
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::{Date, OffsetDateTime};
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/sessions", get(list).post(create))
        .route("/sessions/{id}", get(read).patch(update).delete(delete))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Session {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub title: String,
    pub session_number: Option<i32>,
    pub played_at: Option<Date>,
    pub status: String,
    pub recap: Option<String>,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SessionCreate {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    pub session_number: Option<i32>,
    pub played_at: Option<Date>,
    pub status: Option<String>,
    pub recap: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct SessionUpdate {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,
    pub session_number: Option<i32>,
    pub played_at: Option<Date>,
    pub status: Option<String>,
    pub recap: Option<String>,
    pub visibility: Option<String>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Session>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query branches instead of format!
    let rows: Vec<Session> = if role == Role::Master {
        sqlx::query_as::<_, Session>(
            "select id, campaign_id, title, session_number, played_at,
                    status::text as status, recap, visibility::text as visibility, updated_at
             from campaign_sessions where campaign_id = $1 order by coalesce(session_number, 0) desc, played_at desc nulls last")
            .bind(cid)
            .fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Session>(
            "select id, campaign_id, title, session_number, played_at,
                    status::text as status, recap, visibility::text as visibility, updated_at
             from campaign_sessions where campaign_id = $1 and visibility = 'players' order by coalesce(session_number, 0) desc, played_at desc nulls last")
            .bind(cid)
            .fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<SessionCreate>,
) -> AppResult<(StatusCode, Json<Session>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let status = body.status.as_deref().unwrap_or("played");
    let vis = body.visibility.as_deref().unwrap_or("players");
    let sess: Session = sqlx::query_as::<_, Session>(
        "insert into campaign_sessions (campaign_id, title, session_number, played_at, status, recap, visibility, created_by)
         values ($1, $2, $3, $4, $5::session_status, $6, $7::visibility, $8)
         returning id, campaign_id, title, session_number, played_at,
                   status::text as status, recap, visibility::text as visibility, updated_at")
        .bind(cid).bind(&body.title).bind(body.session_number).bind(body.played_at)
        .bind(status).bind(&body.recap).bind(vis).bind(uid).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"session_created","id":sess.id}).to_string());
    Ok((StatusCode::CREATED, Json(sess)))
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Session>> {
    let sess: Session = sqlx::query_as::<_, Session>(
        "select id, campaign_id, title, session_number, played_at,
                status::text as status, recap, visibility::text as visibility, updated_at
         from campaign_sessions where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, sess.campaign_id).await?;
    if role == Role::Player && sess.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(sess))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SessionUpdate>,
) -> AppResult<Json<Session>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from campaign_sessions where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let sess: Session = sqlx::query_as::<_, Session>(
        "update campaign_sessions set
           title          = coalesce($2, title),
           session_number = coalesce($3, session_number),
           played_at      = coalesce($4, played_at),
           status         = coalesce($5::session_status, status),
           recap          = coalesce($6, recap),
           visibility     = coalesce($7::visibility, visibility)
         where id = $1
         returning id, campaign_id, title, session_number, played_at,
                   status::text as status, recap, visibility::text as visibility, updated_at")
        .bind(id).bind(body.title).bind(body.session_number).bind(body.played_at)
        .bind(body.status).bind(body.recap).bind(body.visibility).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"session_updated","id":id}).to_string());
    Ok(Json(sess))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from campaign_sessions where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from campaign_sessions where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(cid, json!({"type":"session_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
