// Combatant ↔ character sheet sync utilities.
// Extracted from actions.rs to keep the route handler file under the 500-line
// guideline (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
use super::super::combatants::Combatant;
use crate::error::AppResult;
use uuid::Uuid;

/// Sync combatant HP/temp to linked character sheet (non-tx).
/// Preserves sheet.hp_max_reduction: writes raw max = effective + reduction so the
/// combat→sheet→combat round trip doesn't drop the debuff.
pub async fn sync_combatant_hp_to_sheet(
    db: &sqlx::PgPool,
    combatant_id: Uuid,
    hp: i32,
    temp: i32,
) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac,
                coalesce((sheet->>'hp_max_reduction')::int, 0)
           from combatants c
           left join characters ch on ch.id = c.character_id
          where c.id = $1 and c.ref_type = 'character'",
    )
    .bind(combatant_id)
    .fetch_optional(db)
    .await?;
    if let Some((chid, hp_max_effective, ac, reduction)) = row {
        let alive = hp > 0;
        let hp_max_raw = hp_max_effective + reduction;
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max_raw).bind(temp).bind(ac).bind(alive)
        .execute(db).await?;
        // H-22: combat→sheet sync was silent — the character page only
        // reloads on `character_updated`/`combatant_updates`, so sheet HP
        // stayed stale after every attack/damage/heal until a manual reload.
        // Publish the event here so ALL combat sync callers (attack, damage,
        // heal, death save, fall) get it. (AGENTS.md §10.6 documented this
        // emit — the code never did it.)
        let campaign_id: Option<Uuid> = sqlx::query_scalar(
            "select e.campaign_id from combatants c
             join encounters e on e.id = c.encounter_id where c.id = $1",
        )
        .bind(combatant_id)
        .fetch_optional(db)
        .await?
        .flatten();
        if let Some(cid) = campaign_id {
            crate::ws::publish_persist(
                db,
                cid,
                serde_json::json!({"type":"character_updated","id":chid}),
            )
            .await;
        }
    }
    Ok(())
}

/// Sync combatant HP/temp to linked character sheet (inside a tx).
/// R6: preserves sheet.hp_max_reduction like the non-tx variant — the
/// combatant hp_max is the EFFECTIVE max; the sheet stores the raw max.
pub async fn sync_combatant_hp_to_sheet_tx(
    conn: &mut sqlx::PgConnection,
    combatant_id: Uuid,
    hp: i32,
    temp: i32,
) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac,
                coalesce((ch.sheet->>'hp_max_reduction')::int, 0)
           from combatants c
           left join characters ch on ch.id = c.character_id
          where c.id = $1 and c.ref_type = 'character'",
    )
    .bind(combatant_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((chid, hp_max_effective, ac, reduction)) = row {
        let alive = hp > 0;
        let hp_max_raw = hp_max_effective + reduction;
        sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max_raw).bind(temp).bind(ac).bind(alive)
        .execute(&mut *conn).await?;
    }
    Ok(())
}

/// F11: batched variant of sync_combatant_hp_to_sheet_tx. For N combatants,
/// 1 SELECT + 1 UPDATE instead of N×2. Caller passes all (id, hp, temp) at
/// once; NPCs (no character_id) are silently skipped via the WHERE.
pub async fn sync_combatant_hp_to_sheet_batch_tx(
    conn: &mut sqlx::PgConnection,
    updates: &[(Uuid, i32, i32)],
) -> AppResult<()> {
    if updates.is_empty() {
        return Ok(());
    }
    let ids: Vec<Uuid> = updates.iter().map(|(id, _, _)| *id).collect();
    // 1 query: pull character_id, hp_max, ac, hp_max_reduction for the
    // character-bound subset (R6: reduction-preserving like the non-tx
    // variant — combatant hp_max is effective, sheet stores raw).
    let rows: Vec<(Uuid, Uuid, i32, i32, i32)> = sqlx::query_as(
        "select c.id, c.character_id, c.hp_max, c.ac,
                coalesce((ch.sheet->>'hp_max_reduction')::int, 0)
           from combatants c
           left join characters ch on ch.id = c.character_id
          where c.id = ANY($1::uuid[]) and c.ref_type = 'character'",
    )
    .bind(&ids)
    .fetch_all(&mut *conn)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    // Build per-row update params from the join.
    let hp_map: std::collections::HashMap<Uuid, (i32, i32)> = updates
        .iter()
        .map(|(id, hp, temp)| (*id, (*hp, *temp)))
        .collect();
    let mut chids: Vec<Uuid> = Vec::with_capacity(rows.len());
    let mut hps: Vec<i32> = Vec::with_capacity(rows.len());
    let mut maxes: Vec<i32> = Vec::with_capacity(rows.len());
    let mut temps: Vec<i32> = Vec::with_capacity(rows.len());
    let mut acs: Vec<i32> = Vec::with_capacity(rows.len());
    let mut alives: Vec<bool> = Vec::with_capacity(rows.len());
    for (cid, chid, hp_max, ac, reduction) in rows {
        let (hp, temp) = match hp_map.get(&cid) {
            Some(v) => *v,
            None => continue,
        };
        chids.push(chid);
        hps.push(hp);
        maxes.push(hp_max + reduction);
        temps.push(temp);
        acs.push(ac);
        alives.push(hp > 0);
    }
    // 1 query: batched UPDATE via unnest. Replicates the single-row
    // jsonb_build_object per character.
    sqlx::query(
        r#"update characters as c
           set sheet =
             coalesce(sheet, '{}'::jsonb)
             || jsonb_build_object(
                  'hp', coalesce(sheet->'hp', '{}'::jsonb)
                        || jsonb_build_object('current', v.hp::int, 'max', v.mx::int, 'temp', v.tmp::int),
                  'ac', v.ac::int,
                  'alive', v.alive::bool,
                  'death_saves', case when v.alive::bool and coalesce((sheet->>'alive')::bool, true) = false
                                   then jsonb_build_object('successes', 0, 'failures', 0)
                                   else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                 end
                )
           from unnest($1::uuid[], $2::int[], $3::int[], $4::int[], $5::int[], $6::bool[])
             as v(id, hp, mx, tmp, ac, alive)
           where c.id = v.id"#,
    )
    .bind(&chids)
    .bind(&hps)
    .bind(&maxes)
    .bind(&temps)
    .bind(&acs)
    .bind(&alives)
    .execute(&mut *conn)
    .await?;
    Ok(())
}

/// A17: a mount reduced to 0 HP dismounts its rider (PHB p.198).
pub async fn dismount_riders(conn: &mut sqlx::PgConnection, mount_id: Uuid) -> AppResult<()> {
    sqlx::query("update combatants set mounted_on = null where mounted_on = $1")
        .bind(mount_id)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Re-read full combatant row (returns updated state after mutations).
pub async fn refresh_combatant(db: &sqlx::PgPool, id: Uuid) -> AppResult<Combatant> {
    sqlx::query_as::<_, Combatant>(
        r#"select id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                token_x, token_y, token_color, token_on_map, token_image,
                coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url,
                token_moved_round,
                action_used, bonus_action_used, reaction_used, movement_used_ft,
                legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits, mounted_on
         from combatants where id = $1"#,
    )
    .bind(id)
    .fetch_one(db)
    .await
    .map_err(|_| crate::error::AppError::NotFound)
}

/// L-10: secondary attack paths (opportunity attack, two-weapon fighting,
/// polearm master BA) must apply the same post-hit effects as the main
/// attack path: pending_hits (so Shield/Parry/etc. can react), death-save
/// failures at 0 HP, instant death, rage-on-KO and mount dismount.
pub async fn apply_hit_side_effects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    attacker_id: Uuid,
    target_id: Uuid,
    target_snap: &crate::combat_engine::CombatantSnapshot,
    result: &crate::combat_engine::AttackResult,
    round: i32,
) -> AppResult<()> {
    if !result.hit {
        return Ok(());
    }
    let fail_inc: i32 = if !result.instant_death
        && target_snap.hp_current <= 0
        && result.target_hp_after <= 0
    {
        if result.critical { 2 } else { 1 }
    } else {
        0
    };
    sqlx::query("update combatants set pending_hits = pending_hits || $2 where id = $1")
        .bind(target_id)
        .bind(serde_json::json!({
            "attacker_id": attacker_id,
            "attack_total": result.attack_total,
            "damage": result.damage_applied,
            "round": round,
            "hp_before": target_snap.hp_current,
            "hp_after": result.target_hp_after,
            "natural_roll": result.natural_roll,
            "bonus": result.attack_total - result.natural_roll,
            "temp_before": target_snap.temp_hp,
            "temp_after": result.target_temp_hp_after,
            "death_failures": fail_inc,
            "alive_set_false": result.instant_death,
            "concentration_broken": result.concentration_broken,
            "target_ac": result.target_ac,
        }))
        .execute(&mut **tx)
        .await?;
    if result.target_hp_after <= 0 {
        sqlx::query(
            "update combatant_effects set active = false
             where combatant_id = $1 and name = 'Rage' and active = true",
        )
        .bind(target_id)
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            "update combatants set conditions = array_remove(conditions, 'rage')
             where id = $1 and 'rage' = any(conditions)",
        )
        .bind(target_id)
        .execute(&mut **tx)
        .await?;
        crate::routes::combat::dismount_riders(&mut *tx, target_id).await?;
    }
    if let Some(chid) = target_snap.character_id {
        if result.instant_death {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', false,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                   where id = $1"#,
            )
            .bind(chid)
            .execute(&mut **tx)
            .await?;
        } else if fail_inc > 0 {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('death_saves', jsonb_build_object(
                        'successes', coalesce((sheet->'death_saves'->>'successes')::int, 0),
                        'failures', least(3,
                            coalesce((sheet->'death_saves'->>'failures')::int, 0) + $2
                        )
                   ))
                   where id = $1"#,
            )
            .bind(chid)
            .bind(fail_inc)
            .execute(&mut **tx)
            .await?;
        }
    }
    Ok(())
}
