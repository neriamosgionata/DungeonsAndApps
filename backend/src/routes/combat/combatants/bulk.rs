// bulk_add_combatants — add multiple combatants with per-row error reporting.
use super::*;
use super::types::{BulkAddBody, BulkAddError, BulkAddResult};
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn bulk_add_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<BulkAddBody>,
) -> AppResult<Json<BulkAddResult>> {
    let e = super::super::fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut added = Vec::new();
    let mut errors: Vec<BulkAddError> = Vec::new();
    for (idx, spec) in body.combatants.iter().enumerate() {
        if spec.ref_type != "character" && spec.ref_type != "npc" {
            errors.push(BulkAddError {
                index: idx,
                display_name: Some(spec.display_name.clone()),
                error: format!("invalid ref_type: {}", spec.ref_type),
            });
            continue;
        }
        let mut npc_stats: Option<combat_engine::NpcStats> = None;
        if spec.ref_type == "npc" && spec.npc_id.is_some() {
            match sqlx::query_scalar::<_, serde_json::Value>(
                "select stats from npcs where id = $1 and campaign_id = $2",
            )
            .bind(spec.npc_id)
            .bind(e.campaign_id)
            .fetch_optional(&s.db)
            .await
            {
                Ok(Some(raw)) => {
                    npc_stats = combat_engine::NpcStats::from_value(&raw);
                }
                Ok(None) => {
                    errors.push(BulkAddError {
                        index: idx,
                        display_name: Some(spec.display_name.clone()),
                        error: format!("NPC not found: {:?}", spec.npc_id),
                    });
                    continue;
                }
                Err(er) => {
                    errors.push(BulkAddError {
                        index: idx,
                        display_name: Some(spec.display_name.clone()),
                        error: format!("NPC lookup failed: {er}"),
                    });
                    continue;
                }
            }
        }
        let default_hp_max = npc_stats.as_ref().and_then(|n| n.hp.max).unwrap_or(0);
        let default_hp_current = npc_stats
            .as_ref()
            .and_then(|n| n.hp.current)
            .unwrap_or(default_hp_max);
        let default_ac = npc_stats.as_ref().and_then(|n| n.ac).unwrap_or(10);
        let default_dex = npc_stats.as_ref().map(|n| n.abilities.dex).unwrap_or(10);
        let default_legendary = npc_stats
            .as_ref()
            .and_then(|n| n.legendary_actions.first())
            .map(|_| 3)
            .unwrap_or(0);
        let default_resist = npc_stats
            .as_ref()
            .and_then(|n| {
                n.traits
                    .iter()
                    .find(|t| t.name.to_lowercase().contains("legendary resistance"))
            })
            .map(|_| 3)
            .unwrap_or(0);
        let default_rolled = spec.ref_type != "character";

        match sqlx::query_as::<_, Combatant>(
            r#"insert into combatants
               (encounter_id, ref_type, character_id, npc_id, display_name, initiative, dex_tiebreaker,
                hp_current, hp_max, ac, is_visible, initiative_rolled,
                legendary_actions_max, legendary_resistances_max)
               values ($1, $2::combatant_ref, $3, $4, $5, coalesce($6, 0), coalesce($7, $14),
                      coalesce($8, $15), coalesce($9, $16), coalesce($10, $17), coalesce($11, true), coalesce($12, $13),
                      coalesce($18, 0), coalesce($19, 0))
               returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                         initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                         token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                         action_used, bonus_action_used, reaction_used, movement_used_ft,
                         legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                         readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
        )
        .bind(encounter_id).bind(&spec.ref_type).bind(spec.character_id).bind(spec.npc_id)
        .bind(&spec.display_name).bind(spec.initiative).bind(spec.dex_tiebreaker)
        .bind(spec.hp_current).bind(spec.hp_max).bind(spec.ac)
        .bind(spec.is_visible).bind(spec.initiative_rolled).bind(default_rolled)
        .bind(default_dex as i16).bind(default_hp_current).bind(default_hp_max)
        .bind(default_ac).bind(default_legendary).bind(default_resist)
        .fetch_one(&s.db)
        .await
        {
            Ok(c) => added.push(c),
            Err(er) => {
                errors.push(BulkAddError {
                    index: idx,
                    display_name: Some(spec.display_name.clone()),
                    error: format!("insert failed: {er}"),
                });
            }
        }
    }

    for c in &added {
        let _ = crate::routes::notifications::emit_campaign(
            &s.db,
            e.campaign_id,
            Some(uid),
            "combat.joined",
            &format!("{} joined combat", c.display_name),
            Some(&format!(
                "Init {} · HP {}/{} · AC {}",
                c.initiative, c.hp_current, c.hp_max, c.ac
            )),
            Some("encounter"),
            Some(encounter_id),
        )
        .await;
    }

    if !added.is_empty() {
        ws::publish(
            e.campaign_id,
            json!({"type":"combatant_joins","encounter_id":encounter_id,"id":added[0].id}).to_string(),
        );
    }

    Ok(Json(BulkAddResult {
        added: added.len(),
        failed: errors.len(),
        combatants: added,
        errors,
    }))
}
