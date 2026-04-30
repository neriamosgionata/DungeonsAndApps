use crate::{
    AppState,
    auth::hash_password,
    error::{AppError, AppResult},
    extract::AuthUser,
    routes::auth::validate_password_strength,
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
        .route("/users", get(list))
        .route("/users/{id}", axum::routing::patch(update).delete(delete))
        .route("/users/{id}/reset-password", post(reset_password))
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserRow {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub language: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

async fn require_admin(db: &sqlx::PgPool, uid: Uuid) -> AppResult<()> {
    let role: String = sqlx::query_scalar("select role::text from users where id = $1")
        .bind(uid).fetch_optional(db).await?.ok_or(AppError::Unauthorized)?;
    if role != "admin" { return Err(AppError::Forbidden); }
    Ok(())
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<Json<Vec<UserRow>>> {
    require_admin(&s.db, uid).await?;
    let rows: Vec<UserRow> = sqlx::query_as::<_, UserRow>(
        "select id, email::text as email, display_name, role::text as role,
                language::text as language, created_at
         from users order by created_at"
    ).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize, Validate)]
pub struct UserUpdate {
    #[validate(length(min = 1, max = 64))]
    pub display_name: Option<String>,
    pub role: Option<String>, // "user" | "admin"
    pub language: Option<String>, // "en" | "it"
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UserUpdate>,
) -> AppResult<Json<UserRow>> {
    body.validate()?;
    require_admin(&s.db, uid).await?;

    if let Some(r) = &body.role {
        if r != "user" && r != "admin" {
            return Err(AppError::BadRequest("invalid role".into()));
        }
        // prevent demoting the last admin
        if r == "user" {
            let target_role: String = sqlx::query_scalar("select role::text from users where id = $1")
                .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
            if target_role == "admin" {
                let admins: i64 = sqlx::query_scalar("select count(*) from users where role = 'admin'")
                    .fetch_one(&s.db).await?;
                if admins <= 1 {
                    return Err(AppError::BadRequest("cannot demote the last admin".into()));
                }
            }
        }
    }
    if let Some(l) = &body.language {
        if l != "en" && l != "it" {
            return Err(AppError::BadRequest("invalid language".into()));
        }
    }

    let row: UserRow = sqlx::query_as::<_, UserRow>(
        r#"update users set
             display_name = coalesce($2, display_name),
             role         = coalesce($3::user_role, role),
             language     = coalesce($4::language_code, language)
           where id = $1
           returning id, email::text as email, display_name, role::text as role,
                     language::text as language, created_at"#,
    )
    .bind(id).bind(body.display_name).bind(body.role).bind(body.language)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    Ok(Json(row))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    require_admin(&s.db, uid).await?;
    if id == uid {
        return Err(AppError::BadRequest("cannot delete yourself".into()));
    }
    // don't allow deleting the last admin
    let target_role: String = sqlx::query_scalar("select role::text from users where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    if target_role == "admin" {
        let admins: i64 = sqlx::query_scalar("select count(*) from users where role = 'admin'")
            .fetch_one(&s.db).await?;
        if admins <= 1 {
            return Err(AppError::BadRequest("cannot delete the last admin".into()));
        }
    }
    let res = sqlx::query("delete from users where id = $1")
        .bind(id).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct ResetPassword {
    #[validate(custom(function = "validate_password_strength"))]
    pub new_password: String,
}

async fn reset_password(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ResetPassword>,
) -> AppResult<StatusCode> {
    body.validate()?;
    require_admin(&s.db, uid).await?;
    let hash = hash_password(&body.new_password)?;
    let res = sqlx::query("update users set password_hash = $2 where id = $1")
        .bind(id).bind(&hash).execute(&s.db).await?;
    if res.rows_affected() == 0 { return Err(AppError::NotFound); }
    // revoke any active refresh sessions
    sqlx::query("update sessions_auth set revoked_at = now() where user_id = $1 and revoked_at is null")
        .bind(id).execute(&s.db).await.ok();
    Ok(StatusCode::NO_CONTENT)
}
