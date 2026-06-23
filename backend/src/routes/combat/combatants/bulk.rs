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
    if body.combatants.is_empty() || body.combatants.len() > 100 {
        return Err(AppError::BadRequest(format!(
            "combatants array must contain 1-100 items, got {}",
            body.combatants.len()
        )));
    }

    let mut added = Vec::new();
    let mut errors: Vec<BulkAddError> = Vec::new();

    // First pass: validate every row, collect distinct NPC ids + character/npc ids
    // for batch lookups. N+1 fix — was 4 queries per row, now 2 batched.
    let mut npc_stats_cache: std::collections::HashMap<Uuid, Option<combat_engine::NpcStats>> =
        std::collections::HashMap::new();
    let mut all_npc_ids: Vec<Uuid> = Vec::new();
    let mut all_char_ids: Vec<Uuid> = Vec::new();
    for (idx, spec) in body.combatants.iter().enumerate() {
        if let Err(ve) = spec.validate() {
            errors.push(BulkAddError {
                index: idx,
                display_name: Some(spec.display_name.clone()),
                error: format!("invalid row: {ve}"),
            });
            continue;
        }
        if spec.ref_type != "character" && spec.ref_type != "npc" {
            errors.push(BulkAddError {
                index: idx,
                display_name: Some(spec.display_name.clone()),
                error: format!("invalid ref_type: {}", spec.ref_type),
            });
            continue;
        }
        if spec.ref_type == "npc" {
            if let Some(nid) = spec.npc_id {
                if !npc_stats_cache.contains_key(&nid) {
                    all_npc_ids.push(nid);
                    npc_stats_cache.insert(nid, None);
                }
            }
        }
        if let Some(chid) = spec.character_id {
            all_char_ids.push(chid);
        }
    }
    // Batch fetch NPC stats
    if !all_npc_ids.is_empty() {
        let rows: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
            "select id, stats from npcs where campaign_id = $1 and id = any($2)",
        )
        .bind(e.campaign_id)
        .bind(&all_npc_ids)
        .fetch_all(&s.db)
        .await?;
        for (id, raw) in rows {
            npc_stats_cache.insert(id, combat_engine::NpcStats::from_value(&raw));
        }
    }
    // Batch fetch existing duplicates in this encounter
    let mut existing_char_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    let mut existing_npc_ids: std::collections::HashSet<Uuid> = std::collections::HashSet::new();
    if !all_char_ids.is_empty() || !all_npc_ids.is_empty() {
        // Combined query: fetch any character_id OR npc_id from this encounter
        // that matches our incoming set. We union both sets in the query.
        // Postgres: id = any($2) OR id = any($3). character_id IS NOT NULL filter.
        let char_existing: Vec<Uuid> = if all_char_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_scalar::<_, Option<Uuid>>(
                "select character_id from combatants
                 where encounter_id = $1 and character_id = any($2)",
            )
            .bind(encounter_id)
            .bind(&all_char_ids)
            .fetch_all(&s.db)
            .await?
            .into_iter()
            .flatten()
            .collect()
        };
        existing_char_ids.extend(char_existing);
        let npc_existing: Vec<Uuid> = if all_npc_ids.is_empty() {
            Vec::new()
        } else {
            sqlx::query_scalar::<_, Option<Uuid>>(
                "select npc_id from combatants
                 where encounter_id = $1 and npc_id = any($2)",
            )
            .bind(encounter_id)
            .bind(&all_npc_ids)
            .fetch_all(&s.db)
            .await?
            .into_iter()
            .flatten()
            .collect()
        };
        existing_npc_ids.extend(npc_existing);
    }

    // Second pass: insert rows that passed validation + dup check.
    // HIGH-12: wrap the loop in a single tx. Per-row failure is isolated via
    // savepoints so the rest of the batch still commits. The DB's unique
    // partial index (`20260617000002`) is the ultimate safety net against
    // dup_char/dup_npc races.
    let mut tx = s.db.begin().await?;
    for (idx, spec) in body.combatants.iter().enumerate() {
        // Skip if row was already flagged in validation pass
        if errors.iter().any(|e| e.index == idx) {
            continue;
        }
        let npc_stats: Option<&combat_engine::NpcStats> = if spec.ref_type == "npc" {
            spec.npc_id.and_then(|nid| npc_stats_cache.get(&nid).and_then(|s| s.as_ref()))
        } else {
            None
        };
        // NPC existence check
        if spec.ref_type == "npc" {
            match spec.npc_id {
                Some(nid) if npc_stats_cache.get(&nid).is_none() => {
                    errors.push(BulkAddError {
                        index: idx,
                        display_name: Some(spec.display_name.clone()),
                        error: format!("NPC not found: {nid:?}"),
                    });
                    continue;
                }
                None => {
                    errors.push(BulkAddError {
                        index: idx,
                        display_name: Some(spec.display_name.clone()),
                        error: "npc_id required for ref_type=npc".into(),
                    });
                    continue;
                }
                _ => {}
            }
        }
        // Dup check
        if let Some(chid) = spec.character_id {
            if existing_char_ids.contains(&chid) {
                errors.push(BulkAddError {
                    index: idx,
                    display_name: Some(spec.display_name.clone()),
                    error: "character already in encounter".into(),
                });
                continue;
            }
            existing_char_ids.insert(chid); // reserve for later rows
        }
        if let Some(nid) = spec.npc_id {
            if existing_npc_ids.contains(&nid) {
                errors.push(BulkAddError {
                    index: idx,
                    display_name: Some(spec.display_name.clone()),
                    error: "NPC already in encounter".into(),
                });
                continue;
            }
            existing_npc_ids.insert(nid);
        }
        let default_hp_max = npc_stats.and_then(|n| n.hp.max).unwrap_or(0);
        let default_hp_current = npc_stats
            .and_then(|n| n.hp.current)
            .unwrap_or(default_hp_max);
        let default_ac = npc_stats.and_then(|n| n.ac).unwrap_or(10);
        let default_dex = npc_stats.map(|n| n.abilities.dex).unwrap_or(10);
        let default_legendary = npc_stats
            .and_then(|n| n.legendary_actions.first())
            .map(|_| 3)
            .unwrap_or(0);
        let default_resist = npc_stats
            .and_then(|n| {
                n.traits
                    .iter()
                    .find(|t| t.name.to_lowercase().contains("legendary resistance"))
            })
            .map(|_| 3)
            .unwrap_or(0);
        let default_rolled = spec.ref_type != "character";

        // Per-row savepoint: roll back just this row on error, keep the tx
        // alive for the rest of the batch.
        let sp = format!("sp_{}", idx);
        if let Err(e) = sqlx::query(&format!("savepoint {sp}"))
            .execute(&mut *tx)
            .await
        {
            errors.push(BulkAddError {
                index: idx,
                display_name: Some(spec.display_name.clone()),
                error: format!("savepoint failed: {e}"),
            });
            continue;
        }
        let result = sqlx::query_as::<_, Combatant>(
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
        .fetch_one(&mut *tx)
        .await;
        match result {
            Ok(c) => added.push(c),
            Err(er) => {
                // Roll back this row's savepoint so the tx stays usable.
                let _ = sqlx::query(&format!("rollback to savepoint {sp}"))
                    .execute(&mut *tx)
                    .await;
                errors.push(BulkAddError {
                    index: idx,
                    display_name: Some(spec.display_name.clone()),
                    error: format!("insert failed: {er}"),
                });
            }
        }
    }
    tx.commit().await?;

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

    for c in &added {
        ws::publish_persist(
            &s.db,
            e.campaign_id,
            json!({
                "type": "combatant_joins",
                "encounter_id": encounter_id,
                "id": c.id,
            }),
        )
        .await;
    }

    Ok(Json(BulkAddResult {
        added: added.len(),
        failed: errors.len(),
        combatants: added,
        errors,
    }))
}
