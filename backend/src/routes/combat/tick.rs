// Per-turn combatant effect tick: round_end, target_turn_{end,start}, caster_turn_{end,start},
// surprised block, hazard zones, regen, timed-condition countdown, effect/overlay expiry.
use super::helpers::{has_condition, remove_condition};
use crate::combat_engine;
use anyhow::Result;
use rand::SeedableRng;
use serde_json::json;
use uuid::Uuid;

/// Tick down `name:N` conditions. Returns the new condition list and a flag
/// indicating whether anything changed. Pure function — used by `tick_effects`
/// at the new combatant's `target_turn_start`.
pub(crate) fn tick_conditions(conditions: Vec<String>) -> (Vec<String>, bool) {
    let mut changed = false;
    let new: Vec<String> = conditions
        .into_iter()
        .filter_map(|c| {
            if let Some(idx) = c.rfind(':') {
                let (name, num_str) = c.split_at(idx);
                if let Ok(n) = num_str[1..].parse::<i32>() {
                    if n <= 1 {
                        changed = true;
                        return None;
                    }
                    changed = true;
                    return Some(format!("{}:{}", name, n - 1));
                }
            }
            Some(c)
        })
        .collect();
    (new, changed)
}

/// C-2: consume a surprised combatant's first turn: full economy blocked
/// (action, bonus action, movement) AND `reaction_used = true` — PHB p.189
/// "you can't take a reaction until that turn ends". Also removes the
/// `surprised` condition. Returns true if the condition was present.
pub(crate) async fn consume_surprise(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    cid: Uuid,
) -> Result<bool> {
    let consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants
            set action_used = true,
                bonus_action_used = true,
                movement_used_ft = 9999,
                reaction_used = true
          where id = $1 and 'surprised' = any(conditions)
          returning id",
    )
    .bind(cid)
    .fetch_optional(&mut **tx)
    .await?;
    if consumed.is_none() {
        return Ok(false);
    }
    let conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
        .bind(cid)
        .fetch_one(&mut **tx)
        .await?;
    let new_conds = remove_condition(conditions, "surprised");
    sqlx::query("update combatants set conditions = $1 where id = $2")
        .bind(&new_conds)
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
pub async fn tick_effects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    db: &sqlx::PgPool,
    encounter_id: Uuid,
    old_round: i32,
    old_turn: i32,
    new_round: i32,
    new_turn: i32,
) -> Result<Vec<String>> {
    let mut events: Vec<String> = Vec::new();

    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(encounter_id)
        .fetch_all(&mut **tx).await?;

    if combatants.is_empty() {
        return Ok(events);
    }

    // H-15: only run the tick pipeline on FORWARD transitions. prev_turn /
    // goto_turn backward jumps previously re-ran hazard damage, regen and
    // condition countdowns against the jumped-back-to combatant — a
    // "undo misclick" that damaged the token twice.
    let forward = new_round > old_round || (new_round == old_round && new_turn > old_turn);
    if !forward {
        return Ok(events);
    }

    let _max_turn = (combatants.len() as i32) - 1;

    fn cid_at(turn: i32, list: &[(i32, Uuid)]) -> Option<Uuid> {
        list.iter().find(|(t, _)| *t == turn).map(|(_, id)| *id)
    }

    if new_round > old_round {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'round_end' and remaining is not null
               and combatant_id in (select id from combatants where encounter_id = $1)",
        )
        .bind(encounter_id)
        .execute(&mut **tx)
        .await?;
    }

    let ended_turn = old_turn;
    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_end' and remaining is not null
               and combatant_id = $1",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }

    let started_turn = new_turn;
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_start' and remaining is not null
                and combatant_id = $1",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }

    // A4 + M-31: PHB p.48 — rage ends if the barbarian is knocked
    // unconscious. The condition must go too (pre-fix a revived barbarian
    // kept the `rage` condition forever, blocking casting).
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set active = false
             where combatant_id = $1 and name = 'Rage' and active = true
               and exists (select 1 from combatants c where c.id = $1 and c.hp_current <= 0)",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "update combatants set conditions = array_remove(conditions, 'rage')
             where id = $1 and 'rage' = any(conditions)
               and hp_current <= 0",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }

    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_end' and remaining is not null
               and caster_combatant_id = $1",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }

    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_start' and remaining is not null
               and caster_combatant_id = $1",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await?;
    }

    let expired_effects: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "select id, combatant_id from combatant_effects
         where active = true and remaining is not null and remaining <= 0
           and combatant_id in (select id from combatants where encounter_id = $1)",
    )
    .bind(encounter_id)
    .fetch_all(&mut **tx)
    .await?;

    if !expired_effects.is_empty() {
        sqlx::query(
            "update combatant_effects set active = false
             where active = true and remaining is not null and remaining <= 0
               and combatant_id in (select id from combatants where encounter_id = $1)",
        )
        .bind(encounter_id)
        .execute(&mut **tx)
        .await?;
        for (_, combatant_id) in &expired_effects {
            events.push(
                json!({
                    "type": "effects_change",
                    "combatant_id": combatant_id
                })
                .to_string(),
            );
        }
    }

    let expired_overlays: Vec<Uuid> = sqlx::query_scalar(
        "select id from encounter_overlays
         where active = true and encounter_id = $1
           and (expires_at_round is not null and expires_at_round < $2
                or (expires_at_round = $2 and expires_at_turn is not null and expires_at_turn < $3))")
        .bind(encounter_id).bind(new_round).bind(new_turn)
        .fetch_all(&mut **tx).await?;

    if !expired_overlays.is_empty() {
        sqlx::query(
            "update encounter_overlays set active = false
             where active = true and encounter_id = $1
               and (expires_at_round is not null and expires_at_round < $2
                    or (expires_at_round = $2 and expires_at_turn is not null and expires_at_turn < $3))")
            .bind(encounter_id).bind(new_round).bind(new_turn)
            .execute(&mut **tx).await?;
        events.push(
            json!({
                "type": "overlays_expire",
                "ids": expired_overlays
            })
            .to_string(),
        );
    }

    if let Some(cid) = cid_at(new_turn, &combatants) {
        // L-P2: single SELECT for all fields we need from the active
        // combatant. Pre-fix had 3 separate SELECTs against the same
        // row (conditions+hp_max, token_x+y, then hp_current+max+temp_hp
        // re-fetched inside the per-hazard loop). 5 hazards × 1 extra
        // SELECT each = 5 wasted round-trips per turn transition.
        let snap: Option<(
            Vec<String>,
            i32,
            i32,
            i32,
            Option<f64>,
            Option<f64>,
        )> = sqlx::query_as(
            "select conditions, hp_current, hp_max, temp_hp,
                    token_x::float8, token_y::float8
             from combatants where id = $1",
        )
        .bind(cid)
        .fetch_optional(&mut **tx)
        .await?;
        let (conditions, mut hp_current, hp_max, mut hp_temp, combatant_pos) = match snap {
            Some(s) => (s.0, s.1, s.2, s.3, (s.4, s.5)),
            None => return Ok(events),
        };
        // H6: exhaustion 6 = dead (PHB p.291). Dead combatants get no turn:
        // no hazard damage, no regen, no surprised handling, no conditions tick.
        let snap = combat_engine::load_snapshot(db, cid)
            .await
            .map_err(|e| anyhow::anyhow!("load dead-check snapshot: {e}"))?;
        let stats = combat_engine::compute_stats(&snap);
        if stats.exhaustion_dead {
            return Ok(events);
        }
        let is_surprised = has_condition(&conditions, "surprised");
        if is_surprised {
            // MED-10 + C-2: atomic check-and-set via the shared helper —
            // consumes action/BA/movement AND the reaction (PHB p.189),
            // removes the condition. Runs only on forward turn transitions
            // (backward jumps early-return above).
            if consume_surprise(tx, cid).await? {
                events.push(
                    json!({
                        "type": "combatant_is_surprised",
                        "combatant_id": cid,
                    })
                    .to_string(),
                );
            }
        }

        let (cx, cy) = match combatant_pos {
            (Some(x), Some(y)) => (x, y),
            _ => (0.0, 0.0),
        };
        let hazards: Vec<(
            String,
            f64,
            f64,
            Option<i32>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i32>,
            bool,
            Option<f64>,
            Option<f64>,
            Option<i32>,
            Option<i32>,
        )> = sqlx::query_as(
            r#"select shape, origin_x, origin_y, radius_ft,
                      hazard_damage_expression, hazard_damage_type,
                      hazard_save_ability, hazard_save_dc, hazard_half_on_save,
                      end_x, end_y, width_ft, length_ft
               from encounter_overlays
               where encounter_id = $1 and active = true
                 and zone_type = 'hazard'
                 and hazard_damage_expression is not null"#,
        )
        .bind(encounter_id)
        .fetch_all(&mut **tx)
        .await?;

        for (
            shape,
            ox,
            oy,
            rad,
            dmg_expr,
            dmg_type,
            save_ability,
            save_dc,
            half_on_save,
            end_x,
            end_y,
            width_ft,
            length_ft,
        ) in hazards
        {
            // MED-9: rad is in feet, distance in % of map. 1 cell = 5ft
            // = 20%, so 1ft = 4%. Pre-fix used rad as % directly (~4× too big).
            let r = rad.unwrap_or(20) as f64 * 4.0;
            // H-18: cones and lines resolve with their real geometry — the
            // old `_ => circle` fallback turned a 10-ft cone into a 40%-of-map
            // circle and a wall line into a circle too.
            let in_zone = match shape.as_str() {
                "circle" => {
                    let dx = cx - ox;
                    let dy = cy - oy;
                    (dx * dx + dy * dy).sqrt() <= r
                }
                "cube" | "square" => (cx - ox).abs() <= r && (cy - oy).abs() <= r,
                "cone" => {
                    // Axis origin→end (default: straight right); 5e cone
                    // apex ~53° (half-angle 26.6°, tan ≈ 0.5). Point is in
                    // the cone when its projection along the axis is within
                    // [0, len] and its perpendicular offset stays under the
                    // cone's half-width at that distance.
                    let ex = end_x.unwrap_or(ox + 100.0);
                    let ey = end_y.unwrap_or(oy);
                    let ax = ex - ox;
                    let ay = ey - oy;
                    let alen = (ax * ax + ay * ay).sqrt();
                    if alen < 1e-6 {
                        false
                    } else {
                        let cone_len = length_ft.unwrap_or(rad.unwrap_or(20)) as f64 * 4.0;
                        let ux = ax / alen;
                        let uy = ay / alen;
                        let t = ((cx - ox) * ux + (cy - oy) * uy).clamp(0.0, cone_len);
                        let px = ox + ux * t;
                        let py = oy + uy * t;
                        let perp = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt();
                        perp <= t * 0.5
                    }
                }
                "line" => {
                    // Segment with thickness width_ft (default 5 ft = 20%).
                    let ex = end_x.unwrap_or(ox + 100.0);
                    let ey = end_y.unwrap_or(oy);
                    let ax = ex - ox;
                    let ay = ey - oy;
                    let alen2 = ax * ax + ay * ay;
                    if alen2 < 1e-6 {
                        false
                    } else {
                        let t = (((cx - ox) * ax + (cy - oy) * ay) / alen2).clamp(0.0, 1.0);
                        let px = ox + ax * t;
                        let py = oy + ay * t;
                        let perp = ((cx - px).powi(2) + (cy - py).powi(2)).sqrt();
                        perp <= width_ft.unwrap_or(5) as f64 * 4.0 / 2.0
                    }
                }
                _ => {
                    let dx = cx - ox;
                    let dy = cy - oy;
                    (dx * dx + dy * dy).sqrt() <= r
                }
            };
            if !in_zone {
                continue;
            }

            if let (Some(ref expr), Some(ref dtype)) = (dmg_expr, dmg_type) {
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let roll = crate::dice::roll(expr, &mut rng);
                if let Ok(roll) = roll {
                    // L-P2: use cached hp_current + temp_hp from the single
                    // SELECT above (was: re-fetched per hazard).
                    let raw_dmg = roll.total.max(0);
                    // H7: hazard save resolution — mirrors the spell path
                    // (cast.rs): apply damage type (resist/immune/vuln) first,
                    // then save → half/zero, Evasion on DEX.
                    let (eff_dmg, _, _, _) =
                        combat_engine::apply_damage_type(raw_dmg, dtype, &stats, false);
                    let mut applied = eff_dmg;
                    if let (Some(sa_raw), Some(sdc)) = (save_ability.as_deref(), save_dc) {
                        // H-17: use the full save resolver like every other
                        // save path — the inline roll missed Aura of
                        // Protection, per-ability disadvantage (restrained),
                        // auto-fail STR/DEX (paralyzed/stunned/unconscious)
                        // and advantage sources (Gnome Cunning, Danger
                        // Sense, magic resistance).
                        let aura = super::aura::aura_of_protection_bonus(
                            db,
                            cid,
                            encounter_id,
                            combatant_pos.0.map(|v| v as f32),
                            combatant_pos.1.map(|v| v as f32),
                        )
                        .await?;
                        let sr = combat_engine::resolve_save(
                            &snap,
                            &combat_engine::SaveReq {
                                ability: sa_raw.to_lowercase(),
                                dc: sdc,
                                advantage: false,
                                disadvantage: false,
                                label: None,
                                is_magical: None,
                                aura_bonus: Some(aura),
                            },
                            &stats,
                        )
                        .map_err(|e| anyhow::anyhow!("hazard save: {e}"))?;
                        let passed = sr.passed;
                        let sa = sa_raw.to_lowercase();
                        if passed {
                            applied = if stats.evasion && sa == "dex" {
                                0
                            } else if half_on_save {
                                eff_dmg / 2
                            } else {
                                0
                            };
                        } else if stats.evasion && sa == "dex" {
                            applied = eff_dmg / 2;
                        }
                    }

                    let (new_hp, new_temp) =
                        combat_engine::apply_hp_damage(hp_current, hp_temp, applied);
                    sqlx::query(
                        "update combatants set hp_current = $1, temp_hp = $2 where id = $3",
                    )
                    .bind(new_hp)
                    .bind(new_temp)
                    .bind(cid)
                    .execute(&mut **tx)
                    .await?;
                    // M-35: hazard damage forces a concentration check
                    // (PHB — any damage while concentrating).
                    if applied > 0
                        && snap.active_effects.iter().any(|e| e.concentration)
                    {
                        let (broken, _) = combat_engine::concentration_check(
                            &snap,
                            &stats,
                            applied,
                            &mut rng,
                        );
                        if broken {
                            sqlx::query(
                                "update combatant_effects set active = false
                                 where combatant_id = $1 and concentration = true and active = true",
                            )
                            .bind(cid)
                            .execute(&mut **tx)
                            .await?;
                        }
                    }
                    // Update cached values so subsequent hazards in the
                    // same loop see post-damage HP.
                    hp_current = new_hp;
                    hp_temp = new_temp;
                    events.push(
                        json!({
                            "type": "combatant_takes_hazard_damage",
                            "combatant_id": cid,
                            "damage": applied,
                            "damage_type": dtype,
                            // L7: drop hp_after (M12 visibility leak).
                        })
                        .to_string(),
                    );
                }
            }
        }

    let regen: i32 = sqlx::query_scalar(
            r#"select coalesce(sum((modifiers->>'hp_regen_per_turn')::int), 0)::int
               from combatant_effects
               where combatant_id = $1 and active = true
                 and modifiers ? 'hp_regen_per_turn'"#,
        )
        .bind(cid)
        .fetch_optional(&mut **tx)
        .await?
        .unwrap_or(0);
        if regen > 0 && hp_current > 0 {
            // exhaustion 4 halves the effective max (PHB p.291)
            let eff_max = if stats.hp_max_halved {
                hp_max / 2
            } else {
                hp_max
            };
            if hp_current < eff_max {
                let new_hp = (hp_current + regen).min(eff_max);
                sqlx::query("update combatants set hp_current = $1 where id = $2")
                    .bind(new_hp)
                    .bind(cid)
                    .execute(&mut **tx)
                    .await?;
                events.push(
                    json!({
                        "type": "combatant_regenerates",
                        "combatant_id": cid,
                        "hp_restored": regen,
                        // L8: drop hp_after (M12 visibility leak).
                    })
                    .to_string(),
                );
            }
        }

        // L-21: PHB p.197 — a dying creature makes a death saving throw at
        // the START of its turn. Pre-fix this was GM-manual only.
        if hp_current <= 0 && snap.character_id.is_some() {
            let ds_req = combat_engine::DeathSaveReq {
                advantage: false,
                disadvantage: false,
                label: None,
            };
            if let Ok(ds) = combat_engine::resolve_death_save(&snap, &ds_req) {
                if ds.hp_after != hp_current || ds.died || ds.stabilized || ds.nat20 {
                    sqlx::query("update combatants set hp_current = $1 where id = $2")
                        .bind(ds.hp_after)
                        .bind(cid)
                        .execute(&mut **tx)
                        .await?;
                    if let Some(chid) = snap.character_id {
                        sqlx::query(
                            r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                               || jsonb_build_object(
                                    'death_saves', jsonb_build_object('successes', $2::int, 'failures', $3::int),
                                    'alive', $4::bool
                                  )
                               where id = $1"#,
                        )
                        .bind(chid)
                        .bind(ds.successes_after)
                        .bind(ds.failures_after)
                        .bind(ds.alive)
                        .execute(&mut **tx)
                        .await?;
                    }
                    hp_current = ds.hp_after;
                    events.push(
                        json!({
                            "type": "combatant_death_saves",
                            "combatant_id": cid,
                            "natural_roll": ds.natural_roll,
                            "stabilized": ds.stabilized,
                            "died": ds.died,
                        })
                        .to_string(),
                    );
                }
            }
        }

        let current_conditions = if is_surprised {
            remove_condition(conditions, "surprised")
        } else {
            conditions
        };
        // M-42: hazard damage + regen wrote combatant HP but never synced
        // the character sheet — it went stale after any hazard/regen turn
        // (attack/damage/heal paths all sync).
        super::actions::sync_combatant_hp_to_sheet_batch_tx(&mut **tx, &[(cid, hp_current, hp_temp)])
            .await?;
        let (new_conditions, changed) = tick_conditions(current_conditions);
        if changed {
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conditions)
                .bind(cid)
                .execute(&mut **tx)
                .await?;
            events.push(
                json!({
                    "type": "combatant_conditions_tick",
                    "combatant_id": cid,
                    "conditions": new_conditions,
                })
                .to_string(),
            );
        }
    }

    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::tick_conditions;

    #[test]
    fn tick_conditions_decrements_n_suffix() {
        // "blinded:3" → "blinded:2"
        let (out, changed) = tick_conditions(vec!["blinded:3".into()]);
        assert!(changed);
        assert_eq!(out, vec!["blinded:2".to_string()]);
    }

    #[test]
    fn tick_conditions_removes_at_one() {
        // "blinded:1" → removed
        let (out, changed) = tick_conditions(vec!["blinded:1".into()]);
        assert!(changed);
        assert!(out.is_empty(), "condition at N=1 must be removed");
    }

    #[test]
    fn tick_conditions_preserves_bare_names() {
        // Bare condition names (no `:N` suffix) are NOT timers — preserve.
        let (out, changed) = tick_conditions(vec!["blinded".into(), "stunned".into()]);
        assert!(!changed);
        assert_eq!(out, vec!["blinded".to_string(), "stunned".to_string()]);
    }

    #[test]
    fn tick_conditions_mixed_timed_and_bare() {
        let (out, changed) = tick_conditions(vec![
            "blinded:3".into(),
            "frightened".into(),
            "charmed:1".into(),
        ]);
        assert!(changed);
        // blinded:3 → blinded:2; charmed:1 → removed; frightened preserved
        assert_eq!(
            out,
            vec!["blinded:2".to_string(), "frightened".to_string()]
        );
    }

    #[test]
    fn tick_conditions_zero_removed() {
        // Edge: "blinded:0" → removed (defensive; we shouldn't add N=0 in practice)
        let (out, changed) = tick_conditions(vec!["blinded:0".into()]);
        assert!(changed);
        assert!(out.is_empty());
    }

    #[test]
    fn tick_conditions_ignores_non_numeric_suffix() {
        // "name:foo" — colon but non-numeric, not a timer, preserve as-is.
        let (out, changed) = tick_conditions(vec!["name:foo".into()]);
        assert!(!changed);
        assert_eq!(out, vec!["name:foo".to_string()]);
    }

    #[test]
    fn tick_conditions_empty_input() {
        let (out, changed) = tick_conditions(vec![]);
        assert!(!changed);
        assert!(out.is_empty());
    }
}
