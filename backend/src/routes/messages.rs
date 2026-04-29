use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    routes::notifications::{self as notif, NewNotif},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
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
        .route("/campaigns/{id}/messages", get(list).post(post_msg))
        .route("/messages/{id}", axum::routing::delete(delete_msg))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub scope: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MessagePost {
    #[validate(length(min = 1, max = 4000))]
    pub body: String,
    pub scope: String, // "campaign" | "whisper"
    pub recipient_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ListQ {
    pub limit: Option<i64>,
    pub with_user: Option<Uuid>,
    pub whispers: Option<bool>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Query(q): Query<ListQ>,
) -> AppResult<Json<Vec<Message>>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);

    let rows: Vec<Message> = if q.whispers.unwrap_or(false) {
        // whispers involving uid
        if let Some(other) = q.with_user {
            sqlx::query_as::<_, Message>(
                "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, created_at
                 from messages
                 where campaign_id = $1 and scope = 'whisper' and deleted_at is null
                   and ((sender_id = $2 and recipient_id = $3) or (sender_id = $3 and recipient_id = $2))
                 order by created_at desc limit $4")
                .bind(cid).bind(uid).bind(other).bind(limit).fetch_all(&s.db).await?
        } else {
            sqlx::query_as::<_, Message>(
                "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, created_at
                 from messages
                 where campaign_id = $1 and scope = 'whisper' and deleted_at is null
                   and (sender_id = $2 or recipient_id = $2)
                 order by created_at desc limit $3")
                .bind(cid).bind(uid).bind(limit).fetch_all(&s.db).await?
        }
    } else {
        // campaign chat
        sqlx::query_as::<_, Message>(
            "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, created_at
             from messages
             where campaign_id = $1 and scope = 'campaign' and deleted_at is null
             order by created_at desc limit $2")
            .bind(cid).bind(limit).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn post_msg(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<MessagePost>,
) -> AppResult<(StatusCode, Json<Message>)> {
    body.validate()?;
    rbac::require_member(&s.db, uid, cid).await?;
    if body.scope != "campaign" && body.scope != "whisper" {
        return Err(AppError::BadRequest("invalid scope".into()));
    }
    if body.scope == "whisper" && body.recipient_id.is_none() {
        return Err(AppError::BadRequest("whisper requires recipient".into()));
    }
    if body.scope == "campaign" && body.recipient_id.is_some() {
        return Err(AppError::BadRequest("campaign message has no recipient".into()));
    }
    // if whisper, recipient must be member too
    if let Some(r) = body.recipient_id {
        rbac::require_member(&s.db, r, cid).await?;
    }
    let m: Message = sqlx::query_as::<_, Message>(
        "insert into messages (campaign_id, sender_id, recipient_id, scope, body)
         values ($1, $2, $3, $4::message_scope, $5)
         returning id, campaign_id, sender_id, recipient_id, scope::text as scope, body, created_at")
        .bind(cid).bind(uid).bind(body.recipient_id).bind(&body.scope).bind(&body.body)
        .fetch_one(&s.db).await?;

    // sender display name for notif body
    let sender_name: String = sqlx::query_scalar("select display_name from users where id = $1")
        .bind(uid).fetch_one(&s.db).await.unwrap_or_else(|_| "Someone".to_string());

    // broadcast campaign chat; whispers not broadcast globally (WS filters client-side or use user channel later)
    if m.scope == "campaign" {
        ws::publish(cid, json!({
            "type":"message",
            "id": m.id,
            "sender_id": m.sender_id,
            "scope": "campaign",
            "body": m.body,
            "created_at": m.created_at.unix_timestamp(),
        }).to_string());
        let preview = truncate(&m.body, 120);
        notif::emit_campaign(&s.db, cid, Some(uid),
            "chat.message", &format!("{sender_name} sent a message"),
            Some(&preview), Some("message"), Some(m.id)).await;
    } else {
        ws::publish(cid, json!({
            "type":"whisper",
            "id": m.id,
            "sender_id": m.sender_id,
            "recipient_id": m.recipient_id,
            "created_at": m.created_at.unix_timestamp(),
        }).to_string());
        if let Some(rid) = m.recipient_id {
            let preview = truncate(&m.body, 120);
            // ref_id = sender's user id so the frontend can pre-select the
            // whisper conversation when the recipient opens the notification.
            notif::emit(&s.db, NewNotif {
                user_id: rid, campaign_id: Some(cid),
                kind: "chat.whisper",
                title: &format!("{sender_name} whispered you"),
                body: Some(&preview),
                ref_kind: Some("whisper"), ref_id: Some(m.sender_id),
            }).await;
        }
    }
    Ok((StatusCode::CREATED, Json(m)))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max { return s.to_string(); }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

async fn delete_msg(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "select campaign_id, sender_id from messages where id = $1 and deleted_at is null")
        .bind(id).fetch_optional(&s.db).await?;
    let (cid, sender) = row.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, cid).await?;
    if sender != uid && role != crate::rbac::Role::Master {
        return Err(AppError::Forbidden);
    }
    sqlx::query("update messages set deleted_at = now() where id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
