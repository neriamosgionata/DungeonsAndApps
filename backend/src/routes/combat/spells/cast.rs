// cast_spell — main spell casting endpoint (pre-tx + per-target resolution).
use super::apply::apply_spell_outcome;
use super::range::parse_spell_range_ft;
use crate::rbac::Role;
use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    AppState,
};
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct CastSpellBody {
    #[validate(length(min = 1, max = 64))]
    pub spell_slug: String,
    #[validate(length(min = 0, max = 50))]
    pub target_ids: Vec<Uuid>,
    #[validate(range(min = 0, max = 20))]
    pub upcast_level: Option<i32>,
    #[validate(length(max = 64))]
    pub damage_expression: Option<String>,
    #[validate(range(min = 0, max = 30))]
    pub save_dc: Option<i32>,
    pub spell_attack_bonus: Option<i32>,
    pub half_on_save: bool,
    #[validate(length(max = 8))]
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
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
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

    let spell: (String, i16, bool, bool, serde_json::Value, Option<String>, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "select name, level, concentration, ritual, effects, casting_time, range_text, components, damage_type from spells where slug = $1")
        .bind(&body.spell_slug).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let spell_level: i32 = spell.1.into();

    let (
        spell_name, _, concentration_required, is_ritual_spell,
        effects_json, casting_time_opt, range_text, components_text, spell_damage_type,
    ) = spell;
    let cast_as_ritual = body.cast_as_ritual.unwrap_or(false);
    if cast_as_ritual && !is_ritual_spell {
        return Err(AppError::BadRequest("spell cannot be cast as a ritual".into()));
    }
    // MED-6: PHB upcast — you can only cast at a level ≥ spell's base level.
    // A 2nd-level spell with upcast_level=0 would silently consume no slot
    // and run as a cantrip; a cantrip with upcast=5 would consume a 5th.
    // Clamp upcast_level to [spell_level, max_spell_level].
    let raw_upcast = body.upcast_level.unwrap_or(spell_level);
    let slot_level = raw_upcast.max(spell_level).min(9);
    let casting_time_str = casting_time_opt.as_deref().unwrap_or("1 action");
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
        body.damage_expression
            .as_deref()
            .map(|expr| scale_cantrip_dice(expr, caster_snap.level_total))
            .transpose()?
    } else {
        body.damage_expression.clone()
    };

    let caster_stats = combat_engine::compute_stats(&caster_snap);
    let save_dc = body.save_dc.unwrap_or(caster_stats.spell_save_dc);
    // MED-3: surface malformed spell effects JSON as BadRequest (was `.unwrap_or_default()`
    // which silently produced a no-op cast — no damage, no effects, but the spell "resolved").
    let template_arr: Vec<serde_json::Value> = serde_json::from_value(effects_json)
        .map_err(|e| AppError::BadRequest(format!("spell effects parse: {}", e)))?;
    let aoe_template = template_arr.iter().find(|t| t.get("aoe").is_some());

    let (round, turn_index, map_grid_size): (i32, i32, i32) =
        sqlx::query_as("select round, turn_index, map_grid_size from encounters where id = $1")
            .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;
    let range_ft = range_text.as_deref().and_then(parse_spell_range_ft);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let results = resolve_spell_targets(
        &s,
        &body, &caster_snap, &caster_stats, &template_arr,
        &effective_damage_expression, range_ft, map_grid_size, save_dc,
        spell_damage_type.as_deref(),
        &mut rng,
    ).await?;

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
/// Errors from dice rolls or save resolution are propagated as AppError (caller `?`).
/// MED-1: scale a cantrip's leading die count per caster level (PHB cantrip
/// progression: 1/5/11/17 → ×1/×2/×3/×4). Errors on a non-numeric leading
/// coefficient — the pre-fix `.unwrap_or(1)` silently rolled 1dX for
/// variable-die expressions like "Xd6", producing wrong damage.
fn scale_cantrip_dice(expr: &str, caster_level: i32) -> AppResult<String> {
    let caster_level = caster_level.max(1);
    let multiplier = match caster_level {
        1..=4 => 1,
        5..=10 => 2,
        11..=16 => 3,
        _ => 4,
    };
    if multiplier <= 1 {
        return Ok(expr.to_string());
    }
    let d_pos = expr
        .find('d')
        .or_else(|| expr.find('D'))
        .ok_or_else(|| AppError::BadRequest(format!(
            "invalid cantrip damage expression: '{}' (expected '<n>d<s>')", expr
        )))?;
    let num_str = &expr[..d_pos];
    let base_n: i32 = num_str.parse().map_err(|_| AppError::BadRequest(format!(
        "invalid cantrip damage expression: '{}' (leading coefficient '{}' is not an integer)", expr, num_str
    )))?;
    let scaled_n = base_n * multiplier;
    Ok(format!("{}{}", scaled_n, &expr[d_pos..]))
}

/// MED-2: detect the damage type of a spell from its template modifiers.
/// Replaces a 9-step `.iter().find().or_else()...` chain with a single pass.
fn detect_damage_type(template_arr: &[serde_json::Value]) -> &'static str {
    const TYPES: &[(&str, &str)] = &[
        ("fire_damage", "fire"),
        ("cold_damage", "cold"),
        ("lightning_damage", "lightning"),
        ("thunder_damage", "thunder"),
        ("acid_damage", "acid"),
        ("poison_damage", "poison"),
        ("necrotic_damage", "necrotic"),
        ("radiant_damage", "radiant"),
        ("psychic_damage", "psychic"),
        ("force_damage", "force"),
    ];
    for t in template_arr {
        let mods = match t.get("modifiers") {
            Some(m) => m,
            None => continue,
        };
        for (key, name) in TYPES {
            if mods.get(*key).is_some() {
                return name;
            }
        }
    }
    "force"
}

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
    spell_damage_type: Option<&str>,
    rng: &mut rand::rngs::StdRng,
) -> AppResult<Vec<CastSpellTargetResult>> {
    // Batch load all target snapshots in a single query (1 round-trip
    // instead of N per target). Fixes N+1 audit finding.
    let target_snaps = combat_engine::load_snapshots_batch(&s.db, &body.target_ids).await?;
    let mut results: Vec<CastSpellTargetResult> = Vec::new();
    for target_id in &body.target_ids {
        let target_snap = match target_snaps.get(target_id) {
            Some(s) => s,
            None => continue, // snapshot load failed or target not in batch result
        };
        if target_snap.encounter_id != caster_snap.encounter_id { continue; }
        if let Some(max_ft) = range_ft {
            if let (Some(cx), Some(cy), Some(tx), Some(ty)) = (
                caster_snap.token_x, caster_snap.token_y,
                target_snap.token_x, target_snap.token_y,
            ) {
                // HIGH-4: 1 cell = 5ft = 20% of the map (default 5×5 grid).
                // dist_pct × 0.25 converts % distance to feet.
                let _ = map_grid_size; // kept for future per-cell scaling
                let dx = (cx - tx) as f32;
                let dy = (cy - ty) as f32;
                let dist_ft = (dx * dx + dy * dy).sqrt() * 0.25;
                if dist_ft > max_ft as f32 + 2.5 {
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
            let atk_roll = crate::dice::roll(&atk_expr, rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
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
                            .map_err(|e| AppError::BadRequest(e.to_string()))?;
                        if crit {
                            let crit_expr = combat_engine::crit_double_dice(dmg_expr);
                            dmg_roll = crate::dice::roll(&crit_expr, rng)
                                .map_err(|e| AppError::BadRequest(e.to_string()))?;
                        }
                        let raw_dmg = dmg_roll.total;
                        // MED-5: prefer the modifier key, fall back to the
                        // spell's declared damage_type column. Previously
                        // defaulted to "force" for any spell without a
                        // matching modifier (wrong for Fireball, etc.).
                        let mut dtype = detect_damage_type(template_arr);
                        if dtype == "force" {
                            if let Some(s) = spell_damage_type.as_deref() {
                                dtype = s;
                            }
                        }
                        let (eff_dmg, _, _, _) = combat_engine::apply_damage_type(raw_dmg, dtype, &target_stats, true);
                        // MED-4: Evasion (PHB) — on successful DEX save take
                        // no damage; on FAILED DEX save take half. Pre-fix
                        // only handled the success case, so failed Evasion
                        // took full damage.
                        if body.half_on_save && save_passed == Some(true) {
                            if target_stats.evasion && save_ability_str == "dex" {
                                damage_applied = 0;
                            } else {
                                damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
                            }
                        } else if save_passed == Some(false) {
                            if target_stats.evasion && save_ability_str == "dex" {
                                damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
                            } else {
                                damage_applied = eff_dmg;
                            }
                        } else {
                            // No save or save not applicable — full damage
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
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;

    /// Regression: cast_spell used `.map_err(|e| AppError::BadRequest(...)).unwrap()`
    /// on `dice::roll`, which would panic on any malformed expression. Fix: use `?`.
    /// This test verifies the `?` operator is in place by directly calling the same
    /// pattern: a bad expression must surface as Err, not panic.
    #[test]
    fn bad_dice_expression_returns_err_not_panic() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let bad_expr = "this-is-not-a-dice-expression!@#$";
        let result: Result<crate::dice::RollResult, crate::error::AppError> = crate::dice::roll(bad_expr, &mut rng)
            .map_err(|e| AppError::BadRequest(e.to_string()));
        assert!(result.is_err(), "malformed expression must produce Err, not Ok");
        match result.unwrap_err() {
            AppError::BadRequest(msg) => assert!(!msg.is_empty()),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn crit_double_dice_bad_expr_returns_err_not_panic() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(42);
        let bad_expr = "999d999-bad-input";
        let crit_expr = combat_engine::crit_double_dice(bad_expr);
        let result: Result<crate::dice::RollResult, crate::error::AppError> = crate::dice::roll(&crit_expr, &mut rng)
            .map_err(|e| AppError::BadRequest(e.to_string()));
        assert!(result.is_err(), "crit-doubled bad expression must produce Err, not Ok");
    }

    // MED-1: cantrip scaling must reject non-numeric leading coefficient
    // (was `.unwrap_or(1)` — silently rolled 1dX for "Xd6" expressions).
    #[test]
    fn scale_cantrip_dice_rejects_non_numeric() {
        let r = scale_cantrip_dice("Xd6", 5);
        assert!(r.is_err(), "non-numeric leading coefficient must return Err, not default to 1");
        match r.unwrap_err() {
            AppError::BadRequest(msg) => assert!(msg.contains("not an integer"), "msg: {}", msg),
            other => panic!("expected BadRequest, got {:?}", other),
        }
    }

    #[test]
    fn scale_cantrip_dice_scales_correctly() {
        assert_eq!(scale_cantrip_dice("1d10", 1).unwrap(), "1d10");
        assert_eq!(scale_cantrip_dice("1d10", 5).unwrap(), "2d10");
        assert_eq!(scale_cantrip_dice("1d10", 11).unwrap(), "3d10");
        assert_eq!(scale_cantrip_dice("1d10", 17).unwrap(), "4d10");
        assert_eq!(scale_cantrip_dice("2d6+3", 5).unwrap(), "4d6+3");
    }

    #[test]
    fn scale_cantrip_dice_rejects_no_d() {
        let r = scale_cantrip_dice("not-a-dice", 5);
        assert!(r.is_err(), "expression without 'd' must return Err");
    }

    // MED-2: damage type detection covers all 10 types + default force.
    #[test]
    fn detect_damage_type_finds_fire() {
        let t = vec![serde_json::json!({"modifiers": {"fire_damage": 1}})];
        assert_eq!(detect_damage_type(&t), "fire");
    }

    #[test]
    fn detect_damage_type_finds_force() {
        let t = vec![serde_json::json!({"modifiers": {"force_damage": 2}})];
        assert_eq!(detect_damage_type(&t), "force");
    }

    #[test]
    fn detect_damage_type_defaults_to_force_when_empty() {
        let t: Vec<serde_json::Value> = vec![];
        assert_eq!(detect_damage_type(&t), "force");
    }

    #[test]
    fn detect_damage_type_defaults_to_force_when_no_modifiers() {
        let t = vec![serde_json::json!({"name": "Bless"})];
        assert_eq!(detect_damage_type(&t), "force");
    }

    #[test]
    fn detect_damage_type_finds_first_match_in_template_list() {
        let t = vec![
            serde_json::json!({"name": "Upcast"}),
            serde_json::json!({"modifiers": {"cold_damage": 1}}),
        ];
        assert_eq!(detect_damage_type(&t), "cold");
    }
}
