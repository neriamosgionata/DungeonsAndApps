use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("not found")]
    NotFound,
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden")]
    Forbidden,
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("validation failed")]
    Validation(#[from] validator::ValidationErrors),
    #[error("db error")]
    Db(#[from] sqlx::Error),
    #[error("json error")]
    Json(#[from] serde_json::Error),
    #[error("jwt error")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("password hash error")]
    Hash(String),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl AppError {
    fn status_and_key(&self) -> (StatusCode, &'static str) {
        match self {
            Self::NotFound => (StatusCode::NOT_FOUND, "errors.not_found"),
            Self::Unauthorized => (StatusCode::UNAUTHORIZED, "errors.unauthorized"),
            Self::Forbidden => (StatusCode::FORBIDDEN, "errors.forbidden"),
            Self::Conflict(_) => (StatusCode::CONFLICT, "errors.conflict"),
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "errors.bad_request"),
            Self::Validation(_) => (StatusCode::UNPROCESSABLE_ENTITY, "errors.validation"),
            Self::Db(sqlx::Error::RowNotFound) => (StatusCode::NOT_FOUND, "errors.not_found"),
            Self::Db(_) | Self::Jwt(_) | Self::Hash(_) | Self::Json(_) | Self::Other(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "errors.internal")
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, key) = self.status_and_key();
        if status.is_server_error() {
            // dump the full source chain so the underlying sqlx/jwt/etc error is visible in logs
            let mut chain = String::new();
            let mut cur: Option<&dyn std::error::Error> = Some(&self);
            let mut depth = 0;
            while let Some(e) = cur {
                if depth > 0 {
                    chain.push_str(" | caused by: ");
                }
                chain.push_str(&e.to_string());
                cur = e.source();
                depth += 1;
            }
            tracing::error!(error = %chain, "request failed");
        }
        let body = Json(json!({
            "error": { "key": key, "message": self.to_string() }
        }));
        (status, body).into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
