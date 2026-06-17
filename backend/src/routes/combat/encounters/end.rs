// end encounter — set status='ended', emit notification.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use crate::routes::notifications::emit_campaign;
use super::read::fetch;
use super::types::Encounter;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn end_encounter(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'ended' where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    ws::publish(
        e.campaign_id,
        json!({"type":"encounter_ends","id":id}).to_string(),
    );
    emit_campaign(
        &s.db,
        e.campaign_id,
        None,
        "combat.ended",
        &format!("Combat ended: {}", e.name),
        None,
        Some("encounter"),
        Some(id),
    )
    .await;
    Ok(Json(e))
}
