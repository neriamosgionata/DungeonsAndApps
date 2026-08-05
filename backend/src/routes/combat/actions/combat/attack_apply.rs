// Attack outcome application: action consume, ammo, HP update, combat event, ws publish.
use super::*;
use super::ammo::{decrement_ammo, decrement_thrown_weapon};
use super::super::sync_combatant_hp_to_sheet;
use super::super::auto_trigger_ready_actions_for_event;
use crate::AppState;
use serde_json::json;
use uuid::Uuid;

/// Apply attack outcome: action consume, ammo, HP update, combat event,
/// reaction window, ws publish.
#[allow(clippy::too_many_arguments)]
pub async fn apply_attack_outcome(
    s: &AppState,
    attacker_snap: &combat_engine::CombatantSnapshot,
    target_snap: &combat_engine::CombatantSnapshot,
    weapon: Option<(serde_json::Value, combat_engine::WeaponProps)>,
    attacker_id: Uuid,
    target_id: Uuid,
    skip_ammo: bool,
    result: &mut combat_engine::AttackResult,
    campaign_id: Uuid,
    is_reckless: bool,
    req: &combat_engine::AttackReq,
    bonus_action_attack: bool,
) -> AppResult<()> {
    let (round, turn_index): (i32, i32) =
        sqlx::query_as("select round, turn_index from encounters where id = $1")
            .bind(attacker_snap.encounter_id)
            .fetch_one(&s.db)
            .await?;

    let mut tx = s.db.begin().await?;

    // A13: GWM bonus attack — a follow-up weapon attack made via bonus
    // action after a crit/kill (PHB p.167). Consumes the bonus action and
    // the granted flag instead of the Attack action.
    let mut extra_attack: i32 = 1;
    let mut attacks_made: i32 = 1;
    if bonus_action_attack {
        let flag: bool = sqlx::query_scalar(
            "select gwm_bonus_attack_available from combatants where id = $1 for update",
        )
        .bind(attacker_id)
        .fetch_one(&mut *tx)
        .await?;
        if !flag {
            return Err(AppError::BadRequest(
                "GWM bonus attack not available — requires a critical hit or a kill this turn".into(),
            ));
        }
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true, gwm_bonus_attack_available = false where id = $1 and bonus_action_used = false returning id")
            .bind(attacker_id).fetch_optional(&mut *tx).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        // A1: Extra Attack (PHB p.72) — characters with the feature may make
        // multiple weapon attacks with one Attack action. The FIRST attack
        // consumes the action (existing atomic check); follow-ups only bump
        // attacks_made_this_turn. Non-feature attackers keep the legacy
        // always-consume behavior. Counter resets at turn start.
        extra_attack = combat_engine::extra_attack_count(attacker_snap);
        attacks_made = 0;
        if extra_attack >= 2 {
            let made: i32 = sqlx::query_scalar(
                "select attacks_made_this_turn from combatants where id = $1 for update",
            )
            .bind(attacker_id)
            .fetch_one(&mut *tx)
            .await?;
            if made >= extra_attack {
                return Err(AppError::BadRequest(format!(
                    "no attacks remaining (Extra Attack allows {extra_attack} per Attack action)"
                )));
            }
            attacks_made = made + 1;
            sqlx::query("update combatants set attacks_made_this_turn = $2 where id = $1")
                .bind(attacker_id)
                .bind(attacks_made)
                .execute(&mut *tx)
                .await?;
        }

        let action_consumed: Option<Uuid> = if extra_attack >= 2 && attacks_made > 1 {
            // Follow-up attack — the action was consumed by the first attack.
            Some(attacker_id)
        } else {
            sqlx::query_scalar(
                "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
                .bind(attacker_id).fetch_optional(&mut *tx).await?
        };
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
        // A2: Precision Attack consumed the superiority die (rolled by the
        // resolver) — spend it in the same tx so a depleted pool rolls back
        // the whole attack.
        if req.precision_superiority {
            if let Some(chid) = attacker_snap.character_id {
                let fighter_level: i32 = attacker_snap.classes.as_array().map(|arr| {
                    arr.iter()
                        .filter(|c| {
                            c.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n.eq_ignore_ascii_case("fighter"))
                                .unwrap_or(false)
                        })
                        .filter_map(|c| c.get("level").and_then(|l| l.as_i64()))
                        .sum::<i64>() as i32
                }).unwrap_or(0);
                crate::routes::combat::special::consume_superiority_die(
                    &mut *tx,
                    chid,
                    fighter_level,
                )
                .await?;
            }
        }
    }

    let ammo_info: Option<(String, i32)> = if skip_ammo {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("ammunition") || props.to_lowercase().contains("ammo") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_ammo(&mut *tx, chid, wname).await?
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    let thrown_info: Option<(String, i32)> = if skip_ammo {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("thrown") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_thrown_weapon(&mut *tx, chid, wname).await?
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    if result.sneak_attack_applied {
        sqlx::query(
            "update combatants set sneak_attack_used_this_turn = true where id = $1",
        )
        .bind(attacker_id)
        .execute(&mut *tx)
        .await?;
    }

    // Divine Smite: consume spell slot atomically
    if result.smite_applied {
        if let Some(chid) = attacker_snap.character_id {
            if let Some(slot_level) = result.smite_slot_consumed {
                sqlx::query("select id from characters where id = $1 for update")
                    .bind(chid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .ok_or(AppError::NotFound)?;
                let slot_key = format!("{}", slot_level);
                let slot_current: Option<i32> = sqlx::query_scalar(
                    "select (sheet->'slots'->$1->>'current')::int from characters where id = $2",
                )
                .bind(&slot_key)
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .flatten();
                let cur = slot_current.ok_or(AppError::BadRequest(
                    "spell slot not found on character sheet".into(),
                ))?;
                if cur <= 0 {
                    return Err(AppError::BadRequest(
                        "spell slot depleted before smite could consume it".into(),
                    ));
                }
                sqlx::query(
                    "update characters set sheet = jsonb_set(sheet, array['slots', $1, 'current'], to_jsonb($2::int)) where id = $3",
                )
                .bind(&slot_key)
                .bind(cur - 1)
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    if result.hit {
        // H-23: the resolver ran against a pre-tx snapshot — two concurrent
        // attacks on the same target both computed from stale HP and the
        // unconditional UPDATE made the last commit win, silently dropping
        // the first hit's damage. Re-read the target under a row lock and
        // re-apply THIS hit's damage delta to the fresh state.
        let (fresh_hp, fresh_temp): (i32, i32) = sqlx::query_as(
            "select hp_current, temp_hp from combatants where id = $1 for update",
        )
        .bind(target_id)
        .fetch_one(&mut *tx)
        .await?;
        let hit_delta = result.damage_applied
            + result.extra_damage_applied
            + result.sneak_attack_damage
            + result.smite_damage;
        let (new_target_hp, new_target_temp) =
            combat_engine::apply_hp_damage(fresh_hp, fresh_temp, hit_delta);
        if !result.instant_death
            && fresh_hp > 0
            && (hit_delta - fresh_hp - fresh_temp).max(0) >= target_snap.hp_max
        {
            result.instant_death = true;
        }
        result.target_hp_after = new_target_hp;
        result.target_temp_hp_after = new_target_temp;

        // H-7: record the hit's side effects in the pending_hits entry so a
        // later reaction negation (Shield/Parry/Deflect-to-0/Protection) can
        // unwind exactly what this hit committed.
        // M-13: PHB p.197 — any critical hit at 0 HP causes 2 failures (the
        // melee-only rule is the 5-ft auto-crit, not the failure count).
        let fail_inc: i32 = if !result.instant_death
            && fresh_hp <= 0
            && new_target_hp <= 0
        {
            if result.critical { 2 } else { 1 }
        } else {
            0
        };
        sqlx::query(
            "update combatants set
                last_hit_attack_total = $1,
                last_hit_damage = $2,
                pending_hits = pending_hits || jsonb_build_array(jsonb_build_object(
                    'attacker_id', $3,
                    'attack_total', $1,
                    'damage', $2,
                    'round', $5,
                    'hp_before', $6,
                    'hp_after', $7,
                    'natural_roll', $8,
                    'bonus', $9,
                    'temp_before', $10,
                    'temp_after', $11,
                    'death_failures', $12,
                    'alive_set_false', $13,
                    'concentration_broken', $14
                ))
             where id = $4",
        )
        .bind(result.attack_total)
        .bind(
            result.damage_applied
                + result.extra_damage_applied
                + result.sneak_attack_damage
                + result.smite_damage,
        )
        .bind(attacker_id)
        .bind(target_id)
        .bind(round)
        .bind(fresh_hp)
        .bind(new_target_hp)
        .bind(result.natural_roll)
        .bind(result.attack_total - result.natural_roll)
        .bind(target_snap.temp_hp)
        .bind(result.target_temp_hp_after)
        .bind(fail_inc)
        .bind(result.instant_death)
        .bind(result.concentration_broken)
        .execute(&mut *tx)
        .await?;

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(target_id)
            .execute(&mut *tx)
            .await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(target_id)
                .execute(&mut *tx).await?;
        }

        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#,
                )
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            }
        } else if fail_inc > 0 && let Some(chid) = target_snap.character_id {
            // MED-7 + M-13: PHB p.197 — damage at 0 HP = 1 death-save failure;
            // any critical hit = 2 failures.
            sqlx::query(
                r#"update characters set sheet =
                    coalesce(sheet, '{}'::jsonb)
                    || jsonb_build_object(
                        'death_saves', jsonb_build_object(
                            'successes', coalesce((sheet->'death_saves'->>'successes')::int, 0),
                            'failures', least(3,
                                coalesce((sheet->'death_saves'->>'failures')::int, 0) + $2
                            )
                        )
                    )
                   where id = $1"#,
            )
            .bind(chid)
            .bind(fail_inc)
            .execute(&mut *tx)
            .await?;
        }
    }

    // Stunning Strike: PHB p.79 — on hit, monk can spend 1 Ki to force
    // target CON save vs Ki save DC (8 + prof + WIS mod). On fail: stunned
    // until end of monk's next day. Implemented as 1-round timed condition.
    if req.stunning_strike && result.hit {
        let chid = attacker_snap.character_id;
        if let Some(chid) = chid {
            // Lock character row for atomic Ki consumption
            sqlx::query("select id from characters where id = $1 for update")
                .bind(chid)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(AppError::NotFound)?;
            let monk_level: Option<i32> = sqlx::query_scalar(
                r#"select (elem->>'level')::int
                   from characters, jsonb_array_elements(sheet->'classes') as elem
                   where id = $1 and lower(elem->>'name') = 'monk'
                   limit 1"#,
            )
            .bind(chid)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
            if let Some(monk_level) = monk_level {
                if monk_level >= 5 {
                    // Check Ki resource
                    let ki: Option<(i32, i32)> = sqlx::query_as(
                        r#"select (elem->>'current')::int, (elem->>'max')::int
                           from characters, jsonb_array_elements(sheet->'resources') as elem
                           where id = $1 and lower(elem->>'name') = 'ki'
                           limit 1"#,
                    )
                    .bind(chid)
                    .fetch_optional(&mut *tx)
                    .await?
                    .map(|(c, m): (Option<i32>, Option<i32>)| {
                        (c.unwrap_or(0), m.unwrap_or(0))
                    });
                    if let Some((ki_cur, _ki_max)) = ki {
                        if ki_cur >= 1 {
                            // Consume 1 Ki from the character sheet resources
                            let idx: i32 = sqlx::query_scalar(
                                r#"select position - 1
                                   from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                                   where id = $1 and lower(t.elem->>'name') = 'ki'
                                   limit 1"#,
                            )
                            .bind(chid)
                            .fetch_optional(&mut *tx)
                            .await?
                            .ok_or(AppError::BadRequest(
                                "no Ki resource found on character sheet".into(),
                            ))?;
                            sqlx::query(
                                r#"update characters set sheet = jsonb_set(
                                     sheet, ('{resources,' || $2 || ',current}')::text[],
                                     to_jsonb($3::int)
                                   ) where id = $1"#,
                            )
                            .bind(chid)
                            .bind(idx)
                            .bind(ki_cur - 1)
                            .execute(&mut *tx)
                            .await?;
                            // Compute Ki save DC: 8 + prof + WIS mod
                            let pb = if attacker_snap.proficiency_bonus > 0 {
                                attacker_snap.proficiency_bonus
                            } else {
                                combat_engine::proficiency_from_level(attacker_snap.level_total)
                            };
                            let wis_mod = combat_engine::ability_mod(&attacker_snap, "wis");
                            let dc = 8 + pb + wis_mod;
                            // Force CON save on target
                            let target_save_stats =
                                combat_engine::compute_stats(target_snap);
                            // M21: Aura of Protection — CHA mod of paladin
                            // allies near this target.
                            let aura = super::super::super::aura::aura_of_protection_bonus(
                                &s.db, target_id, target_snap.encounter_id,
                                target_snap.token_x, target_snap.token_y,
                            )
                            .await
                            .unwrap_or(0);
                            let save_req = combat_engine::SaveReq {
                                ability: "con".into(),
                                dc,
                                advantage: false,
                                disadvantage: false,
                                label: Some("Stunning Strike".into()),
                                is_magical: Some(false),
                                aura_bonus: Some(aura),
                            };
                            let save_result = combat_engine::resolve_save(
                                target_snap,
                                &save_req,
                                &target_save_stats,
                            );
                            let save_passed = save_result
                                .as_ref()
                                .map(|r| r.passed)
                                .unwrap_or(true);
                            if !save_passed {
                                // Apply stunned condition (1 round duration)
                                let existing: Vec<String> = sqlx::query_scalar(
                                    "select conditions from combatants where id = $1",
                                )
                                .bind(target_id)
                                .fetch_optional(&mut *tx)
                                .await?
                                .unwrap_or_default();
                                let has_stunned = existing.iter().any(|c| {
                                    c.split(':')
                                        .next()
                                        .unwrap_or(c)
                                        .to_lowercase()
                                        == "stunned"
                                });
                                if !has_stunned {
                                    let mut new_conds = existing.clone();
                                    new_conds.push("stunned:1".into());
                                    sqlx::query(
                                        "update combatants set conditions = $1 where id = $2",
                                    )
                                    .bind(&new_conds)
                                    .bind(target_id)
                                    .execute(&mut *tx)
                                    .await?;
                                }
                            }
                            result.stunning_strike_applied = true;
                            result.stunning_strike_save_passed = Some(!save_passed);
                        }
                    }
                }
            }
        }
    }

    if is_reckless {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
               values ($1, 'Reckless Attack', 'debuff', 'swords', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"attack_advantage_against": true}', 'ability', $2, $3)"#)
            .bind(attacker_id)
            .bind(round)
            .bind(turn_index)
            .execute(&mut *tx).await?;
    }

    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'",
    )
    .bind(attacker_id)
    .execute(&mut *tx)
    .await?;

    let total_dmg = result.damage_applied + result.extra_damage_applied + result.sneak_attack_damage + result.smite_damage;
    let event_action = if result.hit {
        let death_note = if result.instant_death {
            " — INSTANT DEATH"
        } else {
            ""
        };
        format!(
            "{} attacked {}: {} damage{}",
            attacker_snap.display_name, target_snap.display_name, total_dmg, death_note
        )
    } else {
        format!(
            "{} attacked {}: missed ({} vs AC {})",
            attacker_snap.display_name,
            target_snap.display_name,
            result.attack_total,
            result.target_ac
        )
    };
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(attacker_id)
        .bind(target_id)
        .bind(&event_action)
        .bind(if result.hit { -total_dmg } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    // A13: grant the GWM bonus attack when a GWM melee attack crits or
    // kills (PHB p.167). Consumed by the next attack with
    // bonus_action_attack = true.
    let attacker_stats_for_gwm = combat_engine::compute_stats(attacker_snap);
    let gwm_granted = !bonus_action_attack
        && attacker_stats_for_gwm.great_weapon_master
        && (result.critical || result.target_hp_after <= 0)
        && weapon.as_ref().map(|(w, _)| {
            let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
            let p = props.to_lowercase();
            !p.contains("ranged") && !p.contains("thrown")
        }).unwrap_or(false);
    if gwm_granted {
        sqlx::query("update combatants set gwm_bonus_attack_available = true where id = $1")
            .bind(attacker_id)
            .execute(&mut *tx)
            .await?;
    }
    result.gwm_bonus_attack_available = gwm_granted;

    // A17: a dead mount dismounts its rider.
    if result.target_hp_after <= 0 {
        super::super::dismount_riders(&mut *tx, target_id).await?;
    }

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(
            &s.db,
            target_id,
            result.target_hp_after,
            result.target_temp_hp_after,
        )
        .await
        {
            tracing::error!(combatant_id = %target_id, "sync sheet HP: {e}");
        }
        // M-WS4: drop damage_pending from the public event. It tells all
        // members the incoming damage of any other player, which is intel
        // ("A is about to take 24 damage from B's hit"). The target
        // already gets the full AttackResult via the HTTP response, and
        // the actual damage lands in the combatant_damages event.
        // M-F6 part 2: persist for replay.
        ws::publish_persist(
            &s.db,
            campaign_id,
            json!({
                "type": "reaction_window",
                "window_type": "hit_before_damage",
                "target_id": target_id,
                "attacker_id": attacker_id,
                "attack_total": result.attack_total,
                "target_ac": result.target_ac,
            }),
        )
        .await;
        auto_trigger_ready_actions_for_event(
            &s.db,
            campaign_id,
            attacker_snap.encounter_id,
            "target_attacks",
            attacker_id,
            target_id,
        )
        .await;
    }

    // M-F6 part 2: persist for replay.
    ws::publish_persist(&s.db, campaign_id, json!({
        "type": "combatant_attacks",
        "attacker_id": attacker_id,
        "target_id": target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "extra_damage": if result.hit && result.extra_damage_applied > 0 { Some(result.extra_damage_applied) } else { None },
        "extra_damage_type": result.extra_damage_type.as_deref(),
        "sneak_attack": if result.hit && result.sneak_attack_applied { Some(result.sneak_attack_damage) } else { None },
        "smite": if result.hit && result.smite_applied { Some(result.smite_damage) } else { None },
        // MED-12: drop hp_after/temp_hp_after — was leaking HP of hidden
        // enemies to non-owner clients. Frontend re-fetches via the masked
        // /combatants list endpoint.
        "concentration_breaks": if result.hit { Some(result.concentration_broken) } else { None },
        "instant_death": if result.hit { Some(result.instant_death) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
        "ammo_consumed": ammo_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
        "thrown_consumed": thrown_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
    })).await;

    // A1: surface attacks remaining for the Extra Attack UI.
    result.attacks_remaining = if extra_attack >= 2 {
        Some(extra_attack - attacks_made)
    } else {
        None
    };

    Ok(())
}
