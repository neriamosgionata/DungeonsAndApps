use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    routes::notifications::{NewNotif, emit},
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
use tracing::warn;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns", get(list).post(create))
        .route("/campaigns/{id}", get(read).patch(update).delete(delete))
        .route("/campaigns/{id}/archive", post(archive))
        .route("/campaigns/{id}/restore", post(restore))
        .route("/campaigns/{id}/calendar", get(get_calendar).patch(update_calendar))
        .route("/campaigns/{id}/calendar/advance", post(advance_calendar))
        .route(
            "/campaigns/{id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/campaigns/{id}/members/{user_id}",
            axum::routing::patch(update_member).delete(remove_member),
        )
        .route("/campaigns/{id}/presence", get(presence))
}

async fn presence(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<Uuid>>> {
    crate::rbac::require_master(&s.db, uid, id).await?;
    Ok(Json(crate::ws::online_users(id)))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub master_id: Uuid,
    pub icon_url: Option<String>,
    pub leveling: String,
    pub settings: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CampaignCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub leveling: Option<String>, // 'xp' | 'milestone'
    /// House rules / campaign settings (free-form jsonb, master-editable).
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CampaignUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub leveling: Option<String>, // 'xp' | 'milestone'
    /// House rules / campaign settings (free-form jsonb, master-editable).
    pub settings: Option<serde_json::Value>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<Json<Vec<Campaign>>> {
    let rows: Vec<Campaign> = sqlx::query_as::<_, Campaign>(
        r#"select c.id, c.name, c.description, c.master_id, c.icon_url,
                  c.leveling::text as leveling, c.settings, c.created_at, c.archived_at
           from campaigns c
           join memberships m on m.campaign_id = c.id
           where m.user_id = $1 and c.archived_at is null
           order by c.created_at desc"#,
    )
    .bind(uid)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<CampaignCreate>,
) -> AppResult<(StatusCode, Json<Campaign>)> {
    body.validate()?;

    // Any authenticated user may start a campaign — the membership below makes
    // them campaign master. App-wide admin role is separate (manages users).
    let mut tx = s.db.begin().await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "insert into campaigns (name, description, master_id, icon_url, leveling)
         values ($1, $2, $3, $4, coalesce($5::leveling_mode, 'xp'))
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(uid)
    .bind(&body.icon_url)
    .bind(&body.leveling)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1, $2, 'master')")
        .bind(c.id)
        .bind(uid)
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into parties (campaign_id) values ($1)")
        .bind(c.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into campaign_calendar (campaign_id) values ($1) on conflict do nothing")
        .bind(c.id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_member(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "select id, name, description, master_id, icon_url,
                leveling::text as leveling, settings, created_at, archived_at
         from campaigns where id = $1",
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(c))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CampaignUpdate>,
) -> AppResult<Json<Campaign>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        r#"update campaigns
           set name = coalesce($2, name),
               description = coalesce($3, description),
               icon_url = coalesce($4, icon_url),
               leveling = coalesce($5::leveling_mode, leveling),
               settings = coalesce($6, settings)
           where id = $1
           returning id, name, description, master_id, icon_url,
                     leveling::text as leveling, settings, created_at, archived_at"#,
    )
    .bind(id)
    .bind(body.name)
    .bind(body.description)
    .bind(body.icon_url)
    .bind(body.leveling)
    .bind(body.settings)
    .fetch_one(&s.db)
    .await?;
    crate::ws::publish(
        id,
        serde_json::json!({
            "type":"campaign_updated","id":id,"leveling":c.leveling
        })
        .to_string(),
    );
    Ok(Json(c))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Calendar {
    pub campaign_id: Uuid,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub days_per_month: i32,
    pub months: serde_json::Value,
    pub weekdays: serde_json::Value,
    pub notes: String,
}

async fn get_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Calendar>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        "select campaign_id, year, month, day, days_per_month, months, weekdays, notes
         from campaign_calendar where campaign_id = $1",
    )
    .bind(cid)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(cal))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CalendarAdvance {
    #[validate(range(min = 1, max = 3650))]
    pub days: i32,
}

async fn advance_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CalendarAdvance>,
) -> AppResult<Json<Calendar>> {
    body.validate()?;
    rbac::require_member(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        r#"with cal as (
             select campaign_id, year, month, day, days_per_month from campaign_calendar where campaign_id = $1
           )
           update campaign_calendar cc set
             year = y, month = m, day = d, updated_at = now()
           from (select
                   (select year from cal) + (($2 + (select day from cal) - 1) / (select days_per_month from cal) + (select month from cal) - 1) / 12 as y,
                   ((select month from cal) - 1 + ($2 + (select day from cal) - 1) / (select days_per_month from cal)) % 12 + 1 as m,
                   (($2 + (select day from cal) - 1) % (select days_per_month from cal)) + 1 as d
                ) adv
           where cc.campaign_id = $1
           returning cc.campaign_id, cc.year, cc.month, cc.day, cc.days_per_month, cc.months, cc.weekdays, cc.notes"#,
    )
    .bind(cid)
    .bind(body.days)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(cid, serde_json::json!({"type":"calendar_updated"}).to_string());
    Ok(Json(cal))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CalendarUpdate {
    #[validate(range(min = 1, max = 400))]
    pub days_per_month: Option<i32>,
    pub months: Option<serde_json::Value>,
    pub weekdays: Option<serde_json::Value>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
}

async fn update_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CalendarUpdate>,
) -> AppResult<Json<Calendar>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        r#"update campaign_calendar set
             days_per_month = coalesce($2, days_per_month),
             months = coalesce($3, months),
             weekdays = coalesce($4, weekdays),
             notes = coalesce($5, notes),
             updated_at = now()
           where campaign_id = $1
           returning campaign_id, year, month, day, days_per_month, months, weekdays, notes"#,
    )
    .bind(cid)
    .bind(body.days_per_month)
    .bind(body.months)
    .bind(body.weekdays)
    .bind(body.notes)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(cid, serde_json::json!({"type":"calendar_updated"}).to_string());
    Ok(Json(cal))
}

async fn archive(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "update campaigns set archived_at = now() where id = $1
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(c))
}

async fn restore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "update campaigns set archived_at = null where id = $1
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(c))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, id).await?;
    sqlx::query("delete from campaigns where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub character_limit: i32,
}

async fn list_members(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<Member>>> {
    rbac::require_member(&s.db, uid, id).await?;
    let rows: Vec<Member> = sqlx::query_as::<_, Member>(
        r#"select u.id as user_id, u.display_name, u.email::text as email, m.role::text as role, m.character_limit
           from memberships m join users u on u.id = m.user_id
           where m.campaign_id = $1 order by m.joined_at"#,
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct AddMember {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MemberUpdate {
    #[validate(range(min = 1, max = 20))]
    pub character_limit: Option<i32>,
    pub role: Option<String>,
}

async fn update_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((campaign_id, target)): Path<(Uuid, Uuid)>,
    Json(body): Json<MemberUpdate>,
) -> AppResult<Json<Member>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    if let Some(r) = &body.role {
        if r != "player" && r != "master" {
            return Err(AppError::BadRequest("invalid role".into()));
        }
    }
    sqlx::query(
        "update memberships set
           character_limit = coalesce($3, character_limit),
           role            = coalesce($4::membership_role, role)
         where campaign_id = $1 and user_id = $2",
    )
    .bind(campaign_id)
    .bind(target)
    .bind(body.character_limit)
    .bind(&body.role)
    .execute(&s.db)
    .await?;
    let m: Member = sqlx::query_as::<_, Member>(
        r#"select u.id as user_id, u.display_name, u.email::text as email, m.role::text as role, m.character_limit
           from memberships m join users u on u.id = m.user_id
           where m.campaign_id = $1 and m.user_id = $2"#,
    )
    .bind(campaign_id).bind(target).fetch_optional(&s.db).await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(
        campaign_id,
        serde_json::json!({
            "type": "member_updated", "user_id": target
        })
        .to_string(),
    );
    Ok(Json(m))
}

async fn remove_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((campaign_id, target)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, campaign_id).await?;
    let campaign_master: Uuid = sqlx::query_scalar("select master_id from campaigns where id = $1")
        .bind(campaign_id)
        .fetch_one(&s.db)
        .await?;
    if target == campaign_master {
        return Err(AppError::BadRequest("cannot remove campaign master".into()));
    }
    let res = sqlx::query("delete from memberships where campaign_id = $1 and user_id = $2")
        .bind(campaign_id)
        .bind(target)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::ws::publish(
        campaign_id,
        serde_json::json!({
            "type": "member_removed", "user_id": target
        })
        .to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn add_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMember>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    rbac::require_master(&s.db, uid, id).await?;
    if body.role != "player" && body.role != "master" {
        return Err(AppError::BadRequest("invalid role".into()));
    }
    let target: Uuid = sqlx::query_scalar("select id from users where email = $1")
        .bind(&body.email)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let already: Option<i64> =
        sqlx::query_scalar("select 1 from memberships where campaign_id = $1 and user_id = $2")
            .bind(id)
            .bind(target)
            .fetch_optional(&s.db)
            .await?;
    if already.is_some() {
        return Err(AppError::Conflict("already a member".into()));
    }

    sqlx::query(
        "insert into campaign_invitations (campaign_id, user_id, role, invited_by)
         values ($1, $2, $3::membership_role, $4)
         on conflict (campaign_id, user_id) do update
           set role = excluded.role, invited_by = excluded.invited_by,
               responded_at = null, accepted = null, created_at = now()",
    )
    .bind(id)
    .bind(target)
    .bind(&body.role)
    .bind(uid)
    .execute(&s.db)
    .await?;

    let inv_id: Uuid = sqlx::query_scalar(
        "select id from campaign_invitations where campaign_id = $1 and user_id = $2",
    )
    .bind(id)
    .bind(target)
    .fetch_one(&s.db)
    .await?;

    let campaign_name: String = sqlx::query_scalar("select name from campaigns where id = $1")
        .bind(id)
        .fetch_one(&s.db)
        .await
        .unwrap_or_else(|e| {
            warn!(%e, "campaign name lookup failed");
            String::new()
        });
    emit(
        &s.db,
        NewNotif {
            user_id: target,
            campaign_id: Some(id),
            kind: "campaign.invitation",
            title: &format!("Invitation to {campaign_name}"),
            body: Some(&format!("Role: {}", body.role)),
            ref_kind: Some("invitation"),
            ref_id: Some(inv_id),
        },
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "invitation_id": inv_id, "pending": true,
        })),
    ))
}
