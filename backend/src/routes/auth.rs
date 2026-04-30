use crate::{
    AppState,
    auth::{hash_password, issue_jwt, verify_password},
    error::{AppError, AppResult},
    extract::AuthUser,
};
use axum::{
    Json, Router,
    extract::{FromRequestParts, State},
    http::{StatusCode, request::Parts},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use std::net::SocketAddr;
use std::time::{Duration, Instant};
use time::OffsetDateTime;
use tokio::sync::Mutex;
use uuid::Uuid;
use validator::Validate;

// Simple in-memory rate limiting for login attempts with bounded memory.
// Uses LRU-style eviction: cleans up stale entries periodically.
#[derive(Clone)]
struct LoginAttempt {
    timestamps: Vec<Instant>,
    last_access: Instant,
}

static LOGIN_ATTEMPTS: Mutex<Option<dashmap::DashMap<String, LoginAttempt>>> = Mutex::const_new(None);
const LOGIN_WINDOW: Duration = Duration::from_secs(300); // 5 minutes
const LOGIN_MAX_ATTEMPTS: usize = 10; // Max 10 attempts per window
const MAX_TRACKED_IPS: usize = 10000; // Prevent unbounded memory growth

async fn check_login_rate_limit(addr: SocketAddr) -> AppResult<()> {
    let ip = addr.ip().to_string();
    let mut guard = LOGIN_ATTEMPTS.lock().await;
    if guard.is_none() {
        *guard = Some(dashmap::DashMap::new());
    }
    let map = guard.as_ref().unwrap();
    
    let now = Instant::now();
    
    // Cleanup: if map is getting large, remove stale entries
    if map.len() > MAX_TRACKED_IPS {
        let stale_keys: Vec<String> = map
            .iter()
            .filter(|e| now.duration_since(e.value().last_access) > LOGIN_WINDOW * 2)
            .map(|e| e.key().clone())
            .take(MAX_TRACKED_IPS / 2)
            .collect();
        for key in stale_keys {
            map.remove(&key);
        }
    }
    
    let mut entry = map.entry(ip).or_insert(LoginAttempt {
        timestamps: Vec::with_capacity(4),
        last_access: now,
    });
    
    // Remove attempts outside the window
    entry.timestamps.retain(|t| now.duration_since(*t) < LOGIN_WINDOW);
    
    if entry.timestamps.len() >= LOGIN_MAX_ATTEMPTS {
        return Err(AppError::BadRequest("Too many login attempts. Please try again later.".into()));
    }
    
    entry.timestamps.push(now);
    entry.last_access = now;
    Ok(())
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/me", get(me))
        .route("/auth/bootstrap", get(bootstrap_status))
}

// Optional auth — extracts AuthUser if Bearer present, else None.
// Needed so /auth/register can work both for bootstrap (no header) and
// authenticated masters (header present).
pub struct MaybeAuthUser(pub Option<Uuid>);

impl<S> FromRequestParts<S> for MaybeAuthUser
where
    AppState: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match AuthUser::from_request_parts(parts, state).await {
            Ok(AuthUser(id)) => Ok(Self(Some(id))),
            Err(_) => Ok(Self(None)),
        }
    }
}

/// Validates password strength: min 8 chars, requires 3 of 4: uppercase, lowercase, digit, special
pub fn validate_password_strength(password: &str) -> Result<(), validator::ValidationError> {
    if password.len() < 8 {
        return Err(validator::ValidationError::new("password_too_short"));
    }
    if password.len() > 128 {
        return Err(validator::ValidationError::new("password_too_long"));
    }
    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_alphanumeric());
    
    if !has_uppercase || !has_lowercase || !has_digit {
        return Err(validator::ValidationError::new("password_weak"));
    }
    // Require at least 3 of 4 character types for stronger passwords
    let types_count = [has_uppercase, has_lowercase, has_digit, has_special]
        .iter()
        .filter(|&&x| x)
        .count();
    if types_count < 3 {
        return Err(validator::ValidationError::new("password_weak"));
    }
    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct RegisterReq {
    #[validate(email)]
    pub email: String,
    #[validate(custom(function = "validate_password_strength"))]
    pub password: String,
    #[validate(length(min = 1, max = 64))]
    pub display_name: String,
    #[serde(default = "default_lang")]
    pub language: String,
}
fn default_lang() -> String { "en".into() }

#[derive(Debug, Deserialize, Validate)]
pub struct LoginReq {
    #[validate(email)]
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub display_name: String,
    pub role: String,
    pub language: String,
    pub avatar_url: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Serialize)]
pub struct AuthRes {
    pub token: String,
    pub user: UserDto,
}

async fn register(
    State(state): State<AppState>,
    MaybeAuthUser(_caller): MaybeAuthUser,
    Json(body): Json<RegisterReq>,
) -> AppResult<(StatusCode, Json<AuthRes>)> {
    body.validate()?;
    let lang = body.language.as_str();
    if lang != "en" && lang != "it" {
        return Err(AppError::BadRequest("unsupported language".into()));
    }

    // Self-registration is open. Every new account starts as a plain user;
    // admin status is only granted via the seeded default admin or manual DB edit.
    let hash = hash_password(&body.password)?;
    let user: UserDto = sqlx::query_as::<_, UserDto>(
        r#"insert into users (email, password_hash, display_name, language, role)
           values ($1, $2, $3, $4::language_code, 'user'::user_role)
           returning id, email, display_name, role::text as role, language::text as language, avatar_url, created_at"#,
    )
    .bind(&body.email)
    .bind(&hash)
    .bind(&body.display_name)
    .bind(lang)
    .fetch_one(&state.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            AppError::Conflict("email exists".into())
        }
        other => other.into(),
    })?;

    let token = issue_jwt(user.id, &state.cfg.jwt_secret)?;
    Ok((StatusCode::CREATED, Json(AuthRes { token, user })))
}

#[derive(Debug, Serialize)]
pub struct BootstrapStatus {
    pub needs_bootstrap: bool,
}

async fn bootstrap_status(State(state): State<AppState>) -> AppResult<Json<BootstrapStatus>> {
    let n: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&state.db)
        .await?;
    Ok(Json(BootstrapStatus { needs_bootstrap: n == 0 }))
}

async fn login(
    State(state): State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    Json(body): Json<LoginReq>,
) -> AppResult<Json<AuthRes>> {
    body.validate()?;
    check_login_rate_limit(addr).await?;
    let row: Option<(Uuid, String)> =
        sqlx::query_as("select id, password_hash from users where email = $1")
            .bind(&body.email)
            .fetch_optional(&state.db)
            .await?;
    let (id, hash) = row.ok_or(AppError::Unauthorized)?;
    if !verify_password(&body.password, &hash)? {
        return Err(AppError::Unauthorized);
    }
    let user: UserDto = sqlx::query_as::<_, UserDto>(
        r#"select id, email, display_name, role::text as role, language::text as language, avatar_url, created_at
           from users where id = $1"#,
    )
    .bind(id)
    .fetch_one(&state.db)
    .await?;
    let token = issue_jwt(id, &state.cfg.jwt_secret)?;
    Ok(Json(AuthRes { token, user }))
}

async fn me(State(state): State<AppState>, AuthUser(uid): AuthUser) -> AppResult<Json<UserDto>> {
    let user: UserDto = sqlx::query_as::<_, UserDto>(
        r#"select id, email, display_name, role::text as role, language::text as language, avatar_url, created_at
           from users where id = $1"#,
    )
    .bind(uid)
    .fetch_one(&state.db)
    .await?;
    Ok(Json(user))
}
