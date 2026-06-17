// Reaction handlers (Shield, Counterspell, Ready Action) and auto-trigger.
// Extracted from actions.rs to keep the route handler file under the 500-line
// guideline (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
use super::*;
use crate::AppState;
use crate::error::AppResult;
use crate::extract::AuthUser;
use axum::Json;
use axum::extract::{Path, State};

#[derive(Debug, Deserialize)]
pub struct ReactBody {
    pub reaction_type: String, // shield | counterspell | opportunity_attack | custom
    pub label: Option<String>,
    /// Counterspell: which caster's spell to counter. None = legacy LIMIT 1 behavior.
    pub target_caster_id: Option<Uuid>,
    /// Counterspell: slot level used to cast. Drives auto-success check.
    pub slot_level: Option<i32>,
    /// Counterspell: if slot < target_spell_level, client rolls ability check
    /// and passes the total here. Backend validates vs DC = 10 + target_spell_level.
    pub ability_check_total: Option<i32>,
}

pub async fn react(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<super::super::combatants::Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let encounter_id = auth.encounter_id;
    let mut tx = s.db.begin().await?;

    // Atomic reaction consumption
    let c: super::super::combatants::Combatant = sqlx::query_as::<_, super::super::combatants::Combatant>(
        r#"update combatants set reaction_used = true where id = $1 and reaction_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?
    .ok_or(AppError::BadRequest("reaction already used this round".into()))?;

    let mut shield_blocked_hit = false;
    match body.reaction_type.as_str() {
        "shield" => {
            let row: (serde_json::Value, Option<i32>) =
                sqlx::query_as("select pending_hits, hp_max from combatants where id = $1")
                    .bind(id)
                    .fetch_one(&mut *tx)
                    .await?;
            let (pending_hits_raw, hp_max_col_opt) = row;
            let mut hits: Vec<serde_json::Value> =
                pending_hits_raw.as_array().cloned().unwrap_or_default();
            let hit = hits.last().cloned().ok_or_else(|| {
                AppError::BadRequest(
                    "Shield can only be used when you have been hit (no pending hit this round)"
                        .into(),
                )
            })?;
            let atk_total = hit
                .get("attack_total")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            let pending_dmg = hit
                .get("damage")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            hits.pop();
            let new_pending = serde_json::Value::Array(hits);

            let snap = combat_engine::load_snapshot(&s.db, id).await?;
            let stats = combat_engine::compute_stats(&snap);
            let ac_with_shield = stats.ac + 5;
            let attack_total = atk_total.unwrap_or(0);

            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Shield (Reaction)', 'buff', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, '{"ac_bonus": 5}', 'spell')"#,
            )
            .bind(id).execute(&mut *tx).await?;

            if attack_total < ac_with_shield {
                let dmg_to_restore = pending_dmg.unwrap_or(0);
                let (current_hp, sheet_red): (i32, i32) = sqlx::query_as(
                    "select hp_current, coalesce((sheet->>'hp_max_reduction')::int, 0) from combatants c
                     left join characters ch on ch.id = c.character_id where c.id = $1")
                    .bind(id).fetch_one(&mut *tx).await?;
                let hp_max_col = hp_max_col_opt.unwrap_or(0);
                let effective_max = (hp_max_col - sheet_red).max(1);
                let new_hp = (current_hp + dmg_to_restore).min(effective_max);
                sqlx::query("update combatants set hp_current = $1, last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, pending_hits = $2 where id = $3")
                    .bind(new_hp).bind(&new_pending).bind(id).execute(&mut *tx).await?;
                shield_blocked_hit = true;
            } else {
                sqlx::query("update combatants set last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, pending_hits = $2 where id = $1")
                    .bind(id).bind(&new_pending).execute(&mut *tx).await?;
            }
        }
        "counterspell" => {
            let (caster_id, target_spell_level): (Uuid, i32) = if let Some(target_id) =
                body.target_caster_id
            {
                let row: Option<(Uuid, String)> = sqlx::query_as(
                    r#"select id, spell_being_cast from combatants
                       where id = $1 and encounter_id = $2 and spell_being_cast is not null"#,
                )
                .bind(target_id)
                .bind(encounter_id)
                .fetch_optional(&mut *tx)
                .await?;
                let (cid, slug) = row.ok_or_else(|| AppError::BadRequest(
                    "Counterspell target is not currently casting a spell (or not in this encounter)".into()
                ))?;
                let lvl: i32 = sqlx::query_scalar("select level from spells where slug = $1")
                    .bind(&slug)
                    .fetch_one(&s.db)
                    .await?;
                (cid, lvl)
            } else {
                let row: Option<(Uuid, String)> = sqlx::query_as(
                    r#"select id, spell_being_cast from combatants
                       where encounter_id = $1 and spell_being_cast is not null
                       limit 1"#,
                )
                .bind(encounter_id)
                .fetch_optional(&mut *tx)
                .await?;
                if row.is_none() {
                    return Err(AppError::BadRequest(
                        "Counterspell can only be used when a spell is being cast".into(),
                    ));
                }
                let (cid, slug) = row.unwrap();
                let lvl: i32 = sqlx::query_scalar("select level from spells where slug = $1")
                    .bind(&slug)
                    .fetch_one(&s.db)
                    .await?;
                (cid, lvl)
            };

            if let Some(slot) = body.slot_level {
                if slot < target_spell_level {
                    let dc = 10 + target_spell_level;
                    let total = body.ability_check_total.ok_or_else(|| AppError::BadRequest(
                        format!("Counterspell requires ability check (slot {} < target {}); pass ability_check_total (DC {})", slot, target_spell_level, dc)
                    ))?;
                    if total < dc {
                        return Err(AppError::BadRequest(format!(
                            "Counterspell failed: ability check {} < DC {}",
                            total, dc
                        )));
                    }
                }
            } else if body.target_caster_id.is_some() {
                return Err(AppError::BadRequest(
                    "Counterspell: slot_level is required when target_caster_id is provided".into(),
                ));
            }

            sqlx::query("update combatants set spell_being_cast = null where id = $1")
                .bind(caster_id)
                .execute(&mut *tx)
                .await?;
        }
        _ => {}
    }

    tx.commit().await?;

    let label = body.label.unwrap_or_else(|| body.reaction_type.clone());
    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_reacts",
            "combatant_id": id,
            "reaction_type": body.reaction_type,
            "label": label,
            "shield_blocked_hit": shield_blocked_hit,
        })
        .to_string(),
    );

    emit_campaign(
        &s.db,
        campaign_id,
        None,
        "combat.reaction",
        &format!("{} used reaction: {}", c.display_name, label),
        None,
        Some("encounter"),
        Some(encounter_id),
    )
    .await;

    Ok(Json(c))
}

pub async fn auto_trigger_ready_actions_for_event(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    encounter_id: Uuid,
    event_type: &str,
    actor_id: Uuid,
    subject_id: Uuid,
) {
    let readied: Vec<(
        Uuid,
        serde_json::Value,
        Option<f32>,
        Option<f32>,
        Option<i32>,
    )> = match sqlx::query_as(
        r#"select id, readied_action, token_x, token_y,
                  (select map_grid_size from encounters where id = $1)
             from combatants
             where encounter_id = $1 and readied_action is not null and reaction_used = false"#,
    )
    .bind(encounter_id)
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(encounter_id = %encounter_id, "auto_trigger_ready: readied query failed: {e}");
            return;
        }
    };
    let _ = encounter_id;
    let positions: Vec<(Uuid, Option<f32>, Option<f32>)> = sqlx::query_as(
        "select id, token_x, token_y from combatants where id = any($1)")
        .bind(&[actor_id, subject_id][..])
        .fetch_all(db).await.unwrap_or_else(|e| {
            tracing::error!(encounter_id = %encounter_id, "auto_trigger_ready: positions query failed: {e}; continuing empty");
            Vec::new()
        });
    let subject_pos = positions
        .iter()
        .find(|(id, _, _)| *id == subject_id)
        .cloned();

    for (cid, action_json, r_x, r_y, grid_size) in readied {
        if cid == actor_id {
            continue;
        }

        let trigger_event = action_json
            .get("trigger_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let watch_target = action_json
            .get("watch_target_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        if trigger_event != event_type {
            continue;
        }

        if let Some(wid) = watch_target {
            if wid != subject_id {
                continue;
            }
        }

        if trigger_event == "target_enters_range" {
            let watch_ft: f32 = action_json
                .get("watch_distance_ft")
                .and_then(|v| v.as_f64())
                .map(|v| v as f32)
                .unwrap_or(5.0);
            let grid_ft: f32 = grid_size.map(|g| g as f32).unwrap_or(50.0);
            let cell_pct = (grid_ft / 6.0).max(1.0);
            let ft_per_pct = 5.0 / cell_pct;
            let dist_ft = match (
                r_x,
                r_y,
                subject_pos.as_ref().and_then(|p| p.1),
                subject_pos.as_ref().and_then(|p| p.2),
            ) {
                (Some(rx), Some(ry), Some(sx), Some(sy)) => {
                    let dx = (rx - sx) as f32;
                    let dy = (ry - sy) as f32;
                    ((dx * dx + dy * dy).sqrt()) * ft_per_pct
                }
                _ => f32::MAX,
            };
            if dist_ft > watch_ft {
                continue;
            }
        }

        let ok = match sqlx::query(
            "update combatants set reaction_used = true, readied_action = null, action_used = false
             where id = $1 and reaction_used = false",
        )
        .bind(cid)
        .execute(db)
        .await
        {
            Ok(_) => true,
            Err(e) => {
                tracing::error!(combatant_id = %cid, "auto_trigger_ready: reaction consume failed: {e}");
                false
            }
        };

        if ok {
            // Build a dispatch hint so the client can POST the appropriate endpoint.
            // Backend intentionally does not re-enter the attack handler here — that would
            // require duplicate auth + tx + sheet-sync. The client sees this event and
            // dispatches the actual effect (see +page.svelte combatant_readied_triggers).
            let action_kind = action_json
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let target_id = action_json
                .get("target_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<Uuid>().ok());
            let dispatch = match (action_kind, target_id) {
                ("attack", Some(tid)) => json!({
                    "endpoint": "attack",
                    "payload": { "target_id": tid }
                }),
                ("cast spell", _) => json!({
                    "endpoint": "cast_spell",
                    "payload": { "target_id": target_id }
                }),
                _ => json!({"endpoint": "noop"}),
            };
            tracing::info!(
                combatant_id = %cid,
                trigger_event = %event_type,
                action = %action_kind,
                has_target = target_id.is_some(),
                "readied action auto-triggered"
            );
            ws::publish(
                campaign_id,
                json!({
                    "type": "combatant_readied_triggers",
                    "combatant_id": cid,
                    "trigger_event": event_type,
                    "triggered_by": actor_id,
                    "readied_action": action_json,
                    "dispatch": dispatch,
                })
                .to_string(),
            );
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct ReadyBody {
    pub trigger: String,
    pub action: String,
    pub _target_id: Option<Uuid>,
    pub trigger_event: Option<String>,
    pub watch_target_id: Option<Uuid>,
}

pub async fn ready_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadyBody>,
) -> AppResult<Json<super::super::combatants::Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let current_round = auth.round;

    let readied = json!({
        "trigger": body.trigger,
        "action": body.action,
        "target_id": body._target_id,
        "trigger_event": body.trigger_event,
        "watch_target_id": body.watch_target_id,
        "set_at_round": current_round,
        "expires_at_round": current_round + 1,
    });

    let mut tx = s.db.begin().await?;
    let c: Option<super::super::combatants::Combatant> = sqlx::query_as::<_, super::super::combatants::Combatant>(
        r#"update combatants set action_used = true, readied_action = $2
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast, level_override, vision_range, pending_hits"#,
    )
    .bind(id)
    .bind(readied)
    .fetch_optional(&mut *tx).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;
    tx.commit().await?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_readies",
            "id": id,
            "trigger": body.trigger,
            "action": body.action,
        })
        .to_string(),
    );

    Ok(Json(c))
}
