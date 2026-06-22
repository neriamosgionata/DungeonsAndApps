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
    let row: (Uuid, Uuid) = sqlx::query_as(
        "select e.campaign_id, e.id as encounter_id
         from combatants c join encounters e on e.id = c.encounter_id
         where c.id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let (campaign_id, encounter_id) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let mut tx = s.db.begin().await?;
    sqlx::query("delete from combatants where id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    // HIGH-8: renumber turn_order 0..N-1 by initiative DESC, dex DESC so
    // gaps from the delete don't desync `next_turn`'s `turn_order = new_idx`
    // lookup. Mirrors the ROW_NUMBER pattern in start.rs / initiative.rs.
    sqlx::query(
        r#"update combatants c
           set turn_order = sub.new_order
           from (
             select id, (row_number() over (order by initiative desc, dex_tiebreaker desc) - 1)::int as new_order
             from combatants
             where encounter_id = $1 and initiative_rolled = true
           ) sub
           where c.id = sub.id"#,
    )
    .bind(encounter_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    ws::publish(
        campaign_id,
        json!({"type":"combatant_leaves","id":id,"encounter_id":encounter_id}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}
