// death_save — death saving throw handler.
use super::*;
use super::super::economy::require_action_auth;
use super::super::sync_combatant_hp_to_sheet;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DeathSaveBody {
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

pub async fn death_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DeathSaveBody>,
) -> AppResult<Json<combat_engine::DeathSaveResult>> {
    // MED-5: auth + status + round in one query (was 3).
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let round = auth.round;

    let snap = combat_engine::load_snapshot(&s.db, id).await?;

    if snap.hp_current > 0 {
        return Err(AppError::BadRequest("character is not dying".into()));
    }

    let req = combat_engine::DeathSaveReq {
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result =
        combat_engine::resolve_death_save(&snap, &req).map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    if let Some(chid) = snap.character_id {
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'death_saves', jsonb_build_object('successes', $2::int, 'failures', $3::int),
                      'alive', $4::bool
                    )
               where id = $1"#,
        )
        .bind(chid)
        .bind(result.successes_after)
        .bind(result.failures_after)
        .bind(result.alive)
        .execute(&mut *tx)
        .await?;
    }

    let action_str = if result.nat20 {
        "Death Save: NAT 20 — regains 1 HP".to_string()
    } else if result.nat1 {
        format!("Death Save: NAT 1 — {} failures", result.failures_after)
    } else if result.passed {
        format!("Death Save: success ({}/3)", result.successes_after)
    } else {
        format!("Death Save: failure ({}/3)", result.failures_after)
    };

    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(id)
        .bind(&action_str)
        .bind(if result.hp_after > 0 { result.hp_after } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, snap.temp_hp).await {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_death_saves",
            "combatant_id": id,
            "natural_roll": result.natural_roll,
            "passed": result.passed,
            "successes": result.successes_after,
            "failures": result.failures_after,
            "stabilized": result.stabilized,
            "died": result.died,
            // MED-12: drop hp_after (visibility leak). Frontend re-fetches.
            "alive": result.alive,
        }),
    )
    .await;

    Ok(Json(result))
}
