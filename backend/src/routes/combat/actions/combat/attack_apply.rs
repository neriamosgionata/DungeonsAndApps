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
    result: &combat_engine::AttackResult,
    campaign_id: Uuid,
    is_reckless: bool,
    req: &combat_engine::AttackReq,
) -> AppResult<()> {
    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false and hp_current > 0 returning id")
        .bind(attacker_id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
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

    if result.hit {
        sqlx::query(
            "update combatants set
                last_hit_attack_total = $1,
                last_hit_damage = $2,
                pending_hits = pending_hits || jsonb_build_array(jsonb_build_object(
                    'attacker_id', $3,
                    'attack_total', $1,
                    'damage', $2,
                    'round', $5
                ))
             where id = $4",
        )
        .bind(result.attack_total)
        .bind(result.damage_applied + result.extra_damage_applied)
        .bind(attacker_id)
        .bind(target_id)
        .bind(round)
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
        } else if target_snap.hp_current <= 0
            && result.target_hp_after <= 0
            && let Some(chid) = target_snap.character_id
        {
            // MED-7: PHB p.197 — damage at 0 HP = 1 death-save failure.
            // Melee crit within 5ft = 2 failures (PHB "critical hit against
            // a downed creature within 5ft"). The weapon (if any) tells us
            // melee vs ranged/thrown; for `None` weapon (unarmed) treat as
            // melee.
            let is_melee = weapon
                .as_ref()
                .map(|(_, p)| !p.ranged && !p.thrown)
                .unwrap_or(true);
            let fail_inc: i32 = if result.critical && is_melee { 2 } else { 1 };
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

    if is_reckless {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type)
               values ($1, 'Reckless Attack', 'debuff', 'swords', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"attack_advantage_against": true}', 'ability')"#)
            .bind(attacker_id)
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

    let total_dmg = result.damage_applied + result.extra_damage_applied;
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
        ws::publish(
            campaign_id,
            json!({
                "type": "reaction_window",
                "window_type": "hit_before_damage",
                "target_id": target_id,
                "attacker_id": attacker_id,
                "attack_total": result.attack_total,
                "target_ac": result.target_ac,
            })
            .to_string(),
        );
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

    ws::publish(campaign_id, json!({
        "type": "combatant_attacks",
        "attacker_id": attacker_id,
        "target_id": target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "extra_damage": if result.hit && result.extra_damage_applied > 0 { Some(result.extra_damage_applied) } else { None },
        "extra_damage_type": result.extra_damage_type.as_deref(),
        // MED-12: drop hp_after/temp_hp_after — was leaking HP of hidden
        // enemies to non-owner clients. Frontend re-fetches via the masked
        // /combatants list endpoint.
        "concentration_breaks": if result.hit { Some(result.concentration_broken) } else { None },
        "instant_death": if result.hit { Some(result.instant_death) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
        "ammo_consumed": ammo_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
        "thrown_consumed": thrown_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
    }).to_string());

    Ok(())
}
