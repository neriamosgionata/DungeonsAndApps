// move_combatant — token movement with line-of-sight + wall + bounds checks.
use super::*;
use super::super::actions::sync::refresh_combatant;
use super::types::CombatantMove;
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn move_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CombatantMove>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, i32) = sqlx::query_as(
        "select e.campaign_id, e.id, e.status::text, ch.owner_id, e.round
         from combatants c join encounters e on e.id = c.encounter_id
         left join characters ch on ch.id = c.character_id
         where c.id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    let (campaign_id, encounter_id, status, owner, round) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let x = body.x.clamp(0.0, 100.0);
    let y = body.y.clamp(0.0, 100.0);

    let cost = body.movement_cost.unwrap_or(5.0);

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);

    let dash_bonus: i32 = snap
        .active_effects
        .iter()
        .filter_map(|e| {
            e.modifiers
                .as_object()
                .and_then(|m| m.get("movement"))
                .and_then(|v| v.as_object())
                .filter(|mov| mov.get("type").and_then(|t| t.as_str()) == Some("dash_bonus"))
                .and_then(|mov| mov.get("distance_ft").and_then(|d| d.as_i64()))
                .map(|d| d as i32)
        })
        .sum();
    let effective_speed = (speed + dash_bonus) as f32;

    if let (Some(tx), Some(ty)) = (snap.token_x, snap.token_y) {
        let dx = (x - tx).abs();
        let dy = (y - ty).abs();
        let grid_w = (dx / (100.0 / 5.0)).max(dy / (100.0 / 5.0));
        if grid_w > 0.0 {
            let walls: Vec<(f32, f32, f32, f32)> = sqlx::query_as(
                r#"select origin_x, origin_y,
                   coalesce(end_x, origin_x) as end_x,
                   coalesce(end_y, origin_y + 5) as end_y
                   from encounter_overlays
                   where encounter_id = $1 and active = true and zone_type = 'wall' and shape = 'line'"#,
            )
            .bind(encounter_id)
            .fetch_all(&s.db)
            .await?;
            for (wx1, wy1, wx2, wy2) in &walls {
                if super::super::super::combat::tactical::positioning::segments_intersect(tx, ty, x, y, *wx1, *wy1, *wx2, *wy2) {
                    return Err(AppError::BadRequest(
                        "movement blocked by wall obstacle".into(),
                    ));
                }
            }
        }
    }

    // C-1: movement budget is charged INSIDE the tx under the row lock.
    // The old code read movement_used_ft before the tx (two concurrent moves
    // both charged against 0 → double movement for one charge) and dropped
    // the cost on every move after the first per round (token_moved_round
    // gate) → free teleports. Both caps now apply to EVERY move, computed
    // from the locked row.
    let mut tx_db = s.db.begin().await?;
    let movement_used: i32 = sqlx::query_scalar(
        "select movement_used_ft from combatants where id = $1 for update",
    )
    .bind(id)
    .fetch_optional(&mut *tx_db)
    .await?
    .ok_or(AppError::NotFound)?;
    if cost > effective_speed {
        return Err(AppError::BadRequest(format!(
            "movement cost {} exceeds speed {}",
            cost as i32, speed
        )));
    }
    let new_movement_used = movement_used + cost as i32;
    if new_movement_used > effective_speed as i32 {
        return Err(AppError::BadRequest(format!(
            "movement used {} + cost {} exceeds speed {}",
            movement_used, cost as i32, effective_speed as i32
        )));
    }
    let updated: Option<Uuid> = sqlx::query_scalar(
        "update combatants set
             token_x = $1,
             token_y = $2,
             token_on_map = true,
             movement_used_ft = $3,
             token_moved_round = $4
           where id = $5
           returning id",
    )
    .bind(x)
    .bind(y)
    .bind(new_movement_used)
    .bind(round)
    .bind(id)
    .fetch_optional(&mut *tx_db)
    .await?;
    if updated.is_none() {
        return Err(AppError::NotFound);
    }
    // A17: moving a MOUNT moves its rider with it (PHB p.198 — the rider
    // shares the mount's position); moving a RIDER moves the mount too
    // (the rider can't move independently while mounted).
    let counterpart: Option<Uuid> = sqlx::query_scalar(
        r#"select coalesce(
             (select id from combatants where mounted_on = $1),
             mounted_on
           ) from combatants where id = $1"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx_db)
    .await?
    .flatten();
    if let Some(cid) = counterpart {
        sqlx::query(
            "update combatants set token_x = $1, token_y = $2, token_on_map = true where id = $3",
        )
        .bind(x)
        .bind(y)
        .bind(cid)
        .execute(&mut *tx_db)
        .await?;
    }
    tx_db.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type":"combatant_moves",
            "id":id,
            "x":x,
            "y":y,
            "token_moved_round":c.token_moved_round,
            "movement_used_ft":c.movement_used_ft
        }),
    )
    .await;
    Ok(Json(c))
}
