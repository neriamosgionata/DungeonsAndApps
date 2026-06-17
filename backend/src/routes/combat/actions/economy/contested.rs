// contested_hide — stealth roll vs passive perception for each observer.
use super::*;
use super::auth::consume_action_or_bonus;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ContestedHideBody {
    pub observer_ids: Option<Vec<Uuid>>,
    pub use_bonus_action: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ContestedHideResult {
    pub hider_id: Uuid,
    pub hider_name: String,
    pub stealth_total: i32,
    pub natural: i32,
    pub observers: Vec<HideObserverResult>,
    pub hidden: bool,
}

#[derive(Debug, Serialize)]
pub struct HideObserverResult {
    pub observer_id: Uuid,
    pub observer_name: String,
    pub passive_perception: i32,
    pub spotted: bool,
}

pub async fn contested_hide(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ContestedHideBody>,
) -> AppResult<Json<ContestedHideResult>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let encounter_id = auth.encounter_id;

    let hider_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let hider_stats = combat_engine::compute_stats(&hider_snap);
    let stealth_mod = hider_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "stealth")
        .map(|(_, m)| *m)
        .unwrap_or(0);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let expr = format!("1d20+{}", stealth_mod);
    let roll = crate::dice::roll(&expr, &mut rng)
        .map_err(|e| AppError::BadRequest(format!("stealth roll: {}", e)))?;
    let natural = roll
        .terms
        .first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);
    let stealth_total = roll.total.max(1);

    let observer_ids: Vec<Uuid> = if let Some(ref ids) = body.observer_ids {
        ids.clone()
    } else {
        sqlx::query_scalar(
            r#"select c.id from combatants c
               where c.encounter_id = $1 and c.id != $2
               and c.hp_current > 0 and c.initiative_rolled = true
               and ((c.ref_type = 'character' and $3 = 'npc') or (c.ref_type = 'npc' and $3 = 'character'))"#,
        )
        .bind(encounter_id).bind(id)
        .bind(if hider_snap.character_id.is_some() { "character" } else { "npc" })
        .fetch_all(&s.db).await?
    };
    if observer_ids.is_empty() {
        return Err(AppError::BadRequest("no observers to hide from".into()));
    }

    let mut observers = Vec::new();
    let mut all_spotted = true;

    for oid in &observer_ids {
        let snap = combat_engine::load_snapshot(&s.db, *oid).await?;
        let stats = combat_engine::compute_stats(&snap);
        let pp = stats
            .passive_scores
            .iter()
            .find(|(s, _)| s == "perception")
            .map(|(_, m)| *m)
            .unwrap_or(10);
        let spotted = pp >= stealth_total;
        if !spotted {
            all_spotted = false;
        }
        observers.push(HideObserverResult {
            observer_id: *oid,
            observer_name: snap.display_name.clone(),
            passive_perception: pp,
            spotted,
        });
    }

    let mut tx = s.db.begin().await?;

    consume_action_or_bonus(&mut tx, id, body.use_bonus_action.unwrap_or(false)).await?;

    if !all_spotted {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type)
               values ($1, 'Hidden', 'buff', 'eye-slash', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"hidden": true}', 'ability')"#,
        )
        .bind(id)
        .execute(&mut *tx).await?;
    }

    tx.commit().await?;

    let hidden = !all_spotted;
    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_contested_hide",
            "hider_id": id,
            "stealth_total": stealth_total,
            "hidden": hidden,
            "observer_count": observers.len(),
        })
        .to_string(),
    );

    Ok(Json(ContestedHideResult {
        hider_id: id,
        hider_name: hider_snap.display_name.clone(),
        stealth_total,
        natural,
        observers,
        hidden,
    }))
}
