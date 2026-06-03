use super::*;

use super::actions::auto_trigger_ready_actions_for_event;
use super::actions::sync_combatant_hp_to_sheet_tx;

/// Parse a spell's range_text into feet for distance validation.
/// Returns None for unlimited / self / touch (no distance check).
pub fn parse_spell_range_ft(range_text: &str) -> Option<i32> {
    let s = range_text.trim().to_lowercase();
    if s == "self" || s == "touch" || s.contains("unlimited") || s.contains("special") {
        return None;
    }
    if s.contains("mile") {
        let n: i32 = s.split_whitespace().next()?.parse().ok()?;
        return Some(n * 5280);
    }
    let first = s.split_whitespace().next()?;
    first.parse::<i32>().ok()
}

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

pub async fn cast_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(caster_id): Path<Uuid>,
    Json(body): Json<CastSpellBody>,
) -> AppResult<Json<CastSpellResult>> {
    let caster_snap = combat_engine::load_snapshot(&s.db, caster_id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(caster_id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let spell: (String, i32, bool, bool, serde_json::Value, serde_json::Value, Option<String>, Option<String>) = sqlx::query_as(
        "select name, level, concentration, ritual, effects, casting_time, range_text, components from spells where slug = $1")
        .bind(&body.spell_slug)
        .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (spell_name, spell_level, concentration_required, is_ritual_spell, effects_json, _casting_time, range_text, components_text) = spell;
    let cast_as_ritual = body.cast_as_ritual.unwrap_or(false);
    if cast_as_ritual && !is_ritual_spell {
        return Err(AppError::BadRequest("spell cannot be cast as a ritual".into()));
    }
    let slot_level = body.upcast_level.unwrap_or(spell_level);

    let casting_time_str = _casting_time.as_str().unwrap_or("1 action");
    let is_bonus_action = casting_time_str.to_lowercase().contains("bonus");

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
        let has_war_caster = caster_snap.sheet_raw.get("feats")
            .and_then(|v| v.as_array())
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
            let primary_class = caster_snap.classes.as_array()
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            let requires_preparation = matches!(primary_class.as_str(),
                "wizard" | "cleric" | "druid" | "paladin" | "artificer"
            );

            if requires_preparation {
                let prepared: Option<bool> = sqlx::query_scalar(
                    r#"select cs.prepared
                       from character_spells cs
                       join spells s on s.id = cs.spell_id
                       where cs.character_id = $1 and s.slug = $2"#)
                    .bind(chid).bind(&body.spell_slug)
                    .fetch_optional(&s.db).await?;

                match prepared {
                    None => return Err(AppError::BadRequest(
                        format!("'{}' is not in your spell list", spell_name)
                    )),
                    Some(false) => return Err(AppError::BadRequest(
                        format!("'{}' is not prepared", spell_name)
                    )),
                    Some(true) => {}
                }
            }
        }
    }

    let effective_damage_expression = if spell_level == 0 {
        body.damage_expression.as_deref().map(|expr| {
            let caster_level = caster_snap.level_total.max(1);
            let multiplier = match caster_level {
                1..=4 => 1,
                5..=10 => 2,
                11..=16 => 3,
                _ => 4,
            };
            if multiplier <= 1 { return expr.to_string(); }
            let re_pat = expr;
            if let Some(d_pos) = re_pat.find('d').or_else(|| re_pat.find('D')) {
                let num_str = &re_pat[..d_pos];
                let base_n: i32 = num_str.parse().unwrap_or(1);
                let scaled_n = base_n * multiplier;
                format!("{}{}", scaled_n, &re_pat[d_pos..])
            } else {
                expr.to_string()
            }
        })
    } else {
        body.damage_expression.clone()
    };

    let caster_stats = combat_engine::compute_stats(&caster_snap);
    let save_dc = body.save_dc.unwrap_or(caster_stats.spell_save_dc);
    let _spell_atk = body.spell_attack_bonus.unwrap_or(caster_stats.spell_attack_bonus);

    let template_arr: Vec<serde_json::Value> = serde_json::from_value(effects_json)
        .unwrap_or_default();

    let aoe_template = template_arr.iter().find(|t| t.get("aoe").is_some());
    let mut overlay_id: Option<Uuid> = None;

    let mut results = Vec::new();
    let mut rng = rand::rngs::StdRng::from_os_rng();

    let (round, turn_index, map_grid_size): (i32, i32, i32) = sqlx::query_as(
        "select round, turn_index, map_grid_size from encounters where id = $1")
        .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;

    let range_ft = range_text.as_deref().and_then(parse_spell_range_ft);

    for target_id in &body.target_ids {
        let target_snap = match combat_engine::load_snapshot(&s.db, *target_id).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        if target_snap.encounter_id != caster_snap.encounter_id {
            continue;
        }

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
                    return Err(AppError::BadRequest(format!(
                        "target out of range ({:.0}ft, max {}ft)", dist_ft, max_ft
                    )));
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
            let atk_expr = if adv && !dis {
                format!("2d20kh1+{}", spell_atk_bonus)
            } else if dis && !adv {
                format!("2d20kl1+{}", spell_atk_bonus)
            } else {
                format!("1d20+{}", spell_atk_bonus)
            };
            let atk_roll = crate::dice::roll(&atk_expr, &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            let nat = atk_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0);
            let crit_range = caster_snap.sheet_raw.get("crit_range")
                .and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(20);
            let critical = nat >= crit_range;
            let auto_miss = nat == 1;
            let hit = if critical { true } else if auto_miss { false } else { atk_roll.total >= target_stats.ac };
            (Some(hit), critical, Some(atk_roll.total), None, None)
        } else if effective_damage_expression.is_some() {
            let save_req = combat_engine::SaveReq {
                ability: save_ability_str.clone(),
                dc: save_dc,
                advantage: false,
                disadvantage: false,
                label: None,
                is_magical: Some(true),
            };
            let save_res = combat_engine::resolve_save(&target_snap, &save_req, &target_stats)
                .map_err(|e| AppError::BadRequest(e))?;
            (None, false, None, Some(save_res.passed), Some(save_res.save_total))
        } else {
            (None, false, None, None, None)
        };

        let attack_missed = use_attack_roll && hit == Some(false);

        let mut damage_applied = 0i32;
        if !attack_missed {
            if let Some(ref dmg_expr) = effective_damage_expression {
                let mut dmg_roll = crate::dice::roll(dmg_expr, &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;

                if crit {
                    let crit_expr = combat_engine::crit_double_dice(dmg_expr);
                    dmg_roll = crate::dice::roll(&crit_expr, &mut rng)
                        .map_err(|e| AppError::BadRequest(e.to_string()))?;
                }

                let raw_dmg = dmg_roll.total;

                let dtype = template_arr.iter()
                    .find(|t| t.get("modifiers").and_then(|m| m.get("fire_damage")).is_some()).map(|_| "fire")
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
                    if target_stats.evasion && save_ability_str == "dex" {
                        damage_applied = 0;
                    } else {
                        damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
                    }
                } else if save_passed == Some(false) || save_passed.is_none() {
                    damage_applied = eff_dmg;
                }
            }
        }

        let (new_hp, new_temp) = combat_engine::apply_hp_damage(target_snap.hp_current, target_snap.temp_hp, damage_applied);

        let mut conc_broken = false;
        if target_snap.active_effects.iter().any(|e| e.concentration) && damage_applied > 0 {
            let (broken, _) = combat_engine::concentration_check(&target_snap, damage_applied, &mut rng);
            conc_broken = broken;
        }

        results.push(CastSpellTargetResult {
            target_id: *target_id,
            target_name: target_snap.display_name.clone(),
            hit,
            critical: crit,
            attack_total,
            save_passed,
            save_total,
            damage_applied,
            hp_after: new_hp,
            temp_hp_after: new_temp,
            effects_applied: template_arr.iter()
                .filter(|t| t.get("aoe").is_none())
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect(),
            concentration_broken: conc_broken,
        });
    }

    let mut tx = s.db.begin().await?;

    let (prev_action_spell_level, prev_bonus_spell_level): (i16, i16) = sqlx::query_as(
        "select action_spell_level, bonus_action_spell_level from combatants where id = $1")
        .bind(caster_id).fetch_one(&mut *tx).await?;
    if is_bonus_action {
        if prev_action_spell_level > 0 && spell_level > 0 {
            return Err(AppError::BadRequest(
                "you already used your action to cast a spell; bonus-action spell must be a cantrip (PHB p.203)".into()
            ));
        }
    } else {
        if prev_bonus_spell_level > 0 && spell_level > 0 {
            return Err(AppError::BadRequest(
                "you already used your bonus action to cast a spell; action spell must be a cantrip (PHB p.203)".into()
            ));
        }
    }

    sqlx::query("update combatants set spell_being_cast = $1 where id = $2")
        .bind(&body.spell_slug).bind(caster_id).execute(&mut *tx).await?;

    ws::publish(campaign_id, json!({
        "type": "reaction_window",
        "window_type": "spell_being_cast",
        "caster_id": caster_id,
        "spell_slug": body.spell_slug,
        "spell_level": spell_level,
        "slot_level": slot_level,
    }).to_string());

    let action_consumed: Option<Uuid> = if is_bonus_action {
        sqlx::query_scalar(
            "update combatants set bonus_action_used = true, bonus_action_spell_level = $2 where id = $1 and bonus_action_used = false returning id")
            .bind(caster_id).bind(spell_level as i16).fetch_optional(&mut *tx).await?
    } else {
        sqlx::query_scalar(
            "update combatants set action_used = true, action_spell_level = $2 where id = $1 and action_used = false returning id")
            .bind(caster_id).bind(spell_level as i16).fetch_optional(&mut *tx).await?
    };
    if action_consumed.is_none() {
        let msg = if is_bonus_action { "bonus action already used" } else { "action already used" };
        return Err(AppError::BadRequest(msg.into()));
    }

    if !cast_as_ritual && slot_level > 0 {
        if let Some(chid) = caster_snap.character_id {
            let slot_key = format!("{}", slot_level);
            let slot_current: Option<i32> = sqlx::query_scalar(
                "select (sheet->'slots'->$1->>'current')::int from characters where id = $2")
                .bind(&slot_key).bind(chid).fetch_optional(&mut *tx).await?;
            if let Some(current) = slot_current {
                if current <= 0 {
                    return Err(AppError::BadRequest("no spell slots of that level remaining".into()));
                }
                sqlx::query(
                    "update characters set sheet = jsonb_set(sheet, array['slots', $1, 'current'], to_jsonb($2::int)) where id = $3")
                    .bind(&slot_key).bind(current - 1).bind(chid).execute(&mut *tx).await?;
            }
        }
    }

    if concentration_required {
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(caster_id).execute(&mut *tx).await?;
    }

    for result in &results {
        let target_id = result.target_id;

        for t in &template_arr {
            if t.get("aoe").is_some() { continue; }

            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("Effect").to_string();
            let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("neutral").to_string();
            let icon = t.get("icon").and_then(|v| v.as_str()).unwrap_or("circle-dot").to_string();
            let duration_unit = t.get("duration_unit").and_then(|v| v.as_str()).unwrap_or("rounds").to_string();
            let duration_value = t.get("duration_value").and_then(|v| v.as_i64()).map(|v| v as i32);
            let tick_trigger = t.get("tick_trigger").and_then(|v| v.as_str()).unwrap_or("round_end").to_string();
            let conc = t.get("concentration").and_then(|v| v.as_bool()).unwrap_or(false);
            let modifiers = t.get("modifiers").cloned().unwrap_or_else(|| json!({}));

            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, caster_combatant_id, source_type, source_name, source_spell_slug, modifiers,
                    applied_at_round, applied_at_turn_index)
                   values ($1, $2, $3::effect_kind, $4, $5::duration_unit, $6, $7, $8::tick_trigger,
                           $9, $10, 'spell', $11, $12, $13, $14, $15)"#,
            )
            .bind(target_id)
            .bind(&name)
            .bind(&kind)
            .bind(&icon)
            .bind(&duration_unit)
            .bind(duration_value)
            .bind(duration_value)
            .bind(&tick_trigger)
            .bind(conc)
            .bind(caster_id)
            .bind(&spell_name)
            .bind(&body.spell_slug)
            .bind(modifiers)
            .bind(round)
            .bind(turn_index)
            .execute(&mut *tx).await?;
        }

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.hp_after).bind(result.temp_hp_after).bind(target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(target_id).execute(&mut *tx).await?;
        }

        let _ = sync_combatant_hp_to_sheet_tx(&mut *tx, target_id, result.hp_after, result.temp_hp_after).await;
    }

    if let Some(template) = aoe_template {
        if let Some(aoe) = template.get("aoe") {
            let shape = aoe.get("shape").and_then(|v| v.as_str()).unwrap_or("circle");
            let radius_ft = aoe.get("radius_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let length_ft = aoe.get("length_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let width_ft = aoe.get("width_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let color = aoe.get("color").and_then(|v| v.as_str()).unwrap_or("rgba(255,0,0,0.25)");
            let aoe_duration = template.get("duration_value").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(1);

            let oid: Uuid = sqlx::query_scalar(
                r#"insert into encounter_overlays
                   (encounter_id, kind, shape, origin_x, origin_y, radius_ft, length_ft, width_ft, color, label,
                    expires_at_round, source_spell_slug, created_by_combatant_id)
                   values ($1, 'aoe', $2, 50, 50, $3, $4, $5, $6, $7, $8, $9, $10)
                   returning id"#,
            )
            .bind(caster_snap.encounter_id)
            .bind(shape)
            .bind(radius_ft)
            .bind(length_ft)
            .bind(width_ft)
            .bind(color)
            .bind(&spell_name)
            .bind(round + aoe_duration)
            .bind(&body.spell_slug)
            .bind(caster_id)
            .fetch_one(&mut *tx).await?;
            overlay_id = Some(oid);
        }
    }

    tx.commit().await?;

    sqlx::query("update combatants set spell_being_cast = null where id = $1")
        .bind(caster_id).execute(&s.db).await?;

    auto_trigger_ready_actions_for_event(&s.db, campaign_id, caster_snap.encounter_id,
        "target_casts", caster_id, caster_id).await;

    ws::publish(campaign_id, json!({
        "type": "combatant_spell_cast",
        "caster_id": caster_id,
        "spell_slug": body.spell_slug,
        "spell_name": spell_name,
        "targets": results.iter().map(|r| json!({
            "target_id": r.target_id,
            "damage": r.damage_applied,
            "hp_after": r.hp_after,
            "save_passed": r.save_passed,
            "concentration_broken": r.concentration_broken,
        })).collect::<Vec<_>>(),
    }).to_string());

    Ok(Json(CastSpellResult {
        spell_name,
        spell_level,
        caster_id,
        slot_level_consumed: slot_level,
        targets: results,
        overlay_created: overlay_id,
        concentration_required,
    }))
}
