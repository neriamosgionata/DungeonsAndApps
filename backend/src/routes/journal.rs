// Player journal: private per-author notes within a campaign.
use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser, rbac};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/journal", get(list).post(create))
        .route("/journal/{id}", axum::routing::patch(update).delete(delete))
}

#[derive(Debug, Serialize, FromRow)]
pub struct JournalEntry {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub author_id: Uuid,
    pub title: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: time::OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: time::OffsetDateTime,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<JournalEntry>>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let rows: Vec<JournalEntry> = sqlx::query_as::<_, JournalEntry>(
        "select id, campaign_id, author_id, title, body, created_at, updated_at
         from journal_entries where campaign_id = $1 and author_id = $2
         order by updated_at desc",
    )
    .bind(cid)
    .bind(uid)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize, Validate)]
pub struct JournalCreate {
    #[validate(length(min = 1, max = 120))]
    pub title: String,
    #[validate(length(max = 20000))]
    pub body: String,
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<JournalCreate>,
) -> AppResult<(StatusCode, Json<JournalEntry>)> {
    body.validate()?;
    rbac::require_member(&s.db, uid, cid).await?;
    let row: JournalEntry = sqlx::query_as::<_, JournalEntry>(
        "insert into journal_entries (campaign_id, author_id, title, body)
         values ($1, $2, $3, $4)
         returning id, campaign_id, author_id, title, body, created_at, updated_at",
    )
    .bind(cid)
    .bind(uid)
    .bind(&body.title)
    .bind(&body.body)
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(row)))
}

#[derive(Debug, Deserialize, Validate)]
pub struct JournalUpdate {
    #[validate(length(min = 1, max = 120))]
    pub title: Option<String>,
    #[validate(length(max = 20000))]
    pub body: Option<String>,
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<JournalUpdate>,
) -> AppResult<Json<JournalEntry>> {
    body.validate()?;
    let row: JournalEntry = sqlx::query_as::<_, JournalEntry>(
        "update journal_entries set
             title = coalesce($2, title),
             body = coalesce($3, body),
             updated_at = now()
           where id = $1 and author_id = $4
           returning id, campaign_id, author_id, title, body, created_at, updated_at",
    )
    .bind(id)
    .bind(&body.title)
    .bind(&body.body)
    .bind(uid)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let res = sqlx::query("delete from journal_entries where id = $1 and author_id = $2")
        .bind(id)
        .bind(uid)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}
