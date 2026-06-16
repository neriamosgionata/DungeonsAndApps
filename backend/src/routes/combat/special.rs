use super::*;
use super::Combatant;
use super::Encounter;

use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    ws,
};
use axum::{
    Json,
    extract::{Path, State},
};
use rand::SeedableRng;
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

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
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

    let att_ath = attacker_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&attacker_snap, "str"));
    let def_ath = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "str"));
    let def_acr = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "acrobatics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "dex"));
    let def_best = def_ath.max(def_acr);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = if attacker_stats.frightened || attacker_stats.charmed {
        format!("2d20kl1+{}", att_ath)
    } else {
        format!("1d20+{}", att_ath)
    };
    let def_expr = format!("1d20+{}", def_best);

    let att_roll = crate::dice::roll(&att_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let def_roll = crate::dice::roll(&def_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

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
        if !has_condition(&def_conditions, "grappled") {
            def_conditions.push("grappled".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&def_conditions).bind(body.target_id).execute(&mut *tx).await?;

        let mut att_conditions: Vec<String> = attacker_snap.conditions.clone();
        if !has_condition(&att_conditions, "grappling") {
            att_conditions.push("grappling".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&att_conditions).bind(id).execute(&mut *tx).await?;
        grapple_applied = true;
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_grappled",
        "attacker_id": id,
        "target_id": body.target_id,
        "success": success,
    }).to_string());

    Ok(Json(GrappleResult {
        success,
        attacker_roll: att_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0),
        attacker_total: att_roll.total,
        defender_roll: def_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0),
        defender_total: def_roll.total,
        grapple_applied,
    }))
}

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

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
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

    let att_ath = attacker_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&attacker_snap, "str"));
    let def_ath = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "str"));
    let def_acr = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "acrobatics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "dex"));
    let def_best = def_ath.max(def_acr);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = if attacker_stats.frightened || attacker_stats.charmed {
        format!("2d20kl1+{}", att_ath)
    } else {
        format!("1d20+{}", att_ath)
    };
    let def_expr = format!("1d20+{}", def_best);

    let att_roll = crate::dice::roll(&att_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let def_roll = crate::dice::roll(&def_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = att_roll.total >= def_roll.total;
    let mut knocked_prone = false;
    let mut pushed_away = false;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if success {
        if body.knock_prone {
            let mut conditions = defender_snap.conditions.clone();
            if !has_condition(&conditions, "prone") {
                conditions.push("prone".to_string());
            }
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&conditions).bind(body.target_id).execute(&s.db).await?;
            knocked_prone = true;
        } else {
            if let (Some(tx), Some(ty)) = (defender_snap.token_x, defender_snap.token_y) {
                let dx = tx - attacker_snap.token_x.unwrap_or(tx);
                let dy = ty - attacker_snap.token_y.unwrap_or(ty);
                let len = (dx*dx + dy*dy).sqrt().max(0.01);
                let push_pct = 5.0;
                let new_x = (tx + (dx/len) * push_pct).clamp(0.0, 100.0);
                let new_y = (ty + (dy/len) * push_pct).clamp(0.0, 100.0);
                sqlx::query("update combatants set token_x = $1, token_y = $2 where id = $3")
                    .bind(new_x).bind(new_y).bind(body.target_id).execute(&s.db).await?;
            }
            pushed_away = true;
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_shoved",
        "attacker_id": id,
        "target_id": body.target_id,
        "success": success,
        "knocked_prone": knocked_prone,
        "pushed_away": pushed_away,
    }).to_string());

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
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    if !has_condition(&conditions, "prone") {
        return Err(AppError::BadRequest("not prone".into()));
    }

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);
    let has_athlete = snap.sheet_raw.get("feats")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("athlete")))
        .unwrap_or(false);
    let stand_cost = if has_athlete { 5 } else { speed / 2 };

    let dash_bonus: i32 = snap.active_effects.iter()
        .filter_map(|e| {
            e.modifiers.as_object()
                .and_then(|m| m.get("movement"))
                .and_then(|v| v.as_object())
                .filter(|mov| mov.get("type").and_then(|t| t.as_str()) == Some("dash_bonus"))
                .and_then(|mov| mov.get("distance_ft").and_then(|d| d.as_i64()))
                .map(|d| d as i32)
        })
        .sum();
    let effective_speed = speed + dash_bonus;

    if stats.incapacitated {
        return Err(AppError::BadRequest("cannot stand up while incapacitated".into()));
    }
    if movement_used + stand_cost > effective_speed && effective_speed > 0 {
        return Err(AppError::BadRequest(format!(
            "not enough movement to stand up (used {}ft + {}ft > {}ft)",
            movement_used, stand_cost, speed
        )));
    }

    let new_conditions: Vec<String> = remove_condition(conditions, "prone");

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
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#,
    )
    .bind(&new_conditions)
    .bind(stand_cost)
    .bind(id)
    .fetch_one(&s.db)
    .await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_stood_up",
        "combatant_id": id,
        "movement_cost": stand_cost,
    }).to_string());

    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct GrappleEscapeBody {
    pub grappler_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct GrappleEscapeResult {
    pub success: bool,
    pub escapee_roll: i32,
    pub escapee_total: i32,
    pub grappler_roll: i32,
    pub grappler_total: i32,
    pub escaped: bool,
}

pub async fn grapple_escape(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GrappleEscapeBody>,
) -> AppResult<Json<GrappleEscapeResult>> {
    let escapee_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let grappler_snap = combat_engine::load_snapshot(&s.db, body.grappler_id).await?;

    if escapee_snap.encounter_id != grappler_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(escapee_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let action_used: bool = sqlx::query_scalar("select action_used from combatants where id = $1")
        .bind(id).fetch_one(&s.db).await?;
    if action_used {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if !has_condition(&escapee_snap.conditions, "grappled") {
        return Err(AppError::BadRequest("not grappled".into()));
    }

    let mut rng = rand::rngs::StdRng::from_os_rng();

    let escapee_stats = combat_engine::compute_stats(&escapee_snap);
    let athletics = escapee_stats.skill_mods.iter().find(|(s, _)| s == "athletics").map(|(_, m)| *m).unwrap_or(0);
    let acrobatics = escapee_stats.skill_mods.iter().find(|(s, _)| s == "acrobatics").map(|(_, m)| *m).unwrap_or(0);
    let escapee_mod = athletics.max(acrobatics);

    let grappler_stats = combat_engine::compute_stats(&grappler_snap);
    let grappler_athletics = grappler_stats.skill_mods.iter().find(|(s, _)| s == "athletics").map(|(_, m)| *m).unwrap_or(0);

    let esc_expr = format!("1d20+{}", escapee_mod);
    let grap_expr = format!("1d20+{}", grappler_athletics);

    let esc_roll = crate::dice::roll(&esc_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let grap_roll = crate::dice::roll(&grap_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = esc_roll.total >= grap_roll.total;
    let mut escaped = false;

    let mut tx = s.db.begin().await?;

    if success {
        let esc_conditions = remove_condition(escapee_snap.conditions.clone(), "grappled");
        sqlx::query("update combatants set conditions = $1, action_used = true where id = $2")
            .bind(&esc_conditions).bind(id).execute(&mut *tx).await?;

        let grap_conditions = remove_condition(grappler_snap.conditions.clone(), "grappling");
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&grap_conditions).bind(body.grappler_id).execute(&mut *tx).await?;

        escaped = true;
    } else {
        sqlx::query("update combatants set action_used = true where id = $1")
            .bind(id).execute(&mut *tx).await?;
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_grapple_escape",
        "escapee_id": id,
        "grappler_id": body.grappler_id,
        "success": success,
        "escaped": escaped,
    }).to_string());

    Ok(Json(GrappleEscapeResult {
        success,
        escapee_roll: esc_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0),
        escapee_total: esc_roll.total,
        grappler_roll: grap_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0),
        grappler_total: grap_roll.total,
        escaped,
    }))
}

pub async fn lair_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    let e: Option<Encounter> = sqlx::query_as::<_, Encounter>(
        "update encounters set lair_action_used = true where id = $1 and lair_action_used = false
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).fetch_optional(&s.db).await?;
    let e = e.ok_or_else(|| AppError::BadRequest("lair action already used this round".into()))?;
    ws::publish(e.campaign_id, json!({
        "type": "lair_action",
        "encounter_id": id,
        "round": e.round,
    }).to_string());
    Ok(Json(e))
}

#[derive(Debug, Serialize)]
pub struct LegendaryActionResult {
    pub legendary_actions_used: i32,
    pub legendary_actions_max: i32,
}

pub async fn legendary_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LegendaryActionResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select e.campaign_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let encounter_status: String = sqlx::query_scalar(
        "select e.status::text as status from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_one(&s.db).await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    if snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot use legendary actions while at 0 HP".into()));
    }
    let incapacitated = snap.conditions.iter().any(|c| {
        let cl = c.to_lowercase();
        cl.starts_with("incapacitated") || cl.starts_with("paralyzed") || cl.starts_with("petrified") || cl.starts_with("stunned") || cl.starts_with("unconscious")
    });
    if incapacitated {
        return Err(AppError::BadRequest("cannot use legendary actions while incapacitated".into()));
    }

    let updated: Option<(i32, i32)> = sqlx::query_as(
        "update combatants set legendary_actions_used = least(legendary_actions_max, legendary_actions_used + 1)
         where id = $1 and legendary_actions_used < legendary_actions_max
         returning legendary_actions_used, legendary_actions_max")
        .bind(id).fetch_optional(&s.db).await?;
    let (used, max) = updated.ok_or_else(|| AppError::BadRequest("no legendary actions remaining".into()))?;

    ws::publish(campaign_id, json!({
        "type": "combatant_legendary_action",
        "combatant_id": id,
        "legendary_actions_used": used,
        "legendary_actions_max": max,
    }).to_string());

    Ok(Json(LegendaryActionResult {
        legendary_actions_used: used,
        legendary_actions_max: max,
    }))
}

#[derive(Debug, Clone, Serialize)]
pub struct ParsedMultiAttack {
    pub attacks: Vec<ParsedSubAttack>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ParsedSubAttack {
    pub name: String,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    #[serde(default)]
    pub damage_type: String,
    pub label: Option<String>,
}

pub fn parse_npc_multiattack(
    description: &str,
    actions: &[serde_json::Value],
) -> Vec<ParsedSubAttack> {
    let desc = description.to_lowercase();
    let mut attack_names: Vec<(u32, String)> = Vec::new();

    if desc.contains('+') || desc.chars().filter(|&c| c.is_ascii_digit()).count() > 0 {
        for part in desc.split('+') {
            let part = part.trim();
            let (cnt, nm): (u32, String) = if let Some(d) = part.chars().next().and_then(|c| c.to_digit(10)) {
                (d, part.chars().skip(1).collect::<String>().trim().to_string())
            } else {
                let words: Vec<&str> = part.split_whitespace().collect();
                if words.len() >= 2 {
                    let c = match words[0] {
                        "one" | "a" | "an" => 1,
                        "two" => 2,
                        "three" => 3,
                        "four" => 4,
                        "five" => 5,
                        _ => 1,
                    };
                    (c, words[1..].join(" "))
                } else {
                    (1, part.to_string())
                }
            };
            if !nm.is_empty() {
                attack_names.push((cnt, nm));
            }
        }
    }

    if attack_names.is_empty() {
        if let Some(attacks_part) = desc.split(':').nth(1) {
            for segment in attacks_part.split(',') {
                let seg = segment.trim();
                for prefix in &["one with its ", "one with his ", "one with her ", "one "] {
                    if let Some(rest) = seg.strip_prefix(prefix) {
                        let name = rest.trim_end_matches(&['.', ' '][..]).to_string();
                        if !name.is_empty() {
                            attack_names.push((1, name));
                        }
                        break;
                    }
                }
            }
        }
    }

    if attack_names.is_empty() {
        let p3_count = desc.split_whitespace().find_map(|w| {
            w.chars().next().and_then(|c| c.to_digit(10))
        }).unwrap_or(1);
        if let Some(first_atk) = actions.iter().find(|a| {
            let aname = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            aname != "multiattack"
        }) {
            let aname = first_atk.get("name").and_then(|v| v.as_str()).unwrap_or("attack").to_string();
            attack_names.push((p3_count, aname));
        }
    }

    let mut results: Vec<ParsedSubAttack> = Vec::new();
    let actions_lower: Vec<(String, &serde_json::Value)> = actions.iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str())?;
            Some((name.to_lowercase(), a))
        })
        .collect();

    for (count, name_hint) in attack_names {
        let hint = name_hint.trim().to_lowercase();
        let found = actions_lower.iter().find(|(n, _)| *n == hint)
            .or_else(|| actions_lower.iter().find(|(n, _)| n.contains(&hint) || hint.contains(n)))
            .or_else(|| actions_lower.iter().find(|(n, _)| n != &"multiattack"));

        if let Some((_, action)) = found {
            let atk_bonus = action.get("attack_bonus").and_then(|v| v.as_i64()).unwrap_or(0);
            let dam = action.get("damage").and_then(|v| v.as_str()).unwrap_or("1d4");
            let dtype = action.get("damage_type").and_then(|v| v.as_str()).unwrap_or("bludgeoning");
            let aname = action.get("name").and_then(|v| v.as_str()).unwrap_or("Attack");
            for _ in 0..count {
                results.push(ParsedSubAttack {
                    name: aname.to_string(),
                    attack_expression: Some(format!("1d20+{}", atk_bonus)),
                    damage_expression: Some(dam.to_string()),
                    damage_type: dtype.to_string(),
                    label: Some(aname.to_string()),
                });
            }
        }
    }

    results
}

pub async fn try_parse_npc_multiattack(db: &sqlx::PgPool, combatant_id: Uuid) -> Result<ParsedMultiAttack, String> {
    let npc_id: Option<Uuid> = sqlx::query_scalar(
        "select npc_id from combatants where id = $1")
        .bind(combatant_id).fetch_optional(db).await
        .map_err(|e| e.to_string())?
        .flatten()
        .ok_or_else(|| "not an NPC combatant".to_string())?;

    let npc_stats: Option<serde_json::Value> = sqlx::query_scalar(
        "select stats from npcs where id = $1")
        .bind(npc_id).fetch_optional(db).await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "NPC not found".to_string())?;

    let stats = npc_stats.ok_or_else(|| "NPC has no stats".to_string())?;
    let actions: Vec<serde_json::Value> = stats.get("actions")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let multiattack_action = actions.iter().find(|a| {
        a.get("name").and_then(|v| v.as_str())
            .map(|n| n.to_lowercase() == "multiattack")
            .unwrap_or(false)
    }).ok_or_else(|| "NPC has no Multiattack action".to_string())?;

    let description = multiattack_action.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if description.is_empty() {
        return Err("Multiattack action has no description".to_string());
    }

    let attacks = parse_npc_multiattack(description, &actions);
    if attacks.is_empty() {
        return Err(format!("could not parse multiattack description: {}", description));
    }

    Ok(ParsedMultiAttack { attacks })
}

pub async fn parse_multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ParsedMultiAttack>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        r#"select e.campaign_id from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let parsed = try_parse_npc_multiattack(&s.db, id).await
        .map_err(|e| AppError::BadRequest(e))?;
    Ok(Json(parsed))
}

#[derive(Debug, Deserialize)]
pub struct MultiAttackTarget {
    pub target_id: Uuid,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    pub damage_type: String,
    pub damage_die: Option<String>,
    pub ability: Option<String>,
    pub weapon_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MultiAttackBody {
    pub targets: Vec<MultiAttackTarget>,
}

#[derive(Debug, Serialize)]
pub struct MultiAttackResult {
    pub results: Vec<combat_engine::AttackResult>,
    pub targets_hit: usize,
    pub total_damage: i32,
}

pub async fn multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MultiAttackBody>,
) -> AppResult<Json<MultiAttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let needs_auto = body.targets.iter().all(|t| t.attack_expression.is_none() && t.weapon_id.is_none());
    let targets: Vec<MultiAttackTarget> = if !needs_auto {
        body.targets.iter().map(|t| MultiAttackTarget {
            target_id: t.target_id,
            attack_expression: t.attack_expression.clone(),
            damage_expression: t.damage_expression.clone(),
            damage_type: t.damage_type.clone(),
            damage_die: t.damage_die.clone(),
            ability: t.ability.clone(),
            weapon_id: t.weapon_id.clone(),
            label: t.label.clone(),
        }).collect()
    } else if let Ok(ParsedMultiAttack { attacks }) = try_parse_npc_multiattack(&s.db, id).await {
        if attacks.is_empty() {
            return Err(AppError::BadRequest("no targets and could not parse NPC multiattack".into()));
        }
        body.targets.iter().enumerate().map(|(i, t)| {
            let atk = attacks.get(i).cloned().unwrap_or_default();
            MultiAttackTarget {
                target_id: t.target_id,
                attack_expression: t.attack_expression.clone().or(atk.attack_expression),
                damage_expression: t.damage_expression.clone().or(atk.damage_expression),
                damage_type: if t.damage_type == "slashing" && !atk.damage_type.is_empty() { atk.damage_type } else { t.damage_type.clone() },
                damage_die: t.damage_die.clone(),
                ability: t.ability.clone(),
                weapon_id: t.weapon_id.clone(),
                label: t.label.clone().or(atk.label),
            }
        }).collect()
    } else {
        return Err(AppError::BadRequest("no targets specified".into()));
    };

    if targets.is_empty() {
        return Err(AppError::BadRequest("no targets specified".into()));
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let mut results = Vec::new();
    let mut total_damage = 0i32;
    let mut targets_hit = 0usize;

    for t in &targets {
        let target_snap = combat_engine::load_snapshot(&s.db, t.target_id).await?;
        if target_snap.encounter_id != attacker_snap.encounter_id {
            continue;
        }
        let target_stats = combat_engine::compute_stats(&target_snap);

        let req = combat_engine::AttackReq {
            target_id: t.target_id,
            attack_expression: t.attack_expression.clone(),
            damage_expression: t.damage_expression.clone(),
            damage_type: t.damage_type.clone(),
            damage_die: t.damage_die.clone(),
            ability: t.ability.clone(),
            proficient: Some(true),
            advantage: false,
            disadvantage: false,
            cover: None,
            is_spell_attack: false,
            is_magical: false,
            label: t.label.clone(),
            weapon_id: t.weapon_id.clone(),
            extra_damage_expression: None,
            extra_damage_type: None,
            power_attack: false,
            reckless: false,
            bless_dice: None,
            bardic_inspiration_dice: None,
        };

        match combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats) {
            Ok(res) => {
                if res.hit {
                    targets_hit += 1;
                    total_damage += res.damage_applied;
                }
                results.push(res);
            }
            Err(_) => continue,
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    for (i, t) in body.targets.iter().enumerate() {
        if let Some(res) = results.get(i) {
            if res.hit {
                sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                    .bind(res.target_hp_after)
                    .bind(res.target_temp_hp_after)
                    .bind(t.target_id)
                    .execute(&mut *tx).await?;
                if res.concentration_broken {
                    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                        .bind(t.target_id).execute(&mut *tx).await?;
                }
                if let Err(e) = super::actions::sync_combatant_hp_to_sheet_tx(&mut *tx, t.target_id, res.target_hp_after, res.target_temp_hp_after).await { tracing::error!(combatant_id = %t.target_id, "sync sheet HP: {e}"); }
                sqlx::query(
                    "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
                    .bind(attacker_snap.encounter_id)
                    .bind(round)
                    .bind(id)
                    .bind(t.target_id)
                    .bind(format!("Multiattack: {} damage", res.damage_applied))
                    .bind(-res.damage_applied)
                    .bind(t.label.as_deref())
                    .execute(&mut *tx).await?;
            }
        }
    }
    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_multiattack",
        "attacker_id": id,
        "targets_hit": targets_hit,
        "total_damage": total_damage,
    }).to_string());

    Ok(Json(MultiAttackResult { results, targets_hit, total_damage }))
}

pub async fn trigger_ready(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Option<String>, bool, bool, String) = sqlx::query_as(
        r#"select e.campaign_id, c.readied_action, c.action_used, c.reaction_used, e.status::text
           from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, readied, _action_used, reaction_used, status) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if readied.is_none() {
        return Err(AppError::BadRequest("no readied action to trigger".into()));
    }
    if reaction_used {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             reaction_used = true,
             readied_action = null,
             action_used = false
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#)
        .bind(id).fetch_one(&s.db).await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_readied_triggered",
        "combatant_id": id,
        "readied_action": readied,
    }).to_string());

    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct ClassFeatureBody {
    pub feature: String,
    #[serde(alias = "_target_id")]
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct ClassFeatureResult {
    pub feature: String,
    pub success: bool,
    pub message: String,
    pub hp_after: Option<i32>,
    pub effect_applied: bool,
}

pub async fn class_feature(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ClassFeatureBody>,
) -> AppResult<Json<ClassFeatureResult>> {
    let row: (Uuid, Option<Uuid>, String, Option<Uuid>, Uuid) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, e.status::text, c.character_id, c.encounter_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, owner, status, character_id, id_encounter) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let feature = body.feature.to_lowercase();
    let message: String;
    let mut hp_after = None;
    let effect_applied: bool;

    match feature.as_str() {
        "action_surge" => {
            sqlx::query("update combatants set action_used = false where id = $1")
                .bind(id).execute(&s.db).await?;
            message = "Action Surge! You can take an additional action.".into();
            effect_applied = true;
        }
        "second_wind" => {
            if let Some(chid) = character_id {
                let consumed: Option<Uuid> = sqlx::query_scalar(
                    "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
                    .bind(id).fetch_optional(&s.db).await?;
                if consumed.is_none() {
                    return Err(AppError::BadRequest("bonus action already used".into()));
                }
                let fighter_level: i32 = sqlx::query_scalar(
                    "select coalesce((sheet->>'level_total')::int, 1) from characters where id = $1")
                    .bind(chid).fetch_one(&s.db).await?;
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let roll = crate::dice::roll(&format!("1d10+{}", fighter_level), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                let heal = roll.total;
                let snap = combat_engine::load_snapshot(&s.db, id).await?;
                let new_hp = (snap.hp_current + heal).min(snap.hp_max);
                sqlx::query("update combatants set hp_current = $1 where id = $2")
                    .bind(new_hp).bind(id).execute(&s.db).await?;
                if let Err(e) = super::actions::sync_combatant_hp_to_sheet(&s.db, id, new_hp, snap.temp_hp).await { tracing::error!(combatant_id = %id, "sync sheet HP: {e}"); }
                hp_after = Some(new_hp);
                message = format!("Second Wind heals {} HP", heal);
                effect_applied = true;
            } else {
                return Err(AppError::BadRequest("Second Wind requires a linked character".into()));
            }
        }
        "rage" => {
            let barbarian_level: i32 = if let Some(chid) = character_id {
                sqlx::query_scalar(
                    r#"select coalesce((
                         select (elem->>'level')::int
                         from characters, jsonb_array_elements(sheet->'classes') as elem
                         where id = $1 and lower(elem->>'name') = 'barbarian'
                         limit 1
                       ), 1)"#)
                    .bind(chid).fetch_optional(&s.db).await?.unwrap_or(1)
            } else { 1 };
            let rage_dmg_bonus = if barbarian_level >= 16 { 4 } else if barbarian_level >= 9 { 3 } else { 2 };

            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Rage' and active = true")
                .bind(id).execute(&s.db).await?;

            let rage_mods = serde_json::json!({
                "damage_bonus": rage_dmg_bonus,
                "damage_resistance": ["bludgeoning", "piercing", "slashing"],
                "attack_advantage": true
            });
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Rage', 'buff', 'swords', 'manual', null, null, 'round_end',
                           false, true, $2, 'ability')"#)
                .bind(id).bind(rage_mods).execute(&s.db).await?;

            let mut conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
                .bind(id).fetch_one(&s.db).await?;
            if !has_condition(&conditions, "rage") {
                conditions.push("rage".to_string());
            }
            sqlx::query("update combatants set conditions = $1, bonus_action_used = true where id = $2")
                .bind(&conditions).bind(id).execute(&s.db).await?;
            message = format!("Rage! +{} damage, BPS resistance, STR advantage.", rage_dmg_bonus);
            effect_applied = true;
        }
        "lay_on_hands" => {
            let target_id = body.target_id.ok_or(AppError::BadRequest("target_id required for Lay on Hands".into()))?;
            let chid = character_id.ok_or(AppError::BadRequest("Lay on Hands requires a linked character".into()))?;

            // M17: target must be in the same encounter as the caster
            let target_enc: Option<Uuid> = sqlx::query_scalar(
                "select encounter_id from combatants where id = $1")
                .bind(target_id).fetch_optional(&s.db).await?;
            let target_enc = target_enc.ok_or(AppError::NotFound)?;
            if target_enc != id_encounter {
                return Err(AppError::BadRequest("Lay on Hands target must be in the same encounter".into()));
            }

            let pool: Option<serde_json::Value> = sqlx::query_scalar(
                r#"select elem from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') like '%lay on hands%'
                   limit 1"#)
                .bind(chid).fetch_optional(&s.db).await?;
            let (pool_current, _pool_id): (i32, String) = if let Some(p) = pool {
                let cur = p.get("current").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let rid = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (cur, rid)
            } else {
                return Err(AppError::BadRequest("No Lay on Hands pool found on character sheet".into()));
            };
            if pool_current <= 0 {
                return Err(AppError::BadRequest("Lay on Hands pool is empty".into()));
            }

            let target_snap = combat_engine::load_snapshot(&s.db, target_id).await?;
            let missing = (target_snap.hp_max - target_snap.hp_current).max(0);
            let heal_amt = pool_current.min(missing).max(1);
            let new_hp = (target_snap.hp_current + heal_amt).min(target_snap.hp_max);

            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet,
                     ('{resources,' || idx - 1 || ',current}')::text[],
                     to_jsonb($2::int)
                   )
                   from (select position - 1 as idx
                         from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                         where id = $1 and lower(t.elem->>'name') like '%lay on hands%'
                         limit 1) sub
                   where id = $1"#)
                .bind(chid).bind(pool_current - heal_amt).execute(&s.db).await?;

            sqlx::query("update combatants set hp_current = $1 where id = $2")
                .bind(new_hp).bind(target_id).execute(&s.db).await?;
            if let Err(e) = super::actions::sync_combatant_hp_to_sheet(&s.db, target_id, new_hp, target_snap.temp_hp).await { tracing::error!(combatant_id = %target_id, "sync sheet HP: {e}"); }

            hp_after = Some(new_hp);
            message = format!("Lay on Hands heals {} HP (pool: {} remaining)", heal_amt, pool_current - heal_amt);
            effect_applied = true;
        }
        "uncanny_dodge" => {
            let consumed: Option<Uuid> = sqlx::query_scalar(
                "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 returning id")
                .bind(id).fetch_optional(&s.db).await?;
            if consumed.is_none() {
                return Err(AppError::BadRequest("reaction already used or cannot act".into()));
            }
            // PHB: Uncanny Dodge halves incoming attack damage. Read from pending_hits queue
            // (FIFO) so multiple hits in the same round don't all trigger on the same stale value.
            let row: (serde_json::Value, i32, i32) = sqlx::query_as(
                "select pending_hits, hp_current, hp_max from combatants where id = $1")
                .bind(id).fetch_one(&s.db).await?;
            let (pending_raw, hp_cur, hp_max_col) = row;
            let mut hits: Vec<serde_json::Value> = pending_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned();
            let final_dmg: i32 = if let Some(h) = &hit {
                h.get("damage").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0)
            } else {
                // Fallback: legacy last_hit_damage column
                sqlx::query_scalar("select last_hit_damage from combatants where id = $1")
                    .bind(id).fetch_optional(&s.db).await?.unwrap_or(0)
            };
            // PHB: halve is floor, restore half damage to HP. Capped at effective max.
            let halve = (final_dmg / 2).max(0);
            let sheet_red: i32 = combat_engine::load_snapshot(&s.db, id).await?
                .sheet_raw.get("hp_max_reduction")
                .and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(0);
            let effective_max = (hp_max_col - sheet_red).max(1);
            let new_hp = (hp_cur + halve).min(effective_max);
            // Pop the consumed hit
            if hit.is_some() { hits.pop(); }
            let new_pending = serde_json::Value::Array(hits);
            sqlx::query("update combatants set hp_current = $1, last_hit_damage = null, pending_hits = $2 where id = $3")
                .bind(new_hp).bind(&new_pending).bind(id).execute(&s.db).await?;
            message = format!("Uncanny Dodge! Damage halved, healed {} HP.", halve);
            effect_applied = true;
        }
        _ => {
            return Err(AppError::BadRequest(format!("unknown class feature: {}", body.feature)));
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_class_feature",
        "combatant_id": id,
        "feature": feature,
        "message": &message,
        "hp_after": hp_after,
    }).to_string());

    Ok(Json(ClassFeatureResult {
        feature: body.feature,
        success: effect_applied,
        message,
        hp_after,
        effect_applied,
    }))
}


