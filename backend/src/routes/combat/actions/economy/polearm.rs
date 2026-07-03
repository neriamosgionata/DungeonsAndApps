// polearm_bonus_attack — Polearm Master feat (PHB p.168).
//
// Bonus-action melee attack with the butt-end of a glaive, halberd, or
// quarterstaff. d4 bludgeoning damage, no weapon property constraints
// beyond wielding one of the named polearms.
use super::super::sync_combatant_hp_to_sheet;
use super::*;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use crate::rbac::Role;
use crate::ws;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct PolearmAttackBody {
    pub target_id: Uuid,
}

pub async fn polearm_bonus_attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<PolearmAttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest(
            "attacker and target not in same encounter".into(),
        ));
    }

    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    if attacker_snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot act while at 0 HP".into()));
    }
    let incapacitated = attacker_snap.conditions.iter().any(|c| {
        let cl = c.to_lowercase();
        cl.starts_with("incapacitated")
            || cl.starts_with("paralyzed")
            || cl.starts_with("petrified")
            || cl.starts_with("stunned")
            || cl.starts_with("unconscious")
    });
    if incapacitated {
        return Err(AppError::BadRequest(
            "cannot act while incapacitated".into(),
        ));
    }

    // Feat + weapon validation. Polearm Master is the only feat that
    // gates this attack; feats[].key is the source of truth (matches
    // the `combat_tag` set in web/src/lib/feats.ts).
    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    if !attacker_stats.polearm_master {
        return Err(AppError::BadRequest(
            "Polearm Master feat required for polearm bonus attack".into(),
        ));
    }
    if !combat_engine::is_wielding_polearm(&attacker_snap) {
        return Err(AppError::BadRequest(
            "must be wielding a glaive, halberd, or quarterstaff".into(),
        ));
    }

    // Non-master bypass: skip the bonus-action cost (master can use it freely).
    let mut tx = s.db.begin().await?;
    if auth.role != Role::Master {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    }

    let target_stats = combat_engine::compute_stats(&target_snap);
    let result = combat_engine::resolve_polearm_ba_attack(
        &attacker_snap,
        &target_snap,
        &attacker_stats,
        &target_stats,
    )
    .map_err(AppError::BadRequest)?;

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx)
            .await?;
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(
            &s.db,
            body.target_id,
            result.target_hp_after,
            result.target_temp_hp_after,
        )
        .await
        {
            tracing::error!(combatant_id = %body.target_id, "sync sheet HP: {e}");
        }
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_polearm_bonus",
            "attacker_id": id,
            "target_id": body.target_id,
            "hit": result.hit,
            "damage": result.damage_applied,
            "label": "Polearm Master BA",
        })
        .to_string(),
    );

    Ok(Json(result))
}
