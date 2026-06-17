use validator::Validate;
// encounter CRUD: list, create, read, update, delete.
use crate::rbac::Role;
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::types::{Encounter, EncounterCreate, EncounterUpdate};
use crate::routes::notifications::emit_campaign;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<Json<Vec<Encounter>>> {
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let rows: Vec<Encounter> = sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where campaign_id = $1 order by created_at desc"
    )
    .bind(campaign_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

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
