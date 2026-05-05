use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser};
use axum::{Json, Router, extract::{Path, State}, http::StatusCode, routing::{delete, get}};
use serde::Serialize;
use time::OffsetDateTime;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/admin/stats", get(stats))
        .route("/admin/campaigns", get(list_campaigns))
        .route("/admin/campaigns/{id}", delete(delete_campaign))
}

async fn require_admin(db: &sqlx::PgPool, uid: Uuid) -> AppResult<()> {
    let role: String = sqlx::query_scalar("select role::text from users where id = $1")
        .bind(uid).fetch_optional(db).await?.ok_or(AppError::Unauthorized)?;
    if role != "admin" { return Err(AppError::Forbidden); }
    Ok(())
}

#[derive(Serialize)]
pub struct Stats {
    pub users: i64,
    pub campaigns: i64,
    pub characters: i64,
    pub messages: i64,
    pub encounters: i64,
    pub spells: i64,
}

async fn stats(State(s): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<Stats>> {
    require_admin(&s.db, uid).await?;
    let (users, campaigns, characters, messages, encounters, spells): (i64,i64,i64,i64,i64,i64) = tokio::try_join!(
        sqlx::query_scalar("select count(*) from users").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from campaigns").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from characters").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from messages").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from encounters").fetch_one(&s.db),
        sqlx::query_scalar("select count(*) from spells").fetch_one(&s.db),
    )?;
    Ok(Json(Stats { users, campaigns, characters, messages, encounters, spells }))
}

#[derive(Serialize)]
pub struct CampaignRow {
    pub id: Uuid,
    pub name: String,
    pub owner_name: String,
    pub member_count: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

async fn list_campaigns(State(s): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<Vec<CampaignRow>>> {
    require_admin(&s.db, uid).await?;
    let rows = sqlx::query_as::<_, (Uuid, String, String, i64, OffsetDateTime)>(
        r#"select c.id,
                  c.name,
                  coalesce(
                    (select u.display_name from memberships ms
                     join users u on u.id = ms.user_id
                     where ms.campaign_id = c.id and ms.role = 'master'
                     limit 1),
                    'Unknown'
                  ) as owner_name,
                  (select count(*) from memberships m where m.campaign_id = c.id) as member_count,
                  c.created_at
           from campaigns c
           order by c.created_at desc"#,
    )
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(id, name, owner_name, member_count, created_at)| CampaignRow { id, name, owner_name, member_count, created_at })
    .collect();
    Ok(Json(rows))
}

async fn delete_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&s.db, uid).await?;
    let res = sqlx::query("delete from campaigns where id = $1")
        .bind(id).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}
