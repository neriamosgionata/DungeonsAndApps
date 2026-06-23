// use_action — toggle action/bonus/reaction/legendary slots.
use super::*;
use super::super::actions::sync::refresh_combatant;
use super::super::actions::economy::auth::require_action_auth;
use super::types::UseAction;
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn use_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseAction>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let updated: Option<Uuid> = match body.action.as_str() {
        "action" => sqlx::query_scalar(
            "update combatants set action_used = true
             where id = $1 and action_used = false returning id",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?,
        "bonus_action" => sqlx::query_scalar(
            "update combatants set bonus_action_used = true
             where id = $1 and bonus_action_used = false returning id",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?,
        "reaction" => sqlx::query_scalar(
            "update combatants set reaction_used = true
             where id = $1 and reaction_used = false returning id",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?,
        "legendary_action" => sqlx::query_scalar(
            "update combatants set legendary_actions_used = legendary_actions_used + 1
             where id = $1
               and legendary_actions_used < legendary_actions_max
             returning id",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?,
        "legendary_resistance" => sqlx::query_scalar(
            "update combatants set legendary_resistances_used = legendary_resistances_used + 1
             where id = $1
               and legendary_resistances_used < legendary_resistances_max
             returning id",
        )
        .bind(id)
        .fetch_optional(&s.db)
        .await?,
        _ => {
            return Err(AppError::BadRequest(format!(
                "unknown action: {}",
                body.action
            )));
        }
    };
    if updated.is_none() {
        return Err(AppError::BadRequest(format!(
            "{} already used or unavailable",
            body.action
        )));
    }
    // C-F2: broadcast action economy toggle so other clients see updated flags
    // without waiting for the next unrelated event. Pre-fix use_action committed
    // the UPDATE in autocommit with no WS publish — stale "used" state across tabs.
    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_updates",
            "id": id,
        })
        .to_string(),
    );
    let c = refresh_combatant(&s.db, id).await?;
    Ok(Json(c))
}
