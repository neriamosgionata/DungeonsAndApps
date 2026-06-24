use crate::{
    AppState,
    dice::roll,
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
    routing::{get, patch, post},
};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/messages", get(list).post(post_msg))
        .route("/messages/{id}", patch(edit_msg).delete(delete_msg))
        .route(
            "/messages/{id}/reactions",
            post(add_reaction).delete(remove_reaction),
        )
}

#[derive(Debug, Serialize, FromRow)]
pub struct Message {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub sender_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub scope: String,
    pub body: String,
    pub roll_result: Option<serde_json::Value>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub edited_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MessagePost {
    #[validate(length(min = 1, max = 4000))]
    pub body: String,
    pub scope: String, // "campaign" | "whisper"
    pub recipient_id: Option<Uuid>,
    /// When true (or when `body` starts with "/roll "), the trailing dice
    /// expression is rolled server-side and stored in `roll_result`.
    #[serde(default)]
    pub is_roll: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MessageUpdate {
    #[validate(length(min = 1, max = 4000))]
    pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListQ {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
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
    let offset = q.offset.unwrap_or(0).max(0);

    let rows: Vec<Message> = if q.whispers.unwrap_or(false) {
        // whispers involving uid
        if let Some(other) = q.with_user {
            sqlx::query_as::<_, Message>(
                "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, roll_result, created_at, edited_at
                 from messages
                 where campaign_id = $1 and scope = 'whisper' and deleted_at is null
                   and ((sender_id = $2 and recipient_id = $3) or (sender_id = $3 and recipient_id = $2))
                 order by created_at desc limit $4 offset $5")
                .bind(cid).bind(uid).bind(other).bind(limit).bind(offset).fetch_all(&s.db).await?
        } else {
            sqlx::query_as::<_, Message>(
                "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, roll_result, created_at, edited_at
                 from messages
                 where campaign_id = $1 and scope = 'whisper' and deleted_at is null
                   and (sender_id = $2 or recipient_id = $2)
                 order by created_at desc limit $3 offset $4")
                .bind(cid).bind(uid).bind(limit).bind(offset).fetch_all(&s.db).await?
        }
    } else {
        // campaign chat
        sqlx::query_as::<_, Message>(
            "select id, campaign_id, sender_id, recipient_id, scope::text as scope, body, roll_result, created_at, edited_at
             from messages
             where campaign_id = $1 and scope = 'campaign' and deleted_at is null
             order by created_at desc limit $2 offset $3")
            .bind(cid).bind(limit).bind(offset).fetch_all(&s.db).await?
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
        return Err(AppError::BadRequest(
            "campaign message has no recipient".into(),
        ));
    }
    // if whisper, recipient must be member too
    if let Some(r) = body.recipient_id {
        rbac::require_member(&s.db, r, cid).await?;
    }

    // Inline dice: "/roll 1d20+5" (or is_roll=true with a bare expression) is
    // rolled server-side. The expression is the text after the "/roll " prefix,
    // or the whole body when is_roll is set without a prefix.
    let roll_expr = body
        .body
        .strip_prefix("/roll ")
        .or_else(|| body.body.strip_prefix("/r "))
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .or_else(|| body.is_roll.then(|| body.body.trim()).filter(|e| !e.is_empty()));
    let roll_result = match roll_expr {
        Some(expr) => {
            let mut rng = StdRng::from_os_rng();
            let r = roll(expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
            Some(serde_json::to_value(&r)?)
        }
        None => None,
    };

    let m: Message = sqlx::query_as::<_, Message>(
        "insert into messages (campaign_id, sender_id, recipient_id, scope, body, roll_result)
         values ($1, $2, $3, $4::message_scope, $5, $6)
         returning id, campaign_id, sender_id, recipient_id, scope::text as scope, body, roll_result, created_at, edited_at")
        .bind(cid).bind(uid).bind(body.recipient_id).bind(&body.scope).bind(&body.body).bind(&roll_result)
        .fetch_one(&s.db).await?;

    // sender display name for notif body
    let sender_name: String = sqlx::query_scalar("select display_name from users where id = $1")
        .bind(uid)
        .fetch_one(&s.db)
        .await
        .unwrap_or_else(|_| "Someone".to_string());

    // broadcast campaign chat; whispers not broadcast globally (WS filters client-side or use user channel later)
    if m.scope == "campaign" {
        ws::publish(
            cid,
            json!({
                "type":"message",
                "id": m.id,
                "sender_id": m.sender_id,
                "scope": "campaign",
                "body": m.body,
                "roll_result": m.roll_result,
                "created_at": m.created_at.unix_timestamp(),
            })
            .to_string(),
        );
        let preview = truncate(&m.body, 120);
        notif::emit_campaign(
            &s.db,
            cid,
            Some(uid),
            "chat.message",
            &format!("{sender_name} sent a message"),
            Some(&preview),
            Some("message"),
            Some(m.id),
        )
        .await;
        // @mentions: notify each mentioned member directly. Parse "@name"
        // tokens from the body and match them against members' display names
        // (case-insensitive). Self-mentions and the sender are skipped.
        notify_mentions(&s.db, cid, uid, &m, &sender_name).await;
    } else {
        // Whispers must NOT broadcast on the campaign channel — that leaks
        // sender/recipient metadata to everyone. Send to the two parties'
        // per-user channels only. Include campaign_id so the client can
        // refresh the chat only when the event belongs to the open campaign.
        let ev = json!({
            "type":"whisper",
            "id": m.id,
            "campaign_id": cid,
            "sender_id": m.sender_id,
            "recipient_id": m.recipient_id,
            "created_at": m.created_at.unix_timestamp(),
        })
        .to_string();
        ws::publish_user(m.sender_id, ev.clone());
        if let Some(rid) = m.recipient_id {
            ws::publish_user(rid, ev);
        }
        if let Some(rid) = m.recipient_id {
            let preview = truncate(&m.body, 120);
            // ref_id = sender's user id so the frontend can pre-select the
            // whisper conversation when the recipient opens the notification.
            notif::emit(
                &s.db,
                NewNotif {
                    user_id: rid,
                    campaign_id: Some(cid),
                    kind: "chat.whisper",
                    title: &format!("{sender_name} whispered you"),
                    body: Some(&preview),
                    ref_kind: Some("whisper"),
                    ref_id: Some(m.sender_id),
                },
            )
            .await;
        }
    }
    Ok((StatusCode::CREATED, Json(m)))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Extract `@name` tokens from a body. A name is a single run of word chars
/// (letters, digits, `_`, `-`) right after `@`, so "@GM!" matches "GM".
fn parse_mentions(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    for raw in body.split('@').skip(1) {
        let name: String = raw
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
            .collect();
        if !name.is_empty() {
            out.push(name);
        }
    }
    out
}

/// Notify campaign members whose display name is @mentioned in a message.
async fn notify_mentions(
    db: &sqlx::PgPool,
    cid: Uuid,
    sender: Uuid,
    m: &Message,
    sender_name: &str,
) {
    let names = parse_mentions(&m.body);
    if names.is_empty() {
        return;
    }
    // Resolve mentioned display names to member user ids in one query.
    let lowered: Vec<String> = names.iter().map(|n| n.to_lowercase()).collect();
    let rows: Vec<Uuid> = match sqlx::query_scalar::<_, Uuid>(
        "select u.id from users u
         join memberships mem on mem.user_id = u.id
         where mem.campaign_id = $1 and lower(u.display_name) = any($2) and u.id <> $3",
    )
    .bind(cid)
    .bind(&lowered)
    .bind(sender)
    .fetch_all(db)
    .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!(error=%e, "notify_mentions: lookup failed");
            return;
        }
    };
    let preview = truncate(&m.body, 120);
    for rid in rows {
        notif::emit(
            db,
            NewNotif {
                user_id: rid,
                campaign_id: Some(cid),
                kind: "chat.mention",
                title: &format!("{sender_name} mentioned you"),
                body: Some(&preview),
                ref_kind: Some("message"),
                ref_id: Some(m.id),
            },
        )
        .await;
    }
}

async fn edit_msg(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MessageUpdate>,
) -> AppResult<Json<Message>> {
    body.validate()?;
    let row: Option<(Uuid, Uuid, String, Option<Uuid>, OffsetDateTime)> = sqlx::query_as(
        "select campaign_id, sender_id, scope::text, recipient_id, created_at \
         from messages where id = $1 and deleted_at is null",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?;
    let (cid, sender, scope, recipient, created_at) = row.ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, cid).await?;
    if sender != uid {
        return Err(AppError::Forbidden);
    }
    let now = OffsetDateTime::now_utc();
    if (now - created_at).whole_minutes() > 5 {
        return Err(AppError::BadRequest(
            "message can no longer be edited".into(),
        ));
    }
    let Some(new_body) = body.body else {
        return Err(AppError::BadRequest("body is required".into()));
    };
    let m: Message = sqlx::query_as::<_, Message>(
        "update messages set body = $2, edited_at = now()
         where id = $1
         returning id, campaign_id, sender_id, recipient_id, scope::text as scope, body, roll_result, created_at, edited_at")
        .bind(id).bind(&new_body).fetch_one(&s.db).await?;

    let ev = json!({"type":"message_edited","id":m.id,"body":m.body,"edited_at":m.edited_at})
        .to_string();
    if scope == "whisper" {
        ws::publish_user(sender, ev.clone());
        if let Some(rid) = recipient {
            ws::publish_user(rid, ev);
        }
    } else {
        ws::publish(cid, ev);
    }
    Ok(Json(m))
}

async fn delete_msg(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: Option<(Uuid, Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "select campaign_id, sender_id, scope::text, recipient_id \
         from messages where id = $1 and deleted_at is null",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?;
    let (cid, sender, scope, recipient) = row.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, cid).await?;
    if sender != uid && role != crate::rbac::Role::Master {
        return Err(AppError::Forbidden);
    }
    sqlx::query("update messages set deleted_at = now() where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;

    // Broadcast deletion so other clients drop it from their UI immediately.
    // Whispers route only to the two parties; campaign messages to the channel.
    let ev = json!({"type":"message_deleted","id":id,"campaign_id":cid}).to_string();
    if scope == "whisper" {
        ws::publish_user(sender, ev.clone());
        if let Some(rid) = recipient {
            ws::publish_user(rid, ev);
        }
    } else {
        ws::publish(cid, ev);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct ReactionBody {
    #[validate(length(min = 1, max = 16))]
    pub emoji: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ReactionGroup {
    pub emoji: String,
    pub count: i64,
    pub user_ids: Vec<Uuid>,
}

/// Aggregate a message's reactions into one row per emoji.
async fn reaction_groups(db: &sqlx::PgPool, msg_id: Uuid) -> AppResult<Vec<ReactionGroup>> {
    let rows: Vec<ReactionGroup> = sqlx::query_as::<_, ReactionGroup>(
        "select emoji, count(*) as count, array_agg(user_id) as user_ids
         from message_reactions where message_id = $1
         group by emoji order by min(created_at)",
    )
    .bind(msg_id)
    .fetch_all(db)
    .await?;
    Ok(rows)
}

/// Resolve a message's campaign + scope + parties for reaction routing.
async fn message_ctx(
    db: &sqlx::PgPool,
    id: Uuid,
) -> AppResult<(Uuid, Uuid, String, Option<Uuid>)> {
    let row: Option<(Uuid, Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "select campaign_id, sender_id, scope::text, recipient_id \
         from messages where id = $1 and deleted_at is null",
    )
    .bind(id)
    .fetch_optional(db)
    .await?;
    row.ok_or(AppError::NotFound)
}

fn broadcast_reactions(
    cid: Uuid,
    id: Uuid,
    scope: &str,
    sender: Uuid,
    recipient: Option<Uuid>,
    groups: &[ReactionGroup],
) {
    let ev = json!({
        "type": "message_reactions",
        "id": id,
        "campaign_id": cid,
        "reactions": groups,
    })
    .to_string();
    if scope == "whisper" {
        ws::publish_user(sender, ev.clone());
        if let Some(rid) = recipient {
            ws::publish_user(rid, ev);
        }
    } else {
        ws::publish(cid, ev);
    }
}

async fn add_reaction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactionBody>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    let (cid, sender, scope, recipient) = message_ctx(&s.db, id).await?;
    rbac::require_member(&s.db, uid, cid).await?;
    sqlx::query(
        "insert into message_reactions (message_id, user_id, emoji)
         values ($1, $2, $3) on conflict do nothing",
    )
    .bind(id)
    .bind(uid)
    .bind(&body.emoji)
    .execute(&s.db)
    .await?;
    let groups = reaction_groups(&s.db, id).await?;
    broadcast_reactions(cid, id, &scope, sender, recipient, &groups);
    Ok(Json(json!({ "id": id, "reactions": groups })))
}

async fn remove_reaction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactionBody>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    let (cid, sender, scope, recipient) = message_ctx(&s.db, id).await?;
    rbac::require_member(&s.db, uid, cid).await?;
    sqlx::query("delete from message_reactions where message_id = $1 and user_id = $2 and emoji = $3")
        .bind(id)
        .bind(uid)
        .bind(&body.emoji)
        .execute(&s.db)
        .await?;
    let groups = reaction_groups(&s.db, id).await?;
    broadcast_reactions(cid, id, &scope, sender, recipient, &groups);
    Ok(Json(json!({ "id": id, "reactions": groups })))
}
