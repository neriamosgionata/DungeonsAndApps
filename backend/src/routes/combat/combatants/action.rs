// use_action — toggle action/bonus/reaction/legendary slots.
use super::*;
use super::super::actions::sync::refresh_combatant;
use super::types::UseAction;
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

pub async fn use_action(
    State(s): State<AppState>,
    AuthUser(_uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseAction>,
) -> AppResult<Json<Combatant>> {
    let col = match body.action.as_str() {
        "action" => "action_used",
        "bonus_action" => "bonus_action_used",
        "reaction" => "reaction_used",
        "legendary_action" => "legendary_actions_used",
        "legendary_resistance" => "legendary_resistances_used",
        _ => return Err(AppError::BadRequest(format!("unknown action: {}", body.action))),
    };
    let q = if body.action == "legendary_action" {
        format!(
            "update combatants set {col} = least(legendary_actions_max, legendary_actions_used + 1)
             where id = $1 and legendary_actions_used < legendary_actions_max returning id"
        )
    } else if body.action == "legendary_resistance" {
        format!(
            "update combatants set {col} = least(legendary_resistances_max, legendary_resistances_used + 1)
             where id = $1 and legendary_resistances_used < legendary_resistances_max returning id"
        )
    } else {
        format!("update combatants set {col} = true where id = $1 and {col} = false returning id")
    };
    let updated: Option<Uuid> = sqlx::query_scalar(&q).bind(id).fetch_optional(&s.db).await?;
    if updated.is_none() {
        return Err(AppError::BadRequest(format!("{} already used or unavailable", body.action)));
    }
    let c = refresh_combatant(&s.db, id).await?;
    Ok(Json(c))
}
