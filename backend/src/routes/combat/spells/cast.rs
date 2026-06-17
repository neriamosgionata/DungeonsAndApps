// cast_spell — main spell casting endpoint (pre-tx + per-target resolution).
use super::apply::apply_spell_outcome;
use super::range::parse_spell_range_ft;
use super::*;
use crate::rbac::Role;
use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac, ws,
    AppState,
};
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::instrument;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CastSpellBody {
    pub spell_slug: String,
    pub target_ids: Vec<Uuid>,
    pub upcast_level: Option<i32>,
    pub damage_expression: Option<String>,
    pub save_dc: Option<i32>,
    pub spell_attack_bonus: Option<i32>,
    pub half_on_save: bool,
    pub save_ability: Option<String>,
    pub cast_as_ritual: Option<bool>,
    pub use_spell_attack: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct CastSpellTargetResult {
    pub target_id: Uuid,
    pub target_name: String,
    pub hit: Option<bool>,
    pub critical: bool,
    pub attack_total: Option<i32>,
    pub save_passed: Option<bool>,
    pub save_total: Option<i32>,
    pub damage_applied: i32,
    pub hp_after: i32,
    pub temp_hp_after: i32,
    pub instant_death: bool,
    pub effects_applied: Vec<String>,
    pub concentration_broken: bool,
}

#[derive(Debug, Serialize)]
pub struct CastSpellResult {
    pub spell_name: String,
    pub spell_level: i32,
    pub caster_id: Uuid,
    pub slot_level_consumed: i32,
    pub targets: Vec<CastSpellTargetResult>,
    pub overlay_created: Option<Uuid>,
    pub concentration_required: bool,
}

#[tracing::instrument(skip(s, body), fields(uid = %uid, caster_id = %caster_id))]
pub async fn cast_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(caster_id): Path<Uuid>,
    Json(body): Json<CastSpellBody>,
) -> AppResult<Json<CastSpellResult>> {
    let caster_snap = combat_engine::load_snapshot(&s.db, caster_id).await?;
    let (campaign_id, encounter_status): (Uuid, String) =
        sqlx::query_as("select campaign_id, status::text as status from encounters where id = $1")
            .bind(caster_snap.encounter_id)
            .fetch_one(&s.db)
            .await?;
    if encounter_status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(caster_id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if caster_snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot cast spells while at 0 HP".into()));
    }
    let caster_incap = caster_snap.conditions.iter().any(|c| {
        matches!(c.to_lowercase().as_str(), s if s.starts_with("incapacitated") || s.starts_with("paralyzed") || s.starts_with("petrified") || s.starts_with("stunned") || s.starts_with("unconscious"))
    });
    if caster_incap {
        return Err(AppError::BadRequest("cannot cast spells while incapacitated".into()));
    }

    let spell: (String, i32, bool, bool, serde_json::Value, serde_json::Value, Option<String>, Option<String>) = sqlx::query_as(
        "select name, level, concentration, ritual, effects, casting_time, range_text, components from spells where slug = $1")
        .bind(&body.spell_slug).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (
        spell_name, spell_level, concentration_required, is_ritual_spell,
        effects_json, _casting_time, range_text, components_text,
    ) = spell;
    let cast_as_ritual = body.cast_as_ritual.unwrap_or(false);
    if cast_as_ritual && !is_ritual_spell {
        return Err(AppError::BadRequest("spell cannot be cast as a ritual".into()));
    }
    let slot_level = body.upcast_level.unwrap_or(spell_level);
    let casting_time_str = _casting_time.as_str().unwrap_or("1 action");
    let is_bonus_action = casting_time_str.to_lowercase().contains("bonus");

    if role != Role::Master {
        let is_raging = caster_snap.conditions.iter().any(|c| c.to_lowercase().starts_with("rage"));
        if is_raging {
            return Err(AppError::BadRequest("cannot cast spells while raging".into()));
        }
    }

    let comps = components_text.as_deref().unwrap_or("").to_uppercase();
    if comps.contains('V') {
        let is_silenced = caster_snap.active_effects.iter().any(|e| {
            e.modifiers.get("silenced").and_then(|v| v.as_bool()).unwrap_or(false)
        });
        if is_silenced {
            return Err(AppError::BadRequest("cannot cast: silenced (no verbal component)".into()));
        }
    }
    if comps.contains('S') {
        let has_war_caster = caster_snap.sheet_raw.get("feats").and_then(|v| v.as_array())
            .map(|arr| arr.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("war_caster")))
            .unwrap_or(false);
        if !has_war_caster {
            let no_somatic = caster_snap.active_effects.iter().any(|e| {
                e.modifiers.get("no_somatic").and_then(|v| v.as_bool()).unwrap_or(false)
            });
            if no_somatic {
                return Err(AppError::BadRequest("cannot cast: somatic component blocked".into()));
            }
        }
    }

    if spell_level > 0 && role != Role::Master {
        if let Some(chid) = caster_snap.character_id {
            let primary_class = caster_snap.classes.as_array().and_then(|arr| arr.first())
                .and_then(|c| c.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let requires_preparation = matches!(primary_class.as_str(), "wizard" | "cleric" | "druid" | "paladin" | "artificer");
            let is_known_caster = matches!(primary_class.as_str(), "sorcerer" | "bard" | "warlock" | "ranger" | "rogue");
            if requires_preparation || is_known_caster {
                let row: Option<(Option<bool>, Option<bool>)> = sqlx::query_as(
                    r#"select cs.prepared, cs.known from character_spells cs join spells s on s.id = cs.spell_id where cs.character_id = $1 and s.slug = $2"#)
                .bind(chid).bind(&body.spell_slug).fetch_optional(&s.db).await?;
                match row {
                    None => return Err(AppError::BadRequest(format!("'{}' is not in {}'s spell list", spell_name, primary_class))),
                    Some((_, Some(true))) if is_known_caster => {}
                    Some((Some(true), _)) if requires_preparation => {}
                    Some(_) => {
                        if requires_preparation {
                            return Err(AppError::BadRequest(format!("'{}' is not prepared", spell_name)));
                        } else {
                            return Err(AppError::BadRequest(format!("'{}' is not in {}'s known spells", spell_name, primary_class)));
                        }
                    }
                }
            }
        }
    }

    let effective_damage_expression = if spell_level == 0 {
        body.damage_expression.as_deref().map(|expr| {
            let caster_level = caster_snap.level_total.max(1);
            let multiplier = match caster_level { 1..=4 => 1, 5..=10 => 2, 11..=16 => 3, _ => 4 };
            if multiplier <= 1 { return expr.to_string(); }
            let re_pat = expr;
            if let Some(d_pos) = re_pat.find('d').or_else(|| re_pat.find('D')) {
                let num_str = &re_pat[..d_pos];
                let base_n: i32 = num_str.parse().unwrap_or(1);
                let scaled_n = base_n * multiplier;
                format!("{}{}", scaled_n, &re_pat[d_pos..])
            } else { expr.to_string() }
        })
    } else { body.damage_expression.clone() };

    let caster_stats = combat_engine::compute_stats(&caster_snap);
    let save_dc = body.save_dc.unwrap_or(caster_stats.spell_save_dc);
    let template_arr: Vec<serde_json::Value> = serde_json::from_value(effects_json).unwrap_or_default();
    let aoe_template = template_arr.iter().find(|t| t.get("aoe").is_some());

    let (round, turn_index, map_grid_size): (i32, i32, i32) =
        sqlx::query_as("select round, turn_index, map_grid_size from encounters where id = $1")
            .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;
    let range_ft = range_text.as_deref().and_then(parse_spell_range_ft);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let results = resolve_spell_targets(
        &s,
        &body, &caster_snap, &caster_stats, &template_arr,
        &effective_damage_expression, range_ft, map_grid_size, save_dc, &mut rng,
    ).await;

    let mut overlay_id: Option<Uuid> = None;
    apply_spell_outcome(
        &s, &body, caster_id, &caster_snap, campaign_id,
        &spell_name, spell_level, slot_level, is_bonus_action,
        concentration_required, cast_as_ritual, &template_arr, &results,
        aoe_template, round, turn_index, &mut overlay_id,
    ).await?;

    Ok(Json(CastSpellResult {
        spell_name, spell_level, caster_id, slot_level_consumed: slot_level,
        targets: results, overlay_created: overlay_id, concentration_required,
    }))
}

/// Resolve a single spell cast against all targets.
/// Returns Vec<CastSpellTargetResult> with hit/damage/concentration per target.
#[allow(clippy::too_many_arguments)]
async fn resolve_spell_targets(
    s: &AppState,
    body: &CastSpellBody,
    caster_snap: &combat_engine::CombatantSnapshot,
    caster_stats: &combat_engine::ComputedStats,
    template_arr: &[serde_json::Value],
    effective_damage_expression: &Option<String>,
    range_ft: Option<i32>,
    map_grid_size: i32,
    save_dc: i32,
    rng: &mut rand::rngs::StdRng,
) -> Vec<CastSpellTargetResult> {
    let mut results: Vec<CastSpellTargetResult> = Vec::new();
    for target_id in &body.target_ids {
        let target_snap = match combat_engine::load_snapshot(&s.db, *target_id).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        if target_snap.encounter_id != caster_snap.encounter_id { continue; }
        if let Some(max_ft) = range_ft {
            if let (Some(cx), Some(cy), Some(tx), Some(ty)) = (
                caster_snap.token_x, caster_snap.token_y,
                target_snap.token_x, target_snap.token_y,
            ) {
                let pct_per_5ft = 5.0_f32 / (map_grid_size as f32);
                let dx = (cx - tx) / pct_per_5ft;
                let dy = (cy - ty) / pct_per_5ft;
                let dist_ft = (dx * dx + dy * dy).sqrt() * 5.0;
                if dist_ft > max_ft as f32 + 2.5 {
                    // We can't return AppError from this function (no s.db).
                    // Skip target if out of range.
                    continue;
                }
            }
        }
        let target_stats = combat_engine::compute_stats(&target_snap);
        let save_ability_str = body.save_ability.as_deref().unwrap_or("dex").to_lowercase();
        let use_attack_roll = body.use_spell_attack.unwrap_or(false);
        let spell_atk_bonus = body.spell_attack_bonus.unwrap_or(caster_stats.spell_attack_bonus);
        let (hit, crit, attack_total, save_passed, save_total) = if use_attack_roll {
            let adv = caster_stats.attack_advantage;
            let dis = caster_stats.attack_disadvantage;
            let atk_expr = if adv && !dis { format!("2d20kh1+{}", spell_atk_bonus) }
                else if dis && !adv { format!("2d20kl1+{}", spell_atk_bonus) }
                else { format!("1d20+{}", spell_atk_bonus) };
            let atk_roll = crate::dice::roll(&atk_expr, rng).map_err(|e| AppError::BadRequest(e.to_string())).unwrap();
            let nat = atk_roll.terms.first()
                .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied())).unwrap_or(0);
            let crit_range = caster_snap.sheet_raw.get("crit_range").and_then(|v| v.as_i64())
                .map(|v| v as i32).unwrap_or(20);
            let critical = nat >= crit_range;
            let auto_miss = nat == 1;
            let hit = if critical { true } else if auto_miss { false } else { atk_roll.total >= target_stats.ac };
            (Some(hit), critical, Some(atk_roll.total), None, None)
        } else if effective_damage_expression.is_some() {
            let save_req = combat_engine::SaveReq {
                ability: save_ability_str.clone(), dc: save_dc, advantage: false,
                disadvantage: false, label: None, is_magical: Some(true),
            };
            let save_res = combat_engine::resolve_save(&target_snap, &save_req, &target_stats)
                .map_err(|e| AppError::BadRequest(e)).unwrap_or(combat_engine::SaveResult {
                    passed: false, natural_roll: 1, save_total: 1, dc: save_dc,
                    save_roll: crate::dice::RollResult { expression: "1d20".into(), terms: vec![], total: 1 },
                    save_advantage: false, save_disadvantage: true,
                });
            (None, false, None, Some(save_res.passed), Some(save_res.save_total))
        } else { (None, false, None, None, None) };

        let attack_missed = use_attack_roll && hit == Some(false);
        let mut damage_applied = 0i32;
        if !attack_missed {
            if let Some(dmg_expr) = effective_damage_expression.as_deref() {
                let mut dmg_roll = crate::dice::roll(dmg_expr, rng)
                    .map_err(|e| AppError::BadRequest(e.to_string())).unwrap();
                if crit {
                    let crit_expr = combat_engine::crit_double_dice(dmg_expr);
                    dmg_roll = crate::dice::roll(&crit_expr, rng)
                        .map_err(|e| AppError::BadRequest(e.to_string())).unwrap();
                }
                let raw_dmg = dmg_roll.total;
                let dtype = template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("fire_damage")).is_some())
                    .map(|_| "fire")
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("cold_damage")).is_some()).map(|_| "cold"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("lightning_damage")).is_some()).map(|_| "lightning"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("thunder_damage")).is_some()).map(|_| "thunder"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("acid_damage")).is_some()).map(|_| "acid"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("poison_damage")).is_some()).map(|_| "poison"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("necrotic_damage")).is_some()).map(|_| "necrotic"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("radiant_damage")).is_some()).map(|_| "radiant"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("psychic_damage")).is_some()).map(|_| "psychic"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("force_damage")).is_some()).map(|_| "force"))
                    .unwrap_or("force");
                let (eff_dmg, _, _, _) = combat_engine::apply_damage_type(raw_dmg, dtype, &target_stats, true);
                if body.half_on_save && save_passed == Some(true) {
                    if target_stats.evasion && save_ability_str == "dex" { damage_applied = 0; }
                    else { damage_applied = (eff_dmg as f32 / 2.0).floor() as i32; }
                } else if save_passed == Some(false) || save_passed.is_none() {
                    damage_applied = eff_dmg;
                }
            }
        }
        let (new_hp, new_temp) = combat_engine::apply_hp_damage(target_snap.hp_current, target_snap.temp_hp, damage_applied);
        let instant_death = target_snap.hp_current > 0
            && (damage_applied - target_snap.hp_current - target_snap.temp_hp).max(0) >= target_snap.hp_max;
        let mut conc_broken = false;
        if target_snap.active_effects.iter().any(|e| e.concentration) && damage_applied > 0 {
            let (broken, _) = combat_engine::concentration_check(&target_snap, damage_applied, rng);
            conc_broken = broken;
        }
        results.push(CastSpellTargetResult {
            target_id: *target_id, target_name: target_snap.display_name.clone(),
            hit, critical: crit, attack_total, save_passed, save_total,
            damage_applied, hp_after: new_hp, temp_hp_after: new_temp, instant_death,
            effects_applied: template_arr.iter()
                .filter(|t| t.get("aoe").is_none())
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect(),
            concentration_broken: conc_broken,
        });
    }
    results
}
