// Grapple, grapple_escape, stand_up, and shove handlers.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct GrappleResult {
    pub success: bool,
    pub attacker_roll: i32,
    pub attacker_total: i32,
    pub defender_roll: i32,
    pub defender_total: i32,
    pub grapple_applied: bool,
}

#[derive(Debug, Deserialize)]
pub struct GrappleBody {
    pub target_id: Uuid,
}

pub async fn grapple(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GrappleBody>,
) -> AppResult<Json<GrappleResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let defender_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != defender_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let defender_stats = combat_engine::compute_stats(&defender_snap);

    let att_ath = attacker_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "athletics")
        .map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&attacker_snap, "str"));
    let def_ath = defender_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "athletics")
        .map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "str"));
    let def_acr = defender_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "acrobatics")
        .map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "dex"));
    let def_best = def_ath.max(def_acr);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = if attacker_stats.frightened || attacker_stats.charmed {
        format!("2d20kl1+{}", att_ath)
    } else {
        format!("1d20+{}", att_ath)
    };
    let def_expr = format!("1d20+{}", def_best);

    let att_roll =
        crate::dice::roll(&att_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let def_roll =
        crate::dice::roll(&def_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = att_roll.total >= def_roll.total;
    let mut grapple_applied = false;

    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if success {
        let mut def_conditions: Vec<String> = defender_snap.conditions.clone();
        if !super::super::has_condition(&def_conditions, "grappled") {
            def_conditions.push("grappled".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&def_conditions)
            .bind(body.target_id)
            .execute(&mut *tx)
            .await?;

        let mut att_conditions: Vec<String> = attacker_snap.conditions.clone();
        if !super::super::has_condition(&att_conditions, "grappling") {
            att_conditions.push("grappling".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&att_conditions)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        grapple_applied = true;
    }

    tx.commit().await?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_grapples",
            "attacker_id": id,
            "target_id": body.target_id,
            "success": success,
        })
        .to_string(),
    );

    Ok(Json(GrappleResult {
        success,
        attacker_roll: att_roll
            .terms
            .first()
            .and_then(|t| t.rolls.first().copied())
            .unwrap_or(0),
        attacker_total: att_roll.total,
        defender_roll: def_roll
            .terms
            .first()
            .and_then(|t| t.rolls.first().copied())
            .unwrap_or(0),
        defender_total: def_roll.total,
        grapple_applied,
    }))
}



