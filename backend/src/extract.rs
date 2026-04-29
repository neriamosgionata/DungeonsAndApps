use crate::{AppState, auth::decode_jwt, error::AppError};
use axum::{
    extract::{FromRef, FromRequestParts},
    http::{header::AUTHORIZATION, request::Parts},
};
use uuid::Uuid;

pub struct AuthUser(pub Uuid);

impl<S> FromRequestParts<S> for AuthUser
where
    AppState: axum::extract::FromRef<S>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let app_state = AppState::from_ref(state);
        let hv = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or(AppError::Unauthorized)?;
        let token = hv.strip_prefix("Bearer ").ok_or(AppError::Unauthorized)?;
        let claims = decode_jwt(token, &app_state.cfg.jwt_secret)
            .map_err(|_| AppError::Unauthorized)?;
        Ok(AuthUser(claims.sub))
    }
}
