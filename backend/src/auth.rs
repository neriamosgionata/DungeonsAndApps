use crate::error::{AppError, AppResult};
use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString, rand_core::OsRng},
};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub exp: i64,
    pub iat: i64,
}

pub fn hash_password(password: &str) -> AppResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| AppError::Hash(e.to_string()))
}

pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let parsed = PasswordHash::new(hash).map_err(|e| AppError::Hash(e.to_string()))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

pub fn issue_jwt(user_id: Uuid, secret: &str) -> AppResult<String> {
    let now = OffsetDateTime::now_utc();
    let claims = Claims {
        sub: user_id,
        iat: now.unix_timestamp(),
        exp: (now + Duration::hours(24)).unix_timestamp(),
    };
    Ok(encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

pub fn decode_jwt(token: &str, secret: &str) -> AppResult<Claims> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.validate_nbf = false;
    // Explicitly require that the token has not expired
    // Clock skew tolerance is 60 seconds by default which is reasonable
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    Ok(data.claims)
}
