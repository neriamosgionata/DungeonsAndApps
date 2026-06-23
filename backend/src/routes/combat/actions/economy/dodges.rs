// dodge, disengage handlers.
use super::*;
use super::auth::consume_action_or_bonus;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub async fn dodge(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let mut tx = s.db.begin().await?;
    consume_action_or_bonus(&mut tx, id, false).await?;

    sqlx::query(
        "update combatant_effects set active = false where combatant_id = $1 and name = 'Dodge'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dodge', 'buff', 'shield', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"attack_disadvantage_against": true, "dex_save_advantage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx).await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({"type":"combatant_dodges","id":id}),
    )
    .await;
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct ShoveBody {
    pub use_bonus_action: bool,
}

#[derive(Debug, Serialize)]
pub struct ShoveResult {
    pub combatant_id: Uuid,
    pub disengaged: bool,
}

pub async fn disengage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ShoveBody>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let mut tx = s.db.begin().await?;
    consume_action_or_bonus(&mut tx, id, body.use_bonus_action).await?;

    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Disengage'")
        .bind(id).execute(&mut *tx).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Disengage', 'buff', 'wind', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"disengage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({"type":"combatant_disengages","id":id}),
    )
    .await;
    Ok(Json(c))
}
