// delete encounter.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

pub async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: (Uuid, String) = sqlx::query_as("select campaign_id, status::text as status from encounters where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, status) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    if status == "active" {
        return Err(AppError::Conflict("end encounter before deleting".into()));
    }
    sqlx::query("delete from encounters where id = $1")
        .bind(id).execute(&s.db).await?;
    ws::publish(
        campaign_id,
        json!({"type":"encounter_deletes","id":id}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}
