pub mod actions;
pub mod combatants;
pub mod encounters;
pub mod events;
pub mod special;
pub mod spells;
pub mod tactical;

use crate::{
    AppState,
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    routes::notifications::{emit, emit_campaign, NewNotif},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use serde_json::Value;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

use self::spells::cast_spell;
use self::actions::*;
use self::combatants::{
    add_combatant, bulk_add_combatants, delete_combatant, list_combatants,
    move_combatant, update_combatant, use_action, Combatant,
};
use self::events::{
    list_events, delete_event, patch_effects,
};
use self::tactical::{
    add_condition, calculate_cover, check_flanking, create_overlay, delete_overlay,
    encounter_difficulty, list_overlays, overlay_damage,
    surprise_auto, surprise_round,
};
use self::encounters::{
    list, create, read, update, delete, start, set_initiative, next_turn, prev_turn,
    goto_turn, end_encounter,
};
use self::special::{
    class_feature, grapple, grapple_escape, lair_action, legendary_action, multiattack,
    parse_multiattack, shove, stand_up, trigger_ready,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/encounters", get(list).post(create))
        .route("/encounters/{id}", get(read).patch(update).delete(delete))
        .route("/encounters/{id}/combatants", get(list_combatants).post(add_combatant))
        .route("/encounters/{id}/combatants/bulk", post(bulk_add_combatants))
        .route("/combatants/{id}", axum::routing::patch(update_combatant).delete(delete_combatant))
        .route("/combatants/{id}/move", post(move_combatant))
        .route("/combatants/{id}/use-action", post(use_action))
        .route("/encounters/{id}/next-turn", post(next_turn))
        .route("/encounters/{id}/prev-turn", post(prev_turn))
        .route("/encounters/{id}/goto-turn", post(goto_turn))
        .route("/encounters/{id}/start", post(start))
        .route("/encounters/{id}/end", post(end_encounter))
        .route("/encounters/{id}/set-initiative", post(set_initiative))
        .route("/encounters/{id}/overlays", get(list_overlays).post(create_overlay))
        .route("/encounters/{id}/overlays/{overlay_id}", axum::routing::delete(delete_overlay))
        .route("/combatants/{id}/attack", post(attack))
        .route("/combatants/{id}/damage", post(deal_damage))
        .route("/combatants/{id}/save", post(roll_save))
        .route("/combatants/{id}/computed-stats", get(computed_stats))
        .route("/combatants/{id}/react", post(react))
        .route("/combatants/{id}/cast-spell", post(cast_spell))
        .route("/combatants/{id}/dodge", post(dodge))
        .route("/combatants/{id}/disengage", post(disengage))
        .route("/combatants/{id}/help", post(help_action))
        .route("/combatants/{id}/opportunity-attack", post(opportunity_attack))
        .route("/combatants/{id}/ready", post(ready_action))
        .route("/combatants/{id}/delay", post(delay_turn))
        .route("/combatants/{id}/grapple", post(grapple))
        .route("/combatants/{id}/grapple-escape", post(grapple_escape))
        .route("/combatants/{id}/shove", post(shove))
        .route("/combatants/{id}/stand-up", post(stand_up))
        .route("/combatants/{id}/heal", post(heal))
        .route("/combatants/{id}/death-save", post(death_save))
        .route("/combatants/{id}/skill-check", post(skill_check))
        .route("/encounters/{id}/lair-action", post(lair_action))
        .route("/combatants/{id}/legendary-action", post(legendary_action))
        .route("/combatants/{id}/multiattack", post(multiattack))
        .route("/combatants/{id}/parse-multiattack", get(parse_multiattack))
        .route("/combatants/{id}/trigger-ready", post(trigger_ready))
        .route("/combatants/{id}/class-feature", post(class_feature))
        .route("/combatants/{id}/two-weapon-fight", post(two_weapon_fight))
        .route("/combatants/{id}/dash", post(dash))
        .route("/combatants/{id}/hide", post(hide))
        .route("/combatants/{id}/contested-hide", post(contested_hide))
        .route("/combatants/{id}/search", post(search_action))
        .route("/combatants/{id}/use-object", post(use_object))
        .route("/combatants/{id}/conditions", post(add_condition))
        .route("/encounters/{id}/effects", patch(patch_effects))
        .route("/encounters/{id}/overlay-damage", post(overlay_damage))
        .route("/encounters/{id}/surprise", post(surprise_round))
        .route("/encounters/{id}/surprise-auto", post(surprise_auto))
        .route("/encounters/{id}/difficulty", get(encounter_difficulty))
        .route("/encounters/{id}/flanking", get(check_flanking))
        .route("/encounters/{id}/cover", get(calculate_cover))
        .route("/encounters/{id}/events", get(list_events))
        .route("/combat-events/{event_id}", axum::routing::delete(delete_event))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Encounter {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub status: String,
    pub round: i32,
    pub turn_index: i32,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub map_grid_size: i32,
    pub show_grid: bool,
    pub grid_type: String,
    pub lair_action_used: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}



async fn fetch(s: &AppState, id: Uuid) -> AppResult<Encounter> {
    sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)
}




/// Extract condition name, stripping optional duration suffix ("poisoned:3" → "poisoned")
fn cond_name(c: &str) -> &str {
    c.split(':').next().unwrap_or(c)
}

fn has_condition(conditions: &[String], name: &str) -> bool {
    conditions.iter().any(|c| cond_name(c).eq_ignore_ascii_case(name))
}

fn remove_condition(conditions: Vec<String>, name: &str) -> Vec<String> {
    conditions.into_iter().filter(|c| !cond_name(c).eq_ignore_ascii_case(name)).collect()
}




async fn notify_turn(s: &AppState, e: &Encounter, prev_round: i32) {
    let row: Option<(String, Option<Uuid>, Uuid)> = sqlx::query_as(
        r#"select c.display_name, ch.owner_id, c.id
           from combatants c
           left join characters ch on ch.id = c.character_id
           where c.encounter_id = $1
           order by c.turn_order asc
           offset $2 limit 1"#,
    )
    .bind(e.id).bind(e.turn_index as i64).fetch_optional(&s.db).await.ok().flatten();
    if let Some((name, owner, _cid)) = row {
        if e.round > prev_round {
            emit_campaign(&s.db, e.campaign_id, None,
                "combat.round",
                &format!("Round {} — {}", e.round, name),
                None, Some("encounter"), Some(e.id)).await;
        }
        if let Some(o) = owner {
            emit(&s.db, NewNotif {
                user_id: o, campaign_id: Some(e.campaign_id),
                kind: "combat.your_turn",
                title: "It's your turn!",
                body: Some(&format!("{} — round {}", name, e.round)),
                ref_kind: Some("encounter"), ref_id: Some(e.id),
            }).await;
        }
    }
}



/// Tick down combatant effects based on turn/round advancement.
/// `old_turn` = turn_index before change, `new_turn` = after.
async fn tick_effects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    encounter_id: Uuid,
    old_round: i32,
    old_turn: i32,
    new_round: i32,
    new_turn: i32,
    campaign_id: Uuid,
) -> anyhow::Result<()> {
    // Build a mapping of turn_index -> combatant_id for this encounter
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(encounter_id)
        .fetch_all(&mut **tx).await?;

    if combatants.is_empty() { return Ok(()); }

    let _max_turn = (combatants.len() as i32) - 1;

    // Helper: find combatant ID at a given turn index
    fn cid_at(turn: i32, list: &[(i32, Uuid)]) -> Option<Uuid> {
        list.iter().find(|(t, _)| *t == turn).map(|(_, id)| *id)
    }

    // 1. round_end: tick down when round increments
    if new_round > old_round {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'round_end' and remaining is not null
               and combatant_id in (select id from combatants where encounter_id = $1)")
            .bind(encounter_id)
            .execute(&mut **tx).await?;
    }

    // 2. target_turn_end: tick down for combatant whose turn just ended
    let ended_turn = old_turn;
    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_end' and remaining is not null
               and combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 3. target_turn_start: tick down for combatant whose turn is starting
    let started_turn = new_turn;
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_start' and remaining is not null
               and combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 4. caster_turn_end: tick down for effects where caster's turn just ended
    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_end' and remaining is not null
               and caster_combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 5. caster_turn_start: tick down for effects where caster's turn is starting
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_start' and remaining is not null
               and caster_combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // Deactivate any effects whose remaining dropped to 0 or below
    let expired_effects: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "select id, combatant_id from combatant_effects
         where active = true and remaining is not null and remaining <= 0
           and combatant_id in (select id from combatants where encounter_id = $1)")
        .bind(encounter_id)
        .fetch_all(&mut **tx).await?;

    if !expired_effects.is_empty() {
        sqlx::query(
            "update combatant_effects set active = false
             where active = true and remaining is not null and remaining <= 0
               and combatant_id in (select id from combatants where encounter_id = $1)")
            .bind(encounter_id)
            .execute(&mut **tx).await?;
        for (_, combatant_id) in &expired_effects {
            ws::publish(campaign_id, json!({
                "type": "effects_change",
                "combatant_id": combatant_id
            }).to_string());
        }
    }

    // Deactivate overlays whose expiry round/turn has passed
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
        ws::publish(campaign_id, json!({
            "type": "overlays_expire",
            "ids": expired_overlays
        }).to_string());
    }

    // Per-turn effects for the combatant whose turn is starting.
    if let Some(cid) = cid_at(new_turn, &combatants) {
        let (conditions, hp_current, hp_max): (Vec<String>, i32, i32) = sqlx::query_as(
            "select conditions, hp_current, hp_max from combatants where id = $1")
            .bind(cid).fetch_optional(&mut **tx).await?.unwrap_or_default();
        // Surprised: block full turn, then remove condition
        let is_surprised = has_condition(&conditions, "surprised");
        if is_surprised {
            sqlx::query(
                "update combatants set action_used = true, bonus_action_used = true, movement_used_ft = 9999 where id = $1")
                .bind(cid).execute(&mut **tx).await?;
            let new_conds = remove_condition(conditions.clone(), "surprised");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conds).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_is_surprised",
                "combatant_id": cid,
            }).to_string());
        }

        // Hazard zones: apply per-turn damage to combatants inside hazard overlays
        let combatant_pos: Option<(f64, f64)> = sqlx::query_as(
            "select token_x, token_y from combatants where id = $1")
            .bind(cid).fetch_optional(&mut **tx).await?;
        if let Some((cx, cy)) = combatant_pos {
            let hazards: Vec<(String, f64, f64, Option<i32>, Option<String>, Option<String>, Option<String>, Option<i32>, bool)> = sqlx::query_as(
                r#"select shape, origin_x, origin_y, radius_ft,
                          hazard_damage_expression, hazard_damage_type,
                          hazard_save_ability, hazard_save_dc, hazard_half_on_save
                   from encounter_overlays
                   where encounter_id = $1 and active = true
                     and zone_type = 'hazard'
                     and hazard_damage_expression is not null"#)
                .bind(encounter_id).fetch_all(&mut **tx).await?;

            for (shape, ox, oy, rad, dmg_expr, dmg_type, save_ability, save_dc, half_on_save) in hazards {
                let r = rad.unwrap_or(20) as f64;
                let in_zone = match shape.as_str() {
                    "circle" => { let dx = cx - ox; let dy = cy - oy; (dx*dx + dy*dy).sqrt() <= r }
                    "cube" | "square" => { (cx - ox).abs() <= r && (cy - oy).abs() <= r }
                    _ => { let dx = cx - ox; let dy = cy - oy; (dx*dx + dy*dy).sqrt() <= r }
                };
                if !in_zone { continue; }

                if let (Some(ref expr), Some(ref dtype)) = (dmg_expr, dmg_type) {
                    let mut rng = rand::rngs::StdRng::from_os_rng();
                    let roll = crate::dice::roll(expr, &mut rng);
                    if let Ok(roll) = roll {
                        let snap_hp: (i32, i32, i32) = sqlx::query_as(
                            "select hp_current, hp_max, temp_hp from combatants where id = $1")
                            .bind(cid).fetch_one(&mut **tx).await?;
                        let dmg = roll.total.max(0);
                        let _ = (save_ability, save_dc, half_on_save); // full save support in overlay_damage endpoint

                        let (new_hp, new_temp) = combat_engine::apply_hp_damage(snap_hp.0, snap_hp.2, dmg);
                        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                            .bind(new_hp).bind(new_temp).bind(cid).execute(&mut **tx).await?;
                        ws::publish(campaign_id, json!({
                            "type": "combatant_takes_hazard_damage",
                            "combatant_id": cid,
                            "damage": dmg,
                            "damage_type": dtype,
                            "hp_after": new_hp,
                        }).to_string());
                    }
                }
            }
        }

        // Regeneration: sum hp_regen_per_turn from active effects modifiers
        let regen: i32 = sqlx::query_scalar(
            r#"select coalesce(sum((modifiers->>'hp_regen_per_turn')::int), 0)::int
               from combatant_effects
               where combatant_id = $1 and active = true
                 and modifiers ? 'hp_regen_per_turn'"#)
            .bind(cid).fetch_optional(&mut **tx).await?.unwrap_or(0);
        if regen > 0 && hp_current > 0 && hp_current < hp_max {
            let new_hp = (hp_current + regen).min(hp_max);
            sqlx::query("update combatants set hp_current = $1 where id = $2")
                .bind(new_hp).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_regenerates",
                "combatant_id": cid,
                "hp_restored": regen,
                "hp_after": new_hp,
            }).to_string());
        }

        // Tick down timed conditions
        let current_conditions = if is_surprised {
            remove_condition(conditions, "surprised")
        } else {
            conditions
        };
        let mut changed = false;
        let new_conditions: Vec<String> = current_conditions.into_iter().filter_map(|c| {
            if let Some(idx) = c.rfind(':') {
                let (name, num_str) = c.split_at(idx);
                if let Ok(n) = num_str[1..].parse::<i32>() {
                    if n <= 1 { changed = true; return None; }
                    changed = true;
                    return Some(format!("{}:{}", name, n - 1));
                }
            }
            Some(c)
        }).collect();
        if changed {
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conditions).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_conditions_tick",
                "combatant_id": cid,
                "conditions": new_conditions,
            }).to_string());
        }
    }

    Ok(())
}







