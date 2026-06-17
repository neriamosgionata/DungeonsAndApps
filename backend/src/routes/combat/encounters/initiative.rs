// set_initiative — set a character's initiative value.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::read::fetch;
use super::types::SetInitiativeBody;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn set_initiative(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<SetInitiativeBody>,
) -> AppResult<Json<()>> {
    let e = fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let updated: Option<Uuid> = sqlx::query_scalar(
        "update combatants set initiative = $1, initiative_rolled = true, turn_order = coalesce(turn_order, 0) where id = $2 and encounter_id = $3 returning id"
    )
    .bind(body.initiative)
    .bind(body.character_id)
    .bind(encounter_id)
    .fetch_optional(&s.db)
    .await?;
    if updated.is_none() {
        return Err(AppError::NotFound);
    }
    ws::publish(
        e.campaign_id,
        json!({"type":"combatant_updates","id":body.character_id,"initiative":body.initiative,"initiative_rolled":true}).to_string(),
    );
    Ok(Json(()))
}
