// Legendary action and lair action handlers.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;
use serde_json::json;
use uuid::Uuid;

pub async fn lair_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = super::super::fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let e: Option<Encounter> = sqlx::query_as::<_, Encounter>(
        "update encounters set lair_action_used = true where id = $1 and lair_action_used = false
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).fetch_optional(&s.db).await?;
    let e = e.ok_or_else(|| AppError::BadRequest("lair action already used this round".into()))?;
    ws::publish(
        e.campaign_id,
        json!({
            "type": "lair_action",
            "encounter_id": id,
            "round": e.round,
        })
        .to_string(),
    );
    Ok(Json(e))
}

#[derive(Debug, Serialize)]
pub struct LegendaryActionResult {
    pub legendary_actions_used: i32,
    pub legendary_actions_max: i32,
}

pub async fn legendary_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LegendaryActionResult>> {
    let row: Option<(Uuid, String)> = sqlx::query_as(
        "select e.campaign_id, e.status::text as status
         from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?;
    let (campaign_id, encounter_status) = row.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    if snap.hp_current <= 0 {
        return Err(AppError::BadRequest(
            "cannot use legendary actions while at 0 HP".into(),
        ));
    }
    let incapacitated = snap.conditions.iter().any(|c| {
        let cl = c.to_lowercase();
        cl.starts_with("incapacitated")
            || cl.starts_with("paralyzed")
            || cl.starts_with("petrified")
            || cl.starts_with("stunned")
            || cl.starts_with("unconscious")
    });
    if incapacitated {
        return Err(AppError::BadRequest(
            "cannot use legendary actions while incapacitated".into(),
        ));
    }

    let updated: Option<(i32, i32)> = sqlx::query_as(
        "update combatants set legendary_actions_used = least(legendary_actions_max, legendary_actions_used + 1)
         where id = $1 and legendary_actions_used < legendary_actions_max
         returning legendary_actions_used, legendary_actions_max")
        .bind(id).fetch_optional(&s.db).await?;
    let (used, max) =
        updated.ok_or_else(|| AppError::BadRequest("no legendary actions remaining".into()))?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_uses_legendary_action",
            "combatant_id": id,
            "legendary_actions_used": used,
            "legendary_actions_max": max,
        })
        .to_string(),
    );

    Ok(Json(LegendaryActionResult {
        legendary_actions_used: used,
        legendary_actions_max: max,
    }))
}
