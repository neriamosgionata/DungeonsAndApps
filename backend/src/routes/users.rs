use crate::{
    AppState,
    auth::{hash_password, verify_password},
    error::{AppError, AppResult},
    extract::AuthUser,
    routes::auth::{validate_password_strength, UserDto},
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
        .route("/users", get(list).post(create))
        .route("/users/me", get(me).patch(update_me))
        .route("/users/me/change-password", post(change_password))
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
pub struct CreateUser {
    #[validate(email)]
    pub email: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
    #[validate(length(min = 1, max = 64))]
    pub display_name: String,
    pub role: Option<String>,
    pub language: Option<String>,
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<CreateUser>,
) -> AppResult<(StatusCode, Json<UserRow>)> {
    body.validate()?;
    require_admin(&s.db, uid).await?;

    let role = body.role.as_deref().unwrap_or("user");
    if role != "user" && role != "admin" {
        return Err(AppError::BadRequest("invalid role".into()));
    }
    let lang = body.language.as_deref().unwrap_or("en");
    if lang != "en" && lang != "it" {
        return Err(AppError::BadRequest("invalid language".into()));
    }

    let hash = hash_password(&body.password)?;
    let row: UserRow = sqlx::query_as::<_, UserRow>(
        r#"insert into users (email, password_hash, display_name, language, role)
           values ($1, $2, $3, $4::language_code, $5::user_role)
           returning id, email::text as email, display_name, role::text as role,
                     language::text as language, created_at"#,
    )
    .bind(&body.email)
    .bind(&hash)
    .bind(&body.display_name)
    .bind(lang)
    .bind(role)
    .fetch_one(&s.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() =>
            AppError::Conflict("email exists".into()),
        other => other.into(),
    })?;

    Ok((StatusCode::CREATED, Json(row)))
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

// =====================================================================
// Self-service endpoints
// =====================================================================

async fn me(State(s): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<UserDto>> {
    let user: UserDto = sqlx::query_as::<_, UserDto>(
        r#"select id, email, display_name, role::text as role, language::text as language, avatar_url, token_version, created_at
           from users where id = $1"#,
    )
    .bind(uid)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(user))
}

#[derive(Debug, Deserialize, Validate)]
pub struct SelfUpdate {
    #[validate(length(min = 1, max = 64))]
    pub display_name: Option<String>,
    pub language: Option<String>, // "en" | "it"
}

async fn update_me(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<SelfUpdate>,
) -> AppResult<Json<UserDto>> {
    body.validate()?;
    if let Some(l) = &body.language {
        if l != "en" && l != "it" {
            return Err(AppError::BadRequest("invalid language".into()));
        }
    }
    let user: UserDto = sqlx::query_as::<_, UserDto>(
        r#"update users set
             display_name = coalesce($2, display_name),
             language     = coalesce($3::language_code, language)
           where id = $1
           returning id, email, display_name, role::text as role, language::text as language, avatar_url, token_version, created_at"#,
    )
    .bind(uid).bind(body.display_name).bind(body.language)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    Ok(Json(user))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ChangePassword {
    pub current_password: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub new_password: String,
}

async fn change_password(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<ChangePassword>,
) -> AppResult<StatusCode> {
    body.validate()?;
    let row: Option<String> = sqlx::query_scalar("select password_hash from users where id = $1")
        .bind(uid).fetch_optional(&s.db).await?;
    let hash = row.ok_or(AppError::NotFound)?;
    if !verify_password(&body.current_password, &hash)? {
        return Err(AppError::Unauthorized);
    }
    let new_hash = hash_password(&body.new_password)?;
    sqlx::query("update users set password_hash = $2 where id = $1")
        .bind(uid).bind(&new_hash).execute(&s.db).await?;
    // revoke any active refresh sessions
    sqlx::query("update sessions_auth set revoked_at = now() where user_id = $1 and revoked_at is null")
        .bind(uid).execute(&s.db).await.ok();
    Ok(StatusCode::NO_CONTENT)
}
