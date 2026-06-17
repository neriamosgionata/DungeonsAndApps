// delete_combatant — remove from encounter (master only).
use super::*;
use crate::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

pub async fn delete_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: (Uuid, Uuid, String) = sqlx::query_as(
        "select c.id, e.campaign_id, e.id::text as encounter_id
         from combatants c join encounters e on e.id = c.encounter_id
         where c.id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let (campaign_id, _encounter_id, encounter_id_str) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    sqlx::query("delete from combatants where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;

    ws::publish(
        campaign_id,
        json!({"type":"combatant_leaves","id":id,"encounter_id":encounter_id_str}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}
