// list encounters.
use crate::rbac;
use crate::AppState;
use crate::error::AppResult;
use crate::extract::AuthUser;
use super::types::Encounter;
use axum::Json;
use axum::extract::{Path, State};
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
