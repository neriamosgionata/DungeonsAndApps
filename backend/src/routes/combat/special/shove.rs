// shove, stand_up handlers — extracted from special.rs.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ShoveResult {
    pub success: bool,
    pub attacker_total: i32,
    pub defender_total: i32,
    pub knocked_prone: bool,
    pub pushed_away: bool,
}

#[derive(Debug, Deserialize)]
pub struct ShoveBody {
    pub target_id: Uuid,
    pub knock_prone: bool,
}

pub async fn shove(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ShoveBody>,
) -> AppResult<Json<ShoveResult>> {
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
    let mut knocked_prone = false;
    let mut pushed_away = false;

    let mut tx = s.db.begin().await?;
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        tx.rollback().await?;
        return Err(AppError::BadRequest("action already used".into()));
    }

    if success {
        if body.knock_prone {
            let mut conditions = defender_snap.conditions.clone();
            if !super::super::has_condition(&conditions, "prone") {
                conditions.push("prone".to_string());
            }
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&conditions)
                .bind(body.target_id)
                .execute(&mut *tx)
                .await?;
            knocked_prone = true;
        } else {
            if let (Some(tk_x), Some(tk_y)) = (defender_snap.token_x, defender_snap.token_y) {
                let dx = tk_x - attacker_snap.token_x.unwrap_or(tk_x);
                let dy = tk_y - attacker_snap.token_y.unwrap_or(tk_y);
                let len = (dx * dx + dy * dy).sqrt().max(0.01);
                let push_pct = 5.0;
                let new_x = (tk_x + (dx / len) * push_pct).clamp(0.0, 100.0);
                let new_y = (tk_y + (dy / len) * push_pct).clamp(0.0, 100.0);
                sqlx::query("update combatants set token_x = $1, token_y = $2 where id = $3")
                    .bind(new_x)
                    .bind(new_y)
                    .bind(body.target_id)
                    .execute(&mut *tx)
                    .await?;
            }
            pushed_away = true;
        }
    }

    tx.commit().await?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_shoves",
            "attacker_id": id,
            "target_id": body.target_id,
            "success": success,
            "knocked_prone": knocked_prone,
            "pushed_away": pushed_away,
        })
        .to_string(),
    );

    Ok(Json(ShoveResult {
        success,
        attacker_total: att_roll.total,
        defender_total: def_roll.total,
        knocked_prone,
        pushed_away,
    }))
}

pub async fn stand_up(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, Vec<String>, i32) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, c.conditions, c.movement_used_ft
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner, conditions, movement_used) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    if !super::super::has_condition(&conditions, "prone") {
        return Err(AppError::BadRequest("not prone".into()));
    }

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);
    let has_athlete = snap
        .sheet_raw
        .get("feats")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .any(|f| f.get("key").and_then(|k| k.as_str()) == Some("athlete"))
        })
        .unwrap_or(false);
    let stand_cost = if has_athlete { 5 } else { speed / 2 };

    let dash_bonus: i32 = snap
        .active_effects
        .iter()
        .filter_map(|e| {
            e.modifiers
                .as_object()
                .and_then(|m| m.get("movement"))
                .and_then(|v| v.as_object())
                .filter(|mov| mov.get("type").and_then(|t| t.as_str()) == Some("dash_bonus"))
                .and_then(|mov| mov.get("distance_ft").and_then(|d| d.as_i64()))
                .map(|d| d as i32)
        })
        .sum();
    let effective_speed = speed + dash_bonus;

    if stats.incapacitated {
        return Err(AppError::BadRequest(
            "cannot stand up while incapacitated".into(),
        ));
    }
    if movement_used + stand_cost > effective_speed && effective_speed > 0 {
        return Err(AppError::BadRequest(format!(
            "not enough movement to stand up (used {}ft + {}ft > {}ft)",
            movement_used, stand_cost, speed
        )));
    }

    let new_conditions: Vec<String> = super::super::remove_condition(conditions, "prone");

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             conditions = $1,
             movement_used_ft = movement_used_ft + $2
           where id = $3
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
    )
    .bind(&new_conditions)
    .bind(stand_cost)
    .bind(id)
    .fetch_one(&s.db)
    .await?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_stands_up",
            "combatant_id": id,
            "movement_cost": stand_cost,
        })
        .to_string(),
    );

    Ok(Json(c))
}



