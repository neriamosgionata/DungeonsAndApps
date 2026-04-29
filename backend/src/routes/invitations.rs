use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    routes::notifications::{self as notif, NewNotif},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/invitations", get(list_mine))
        .route("/campaigns/{id}/invitations", get(list_campaign).post(create))
        .route("/invitations/{id}/accept", post(accept))
        .route("/invitations/{id}/decline", post(decline))
        .route("/invitations/{id}", axum::routing::delete(revoke))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Invitation {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub user_id: Uuid,
    pub role: String,
    pub message: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub accepted: Option<bool>,
    pub campaign_name: String,
    pub inviter_name: Option<String>,
}

async fn list_mine(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<Json<Vec<Invitation>>> {
    let rows: Vec<Invitation> = sqlx::query_as::<_, Invitation>(
        r#"select i.id, i.campaign_id, i.user_id, i.role::text as role, i.message, i.created_at, i.accepted,
                  c.name as campaign_name,
                  u.display_name as inviter_name
           from campaign_invitations i
           join campaigns c on c.id = i.campaign_id
           left join users u on u.id = i.invited_by
           where i.user_id = $1 and i.responded_at is null
           order by i.created_at desc"#,
    )
    .bind(uid).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

async fn list_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Invitation>>> {
    rbac::require_master(&s.db, uid, cid).await?;
    let rows: Vec<Invitation> = sqlx::query_as::<_, Invitation>(
        r#"select i.id, i.campaign_id, i.user_id, i.role::text as role, i.message, i.created_at, i.accepted,
                  c.name as campaign_name, u.display_name as inviter_name
           from campaign_invitations i
           join campaigns c on c.id = i.campaign_id
           left join users u on u.id = i.invited_by
           where i.campaign_id = $1 and i.responded_at is null
           order by i.created_at desc"#,
    )
    .bind(cid).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateReq {
    #[validate(email)]
    pub email: String,
    /// "player" or "master"
    pub role: Option<String>,
    #[validate(length(max = 500))]
    pub message: Option<String>,
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CreateReq>,
) -> AppResult<(StatusCode, Json<Invitation>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;

    let role = body.role.as_deref().unwrap_or("player");
    if role != "player" && role != "master" {
        return Err(AppError::BadRequest("invalid role".into()));
    }

    let target: Uuid = sqlx::query_scalar("select id from users where email = $1")
        .bind(&body.email).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    // already a member?
    let exists_member: Option<i64> = sqlx::query_scalar(
        "select 1 from memberships where campaign_id = $1 and user_id = $2")
        .bind(cid).bind(target).fetch_optional(&s.db).await?;
    if exists_member.is_some() {
        return Err(AppError::Conflict("already a member".into()));
    }

    // upsert pending invitation
    sqlx::query(
        "insert into campaign_invitations (campaign_id, user_id, role, invited_by, message)
         values ($1, $2, $3::membership_role, $4, $5)
         on conflict (campaign_id, user_id) do update
           set role = excluded.role,
               message = excluded.message,
               invited_by = excluded.invited_by,
               responded_at = null,
               accepted = null,
               created_at = now()")
        .bind(cid).bind(target).bind(role).bind(uid).bind(&body.message)
        .execute(&s.db).await?;

    let inv: Invitation = sqlx::query_as::<_, Invitation>(
        r#"select i.id, i.campaign_id, i.user_id, i.role::text as role, i.message, i.created_at, i.accepted,
                  c.name as campaign_name, u.display_name as inviter_name
           from campaign_invitations i
           join campaigns c on c.id = i.campaign_id
           left join users u on u.id = i.invited_by
           where i.campaign_id = $1 and i.user_id = $2"#)
        .bind(cid).bind(target).fetch_one(&s.db).await?;

    // notify invitee
    notif::emit(&s.db, NewNotif {
        user_id: target, campaign_id: Some(cid),
        kind: "campaign.invitation",
        title: &format!("Invitation to {}", inv.campaign_name),
        body: body.message.as_deref().or(Some(&format!("Role: {role}"))),
        ref_kind: Some("invitation"), ref_id: Some(inv.id),
    }).await;

    Ok((StatusCode::CREATED, Json(inv)))
}

async fn accept(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: Option<(Uuid, Uuid, String, Option<Uuid>)> = sqlx::query_as(
        "select campaign_id, user_id, role::text, invited_by from campaign_invitations
         where id = $1 and responded_at is null")
        .bind(id).fetch_optional(&s.db).await?;
    let (cid, target, role, inviter) = row.ok_or(AppError::NotFound)?;
    if target != uid { return Err(AppError::Forbidden); }

    let mut tx = s.db.begin().await?;
    sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1, $2, $3::membership_role)
                 on conflict (campaign_id, user_id) do update set role = excluded.role")
        .bind(cid).bind(uid).bind(&role).execute(&mut *tx).await?;
    sqlx::query("update campaign_invitations set responded_at = now(), accepted = true where id = $1")
        .bind(id).execute(&mut *tx).await?;
    tx.commit().await?;

    // notify the master who invited
    if let Some(inv_by) = inviter {
        let campaign_name: String = sqlx::query_scalar("select name from campaigns where id = $1")
            .bind(cid).fetch_one(&s.db).await.unwrap_or_default();
        let user_name: String = sqlx::query_scalar("select display_name from users where id = $1")
            .bind(uid).fetch_one(&s.db).await.unwrap_or_default();
        notif::emit(&s.db, NewNotif {
            user_id: inv_by, campaign_id: Some(cid),
            kind: "campaign.invite_accepted",
            title: &format!("{user_name} joined {campaign_name}"),
            body: None, ref_kind: Some("campaign"), ref_id: Some(cid),
        }).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn decline(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: Option<(Uuid, Uuid, Option<Uuid>)> = sqlx::query_as(
        "select user_id, campaign_id, invited_by from campaign_invitations
         where id = $1 and responded_at is null")
        .bind(id).fetch_optional(&s.db).await?;
    let (target, cid, inviter) = row.ok_or(AppError::NotFound)?;
    if target != uid { return Err(AppError::Forbidden); }

    sqlx::query("update campaign_invitations set responded_at = now(), accepted = false where id = $1")
        .bind(id).execute(&s.db).await?;

    if let Some(inv_by) = inviter {
        let campaign_name: String = sqlx::query_scalar("select name from campaigns where id = $1")
            .bind(cid).fetch_one(&s.db).await.unwrap_or_default();
        let user_name: String = sqlx::query_scalar("select display_name from users where id = $1")
            .bind(uid).fetch_one(&s.db).await.unwrap_or_default();
        notif::emit(&s.db, NewNotif {
            user_id: inv_by, campaign_id: Some(cid),
            kind: "campaign.invite_declined",
            title: &format!("{user_name} declined {campaign_name}"),
            body: None, ref_kind: Some("campaign"), ref_id: Some(cid),
        }).await;
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn revoke(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: Option<(Uuid,)> = sqlx::query_as("select campaign_id from campaign_invitations where id = $1")
        .bind(id).fetch_optional(&s.db).await?;
    let (cid,) = row.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from campaign_invitations where id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
