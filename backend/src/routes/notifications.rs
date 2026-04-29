use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{FromRow, PgPool};
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/notifications", get(list).delete(delete_all))
        .route("/notifications/read-all", post(mark_all))
        .route("/notifications/{id}/read", post(mark_one))
        .route("/notifications/{id}", axum::routing::delete(delete_one))
}

async fn delete_all(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<StatusCode> {
    sqlx::query("delete from notifications where user_id = $1")
        .bind(uid).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow, Clone)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub campaign_id: Option<Uuid>,
    pub kind: String,
    pub title: String,
    pub body: Option<String>,
    pub ref_kind: Option<String>,
    pub ref_id: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub read_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct ListQ {
    pub limit: Option<i64>,
    pub unread_only: Option<bool>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Notification>>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let only_unread = q.unread_only.unwrap_or(false);
    let rows: Vec<Notification> = if only_unread {
        sqlx::query_as::<_, Notification>(
            "select id, user_id, campaign_id, kind, title, body, ref_kind, ref_id, read_at, created_at
             from notifications
             where user_id = $1 and read_at is null
             order by created_at desc limit $2")
            .bind(uid).bind(limit).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Notification>(
            "select id, user_id, campaign_id, kind, title, body, ref_kind, ref_id, read_at, created_at
             from notifications
             where user_id = $1
             order by created_at desc limit $2")
            .bind(uid).bind(limit).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn mark_one(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let res = sqlx::query(
        "update notifications set read_at = coalesce(read_at, now())
         where id = $1 and user_id = $2")
        .bind(id).bind(uid).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

async fn mark_all(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<StatusCode> {
    sqlx::query("update notifications set read_at = now() where user_id = $1 and read_at is null")
        .bind(uid).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_one(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let res = sqlx::query("delete from notifications where id = $1 and user_id = $2")
        .bind(id).bind(uid).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

// ---- emit helper -----------------------------------------------------------

pub struct NewNotif<'a> {
    pub user_id: Uuid,
    pub campaign_id: Option<Uuid>,
    pub kind: &'a str,
    pub title: &'a str,
    pub body: Option<&'a str>,
    pub ref_kind: Option<&'a str>,
    pub ref_id: Option<Uuid>,
}

/// Insert a notification row and push a WS event to the user's channel.
/// Never fails the caller — best-effort background side effect.
pub async fn emit(db: &PgPool, n: NewNotif<'_>) {
    let res: Result<Notification, _> = sqlx::query_as::<_, Notification>(
        "insert into notifications (user_id, campaign_id, kind, title, body, ref_kind, ref_id)
         values ($1, $2, $3, $4, $5, $6, $7)
         returning id, user_id, campaign_id, kind, title, body, ref_kind, ref_id, read_at, created_at")
        .bind(n.user_id).bind(n.campaign_id).bind(n.kind).bind(n.title).bind(n.body).bind(n.ref_kind).bind(n.ref_id)
        .fetch_one(db).await;
    let row = match res {
        Ok(r) => r,
        Err(e) => { tracing::warn!(error=%e, "failed to persist notification"); return; }
    };
    let ev = json!({
        "type": "notification",
        "notification": row,
    });
    ws::publish_user(n.user_id, ev.to_string());
}

/// Emit a notification to every member of a campaign — convenience for
/// broadcast events (chat, combat round, etc.).
pub async fn emit_campaign(
    db: &PgPool,
    campaign_id: Uuid,
    exclude_user: Option<Uuid>,
    kind: &str,
    title: &str,
    body: Option<&str>,
    ref_kind: Option<&str>,
    ref_id: Option<Uuid>,
) {
    let members: Vec<Uuid> = match sqlx::query_scalar::<_, Uuid>(
        "select user_id from memberships where campaign_id = $1").bind(campaign_id).fetch_all(db).await {
        Ok(v) => v,
        Err(e) => { tracing::warn!(error=%e, "failed to fetch members"); return; }
    };
    for m in members {
        if Some(m) == exclude_user { continue; }
        emit(db, NewNotif {
            user_id: m, campaign_id: Some(campaign_id),
            kind, title, body, ref_kind, ref_id,
        }).await;
    }
}
