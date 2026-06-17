// dash, hide handlers.
use super::*;
use super::auth::consume_action_or_bonus;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ActionBody {
    #[serde(default)]
    pub use_bonus_action: bool,
}

pub async fn dash(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let mut tx = s.db.begin().await?;

    consume_action_or_bonus(&mut tx, id, body.use_bonus_action).await?;

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let extra = stats.speed.max(0);

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dash', 'buff', 'bolt', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, $2, 'ability')"#,
    )
    .bind(id)
    .bind(json!({"movement": {"type": "dash_bonus", "distance_ft": extra}}))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish(
        campaign_id,
        json!({"type":"combatant_dashes","id":id,"extra_movement":extra}).to_string(),
    );
    Ok(Json(c))
}

pub async fn hide(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let mut tx = s.db.begin().await?;

    consume_action_or_bonus(&mut tx, id, body.use_bonus_action).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Hidden', 'buff', 'eye-slash', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"hidden": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish(
        campaign_id,
        json!({"type":"combatant_hides","id":id}).to_string(),
    );
    Ok(Json(c))
}
