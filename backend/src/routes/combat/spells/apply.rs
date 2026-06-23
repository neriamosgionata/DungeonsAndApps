// apply_spell_outcome — tx + post-tx (action consume, slot decrement, effect insert,
// HP update, concentration, AoE overlay, ws publish, auto-trigger).
use super::cast::CastSpellTargetResult;
use crate::{
    combat_engine,
    error::{AppError, AppResult}, ws,
    AppState,
};
use serde_json::json;
use uuid::Uuid;

#[allow(clippy::too_many_arguments)]
pub async fn apply_spell_outcome(
    s: &AppState,
    body: &super::cast::CastSpellBody,
    caster_id: Uuid,
    caster_snap: &combat_engine::CombatantSnapshot,
    campaign_id: Uuid,
    spell_name: &str,
    spell_level: i32,
    slot_level: i32,
    is_bonus_action: bool,
    concentration_required: bool,
    cast_as_ritual: bool,
    template_arr: &[serde_json::Value],
    results: &[CastSpellTargetResult],
    aoe_template: Option<&serde_json::Value>,
    round: i32,
    turn_index: i32,
    overlay_id: &mut Option<Uuid>,
) -> AppResult<()> {
    let mut tx = s.db.begin().await?;

    // F3: track which combatants had effects changed in this tx, so we can
    // emit one `effects_change` WS event per affected combatant after commit.
    // The frontend's `loadEffects()` is gated on this event (initiative/+page.svelte:509-511);
    // without it, the new effect doesn't show up until the next unrelated event.
    let mut effects_changed: std::collections::HashSet<Uuid> = std::collections::HashSet::new();

    let (prev_action_spell_level, prev_bonus_spell_level): (i16, i16) = sqlx::query_as(
        "select action_spell_level, bonus_action_spell_level from combatants where id = $1",
    )
    .bind(caster_id)
    .fetch_one(&mut *tx)
    .await?;
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
        .bind(&body.spell_slug)
        .bind(caster_id)
        .execute(&mut *tx)
        .await?;

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
        let msg = if is_bonus_action {
            "bonus action already used"
        } else {
            "action already used"
        };
        return Err(AppError::BadRequest(msg.into()));
    }

    if !cast_as_ritual && slot_level > 0 {
        if let Some(chid) = caster_snap.character_id {
            // Lock the character row so concurrent casts from the same caster
            // can't both read the slot as available and double-decrement.
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
            .await?;
            if let Some(current) = slot_current {
                if current <= 0 {
                    return Err(AppError::BadRequest(
                        "no spell slots of that level remaining".into(),
                    ));
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
        effects_changed.insert(caster_id);
    }

    for result in results {
        let target_id = result.target_id;

        for t in template_arr {
            if t.get("aoe").is_some() {
                continue;
            }
            effects_changed.insert(target_id);

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
            .bind(target_id).bind(&name).bind(&kind).bind(&icon).bind(&duration_unit)
            .bind(duration_value).bind(duration_value).bind(&tick_trigger).bind(conc)
            .bind(caster_id).bind(spell_name).bind(&body.spell_slug).bind(modifiers)
            .bind(round).bind(turn_index).execute(&mut *tx).await?;
        }

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.hp_after).bind(result.temp_hp_after).bind(target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(target_id).execute(&mut *tx).await?;
            effects_changed.insert(target_id);
        }

        super::super::actions::sync_combatant_hp_to_sheet_tx(
            &mut *tx,
            target_id,
            result.hp_after,
            result.temp_hp_after,
        )
        .await?;
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
            .bind(caster_snap.encounter_id).bind(shape).bind(radius_ft).bind(length_ft)
            .bind(width_ft).bind(color).bind(spell_name).bind(round + aoe_duration)
            .bind(&body.spell_slug).bind(caster_id)
            .fetch_one(&mut *tx).await?;
            *overlay_id = Some(oid);
        }
    }

    tx.commit().await?;

    ws::publish(
        campaign_id,
        json!({
            "type": "reaction_window",
            "window_type": "spell_being_cast",
            "caster_id": caster_id,
            "spell_slug": body.spell_slug,
            "spell_level": spell_level,
            "slot_level": slot_level,
        })
        .to_string(),
    );

    // Idempotent post-commit clear (HIGH-1). `where spell_being_cast is not null`
    // makes the clear safe under concurrent Counterspell (which already nulled it).
    // Retry once on transient DB error; the next cast_spell will overwrite any stuck
    // value, so a permanent failure is self-healing and only logged.
    let mut clear_attempt = 0u8;
    loop {
        match sqlx::query(
            "update combatants set spell_being_cast = null where id = $1 and spell_being_cast is not null",
        )
        .bind(caster_id)
        .execute(&s.db)
        .await
        {
            Ok(_) => break,
            Err(e) if clear_attempt < 1 => {
                clear_attempt += 1;
                tracing::warn!(caster_id = %caster_id, "post-commit clear spell_being_cast retry: {e}");
            }
            Err(e) => {
                tracing::error!(caster_id = %caster_id, "post-commit clear spell_being_cast failed: {e}");
                break;
            }
        }
    }

    super::super::actions::auto_trigger_ready_actions_for_event(
        &s.db,
        campaign_id,
        caster_snap.encounter_id,
        "target_casts",
        caster_id,
        caster_id,
    )
    .await;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_casts_spell",
            "caster_id": caster_id,
            "spell_slug": body.spell_slug,
            "spell_name": spell_name,
            "targets": results.iter().map(|r| json!({
                "target_id": r.target_id,
                "damage": r.damage_applied,
                // L6: drop hp_after (M12 visibility leak).
                "save_passed": r.save_passed,
                "concentration_breaks": r.concentration_broken,
            })).collect::<Vec<_>>(),
        })
        .to_string(),
    );

    // F3: emit one `effects_change` per combatant whose effects were modified
    // in this tx (template inserts, concentration clear, target concentration
    // break). The frontend listens to this event to reload the effect list.
    for cid in effects_changed {
        ws::publish(
            campaign_id,
            json!({
                "type": "effects_change",
                "combatant_id": cid,
            })
            .to_string(),
        );
    }

    Ok(())
}
