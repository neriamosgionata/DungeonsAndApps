use validator::Validate;
// update encounter.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::types::{Encounter, EncounterUpdate};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<EncounterUpdate>,
) -> AppResult<Json<Encounter>> {
    body.validate()?;
    let row: (Uuid, Uuid) = sqlx::query_as("select campaign_id, id from encounters where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, _eid) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    let clear_map_image = body.clear_map_image.unwrap_or(false);
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        r#"update encounters set
             name         = coalesce($2, name),
             notes        = coalesce($3, notes),
             map_image    = case when $5 then null else coalesce($4, map_image) end,
             map_grid_size = coalesce($6, map_grid_size),
             show_grid    = coalesce($7, show_grid),
             grid_type    = coalesce($8, grid_type)
           where id = $1
           returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"#,
    )
    .bind(id).bind(&body.name).bind(&body.notes).bind(&body.map_image)
    .bind(clear_map_image).bind(body.map_grid_size).bind(body.show_grid)
    .bind(&body.grid_type)
    .fetch_one(&s.db)
    .await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({"type":"encounter_updated","id":id}),
    )
    .await;
    Ok(Json(e))
}
