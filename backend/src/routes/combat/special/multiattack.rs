// Multiattack handler and trigger_ready handler.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct MultiAttackTarget {
    pub target_id: Uuid,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    pub damage_type: String,
    pub damage_die: Option<String>,
    pub ability: Option<String>,
    pub weapon_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct MultiAttackBody {
    pub targets: Vec<MultiAttackTarget>,
}

#[derive(Debug, Serialize)]
pub struct MultiAttackResult {
    pub results: Vec<combat_engine::AttackResult>,
    pub targets_hit: usize,
    pub total_damage: i32,
}

pub async fn multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MultiAttackBody>,
) -> AppResult<Json<MultiAttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let needs_auto = body
        .targets
        .iter()
        .all(|t| t.attack_expression.is_none() && t.weapon_id.is_none());
    let targets: Vec<MultiAttackTarget> = if !needs_auto {
        body.targets
            .iter()
            .map(|t| MultiAttackTarget {
                target_id: t.target_id,
                attack_expression: t.attack_expression.clone(),
                damage_expression: t.damage_expression.clone(),
                damage_type: t.damage_type.clone(),
                damage_die: t.damage_die.clone(),
                ability: t.ability.clone(),
                weapon_id: t.weapon_id.clone(),
                label: t.label.clone(),
            })
            .collect()
    } else if let Ok(super::parse_multiattack::ParsedMultiAttack { attacks }) =
        super::parse_multiattack::try_parse_npc_multiattack(&s.db, id).await
    {
        if attacks.is_empty() {
            return Err(AppError::BadRequest(
                "no targets and could not parse NPC multiattack".into(),
            ));
        }
        body.targets
            .iter()
            .enumerate()
            .map(|(i, t)| {
                let atk = attacks.get(i).cloned().unwrap_or_default();
                MultiAttackTarget {
                    target_id: t.target_id,
                    attack_expression: t.attack_expression.clone().or(atk.attack_expression),
                    damage_expression: t.damage_expression.clone().or(atk.damage_expression),
                    damage_type: if t.damage_type == "slashing" && !atk.damage_type.is_empty() {
                        atk.damage_type
                    } else {
                        t.damage_type.clone()
                    },
                    damage_die: t.damage_die.clone(),
                    ability: t.ability.clone(),
                    weapon_id: t.weapon_id.clone(),
                    label: t.label.clone().or(atk.label),
                }
            })
            .collect()
    } else {
        return Err(AppError::BadRequest("no targets specified".into()));
    };

    if targets.is_empty() {
        return Err(AppError::BadRequest("no targets specified".into()));
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let mut total_damage = 0i32;
    let mut targets_hit = 0usize;

    // Batch load all target snapshots in one query (N+1 fix).
    let target_ids: Vec<Uuid> = targets.iter().map(|t| t.target_id).collect();
    let target_snaps = combat_engine::load_snapshots_batch(&s.db, &target_ids).await?;
    // HIGH-1: index each result by its position in the FINAL `targets` list (not
    // body.targets). `results.get(i)` in the apply loop must align with
    // `targets[i]` — using body.targets indices when `needs_auto` reorders
    // (or when resolve_attack returns Err) would apply damage to the wrong
    // combatant. `target_results[i] = None` for skipped targets.
    let mut target_results: Vec<Option<combat_engine::AttackResult>> =
        (0..targets.len()).map(|_| None).collect();
    for (i, t) in targets.iter().enumerate() {
        let target_snap = match target_snaps.get(&t.target_id) {
            Some(s) => s,
            None => continue,
        };
        if target_snap.encounter_id != attacker_snap.encounter_id {
            continue;
        }
        let target_stats = combat_engine::compute_stats(&target_snap);

        let req = combat_engine::AttackReq {
            target_id: t.target_id,
            attack_expression: t.attack_expression.clone(),
            damage_expression: t.damage_expression.clone(),
            damage_type: t.damage_type.clone(),
            damage_die: t.damage_die.clone(),
            ability: t.ability.clone(),
            proficient: Some(true),
            advantage: false,
            disadvantage: false,
            cover: None,
            is_spell_attack: false,
            is_magical: false,
            label: t.label.clone(),
            weapon_id: t.weapon_id.clone(),
            extra_damage_expression: None,
            extra_damage_type: None,
            power_attack: false,
            reckless: false,
            bless_dice: None,
            bardic_inspiration_dice: None,
        };

        match combat_engine::resolve_attack(
            &attacker_snap,
            &target_snap,
            &req,
            &attacker_stats,
            &target_stats,
        ) {
            Ok(res) => {
                if res.hit {
                    targets_hit += 1;
                    total_damage += res.damage_applied;
                }
                target_results[i] = Some(res);
            }
            Err(_) => continue,
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let mut tx = s.db.begin().await?;

    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // F11: collect (id, hp, temp, damage, label) for all HITS, then apply
    // 1 batched UPDATE combatants + 1 batched UPDATE combatant_effects
    // (concentration breaks) + 1 batched sheet sync + 1 batched INSERT
    // combat_events. 5 hits = 4 queries instead of 20.
    let mut hits: Vec<(Uuid, i32, i32, i32, Option<String>)> = Vec::new();
    let mut conc_broken: Vec<Uuid> = Vec::new();
    for (t, res_opt) in targets.iter().zip(target_results.iter()) {
        if let Some(res) = res_opt {
            if res.hit {
                hits.push((
                    t.target_id,
                    res.target_hp_after,
                    res.target_temp_hp_after,
                    res.damage_applied,
                    t.label.clone(),
                ));
                if res.concentration_broken {
                    conc_broken.push(t.target_id);
                }
            }
        }
    }

    if !hits.is_empty() {
        // Batched UPDATE combatants hp+temp.
        let hit_ids: Vec<Uuid> = hits.iter().map(|(id, _, _, _, _)| *id).collect();
        let hit_hps: Vec<i32> = hits.iter().map(|(_, hp, _, _, _)| *hp).collect();
        let hit_temps: Vec<i32> = hits.iter().map(|(_, _, temp, _, _)| *temp).collect();
        sqlx::query(
            r#"update combatants as c
               set hp_current = v.hp, temp_hp = v.tmp
               from unnest($1::uuid[], $2::int[], $3::int[]) as v(id, hp, tmp)
               where c.id = v.id"#,
        )
        .bind(&hit_ids)
        .bind(&hit_hps)
        .bind(&hit_temps)
        .execute(&mut *tx)
        .await?;

        // Batched UPDATE combatant_effects for concentration breaks.
        if !conc_broken.is_empty() {
            sqlx::query(
                "update combatant_effects set active = false
                 where concentration = true and active = true
                   and combatant_id = ANY($1::uuid[])",
            )
            .bind(&conc_broken)
            .execute(&mut *tx)
            .await?;
        }

        // Batched sheet sync (1 SELECT + 1 UPDATE for all characters).
        if let Err(e) = super::super::actions::sync_combatant_hp_to_sheet_batch_tx(
            &mut *tx,
            &hits.iter().map(|(id, hp, temp, _, _)| (*id, *hp, *temp)).collect::<Vec<_>>(),
        )
        .await
        {
            tracing::error!("multiattack batched sheet sync: {e}");
        }

        // Batched INSERT combat_events.
        let evt_targets: Vec<Uuid> = hits.iter().map(|(id, _, _, _, _)| *id).collect();
        let evt_actions: Vec<String> = hits
            .iter()
            .map(|(_, _, _, dmg, _)| format!("Multiattack: {} damage", dmg))
            .collect();
        let evt_deltas: Vec<i32> = hits.iter().map(|(_, _, _, dmg, _)| -dmg).collect();
        let evt_notes: Vec<Option<String>> = hits.iter().map(|(_, _, _, _, l)| l.clone()).collect();
        sqlx::query(
            r#"insert into combat_events
               (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note)
               select $1, $2, $3, t.id, t.action, t.delta, t.note
               from unnest($4::uuid[], $5::text[], $6::int[], $7::text[])
                 as t(id, action, delta, note)"#,
        )
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(&evt_targets)
        .bind(&evt_actions)
        .bind(&evt_deltas)
        .bind(&evt_notes)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_multiattacks",
            "attacker_id": id,
            "targets_hit": targets_hit,
            "total_damage": total_damage,
        }),
    )
    .await;

    let results: Vec<combat_engine::AttackResult> =
        target_results.into_iter().flatten().collect();
    Ok(Json(MultiAttackResult {
        results,
        targets_hit,
        total_damage,
    }))
}

pub async fn trigger_ready(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Option<String>, bool, bool, String) = sqlx::query_as(
        r#"select e.campaign_id, c.readied_action::text, c.action_used, c.reaction_used, e.status::text
           from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let (campaign_id, readied, _action_used, reaction_used, status) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    if readied.is_none() {
        return Err(AppError::BadRequest("no readied action to trigger".into()));
    }
    if reaction_used {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             reaction_used = true,
             readied_action = null,
             action_used = false
           where id = $1 and reaction_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#)
        .bind(id).fetch_optional(&s.db).await?
        .ok_or(AppError::BadRequest("reaction already used".into()))?;

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_triggers_readied_action",
            "combatant_id": id,
            "readied_action": readied,
        }),
    )
    .await;

    Ok(Json(c))
}
