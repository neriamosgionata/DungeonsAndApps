// help_action — grant ally advantage on next attack.
use super::*;
use super::auth::consume_action_or_bonus;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SpecialActionBody {
    pub _target_id: Option<Uuid>,
}

pub async fn help_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SpecialActionBody>,
) -> AppResult<Json<Combatant>> {
    let target_id = body
        ._target_id
        .ok_or(AppError::BadRequest("target_id required".into()))?;
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let mut tx = s.db.begin().await?;
    consume_action_or_bonus(&mut tx, id, false).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Helped', 'buff', 'hand', 'rounds', 1, 1, 'target_turn_start',
                   false, true, '{"attack_advantage_against": true}', 'ability')"#,
    )
    .bind(target_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish(
        campaign_id,
        json!({"type":"combatant_helps","helper_id":id,"target_id":target_id}).to_string(),
    );
    Ok(Json(c))
}
