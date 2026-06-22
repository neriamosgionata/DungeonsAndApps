use validator::Validate;
// create encounter.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::AppResult;
use crate::extract::AuthUser;
use super::types::{Encounter, EncounterCreate};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<EncounterCreate>,
) -> AppResult<Json<Encounter>> {
    body.validate()?;
    let role = rbac::require_master(&s.db, uid, campaign_id).await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "insert into encounters (campaign_id, name, notes) values ($1, $2, $3)
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(campaign_id)
    .bind(&body.name)
    .bind(&body.notes)
    .fetch_one(&s.db)
    .await?;
    ws::publish(
        campaign_id,
        json!({"type":"encounter_creates","id":e.id,"name":e.name}).to_string(),
    );
    let _ = role;
    Ok(Json(e))
}
