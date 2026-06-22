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

#[allow(clippy::too_many_arguments)]
pub async fn tick_effects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
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
        let (conditions, hp_current, hp_max): (Vec<String>, i32, i32) =
            sqlx::query_as("select conditions, hp_current, hp_max from combatants where id = $1")
                .bind(cid)
                .fetch_optional(&mut **tx)
                .await?
                .unwrap_or_default();
        let is_surprised = has_condition(&conditions, "surprised");
        if is_surprised {
            sqlx::query(
                "update combatants set action_used = true, bonus_action_used = true, movement_used_ft = 9999 where id = $1")
                .bind(cid).execute(&mut **tx).await?;
            let new_conds = remove_condition(conditions.clone(), "surprised");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conds)
                .bind(cid)
                .execute(&mut **tx)
                .await?;
            events.push(
                json!({
                    "type": "combatant_is_surprised",
                    "combatant_id": cid,
                })
                .to_string(),
            );
        }

        let combatant_pos: Option<(f64, f64)> =
            sqlx::query_as("select token_x, token_y from combatants where id = $1")
                .bind(cid)
                .fetch_optional(&mut **tx)
                .await?;
        if let Some((cx, cy)) = combatant_pos {
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
            )> = sqlx::query_as(
                r#"select shape, origin_x, origin_y, radius_ft,
                          hazard_damage_expression, hazard_damage_type,
                          hazard_save_ability, hazard_save_dc, hazard_half_on_save
                   from encounter_overlays
                   where encounter_id = $1 and active = true
                     and zone_type = 'hazard'
                     and hazard_damage_expression is not null"#,
            )
            .bind(encounter_id)
            .fetch_all(&mut **tx)
            .await?;

            for (shape, ox, oy, rad, dmg_expr, dmg_type, save_ability, save_dc, half_on_save) in
                hazards
            {
                let r = rad.unwrap_or(20) as f64;
                let in_zone = match shape.as_str() {
                    "circle" => {
                        let dx = cx - ox;
                        let dy = cy - oy;
                        (dx * dx + dy * dy).sqrt() <= r
                    }
                    "cube" | "square" => (cx - ox).abs() <= r && (cy - oy).abs() <= r,
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
                        let snap_hp: (i32, i32, i32) = sqlx::query_as(
                            "select hp_current, hp_max, temp_hp from combatants where id = $1",
                        )
                        .bind(cid)
                        .fetch_one(&mut **tx)
                        .await?;
                        let dmg = roll.total.max(0);
                        let _ = (save_ability, save_dc, half_on_save);

                        let (new_hp, new_temp) =
                            combat_engine::apply_hp_damage(snap_hp.0, snap_hp.2, dmg);
                        sqlx::query(
                            "update combatants set hp_current = $1, temp_hp = $2 where id = $3",
                        )
                        .bind(new_hp)
                        .bind(new_temp)
                        .bind(cid)
                        .execute(&mut **tx)
                        .await?;
                        events.push(
                            json!({
                                "type": "combatant_takes_hazard_damage",
                                "combatant_id": cid,
                                "damage": dmg,
                                "damage_type": dtype,
                                "hp_after": new_hp,
                            })
                            .to_string(),
                        );
                    }
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
        if regen > 0 && hp_current > 0 && hp_current < hp_max {
            let new_hp = (hp_current + regen).min(hp_max);
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
                    "hp_after": new_hp,
                })
                .to_string(),
            );
        }

        let current_conditions = if is_surprised {
            remove_condition(conditions, "surprised")
        } else {
            conditions
        };
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
