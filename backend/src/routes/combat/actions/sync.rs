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
    }
    Ok(())
}

/// Sync combatant HP/temp to linked character sheet (inside a tx).
pub async fn sync_combatant_hp_to_sheet_tx(
    conn: &mut sqlx::PgConnection,
    combatant_id: Uuid,
    hp: i32,
    temp: i32,
) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac from combatants where id = $1 and ref_type = 'character'",
    )
    .bind(combatant_id)
    .fetch_optional(&mut *conn)
    .await?;
    if let Some((chid, hp_max, ac)) = row {
        let alive = hp > 0;
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
        .bind(chid).bind(hp).bind(hp_max).bind(temp).bind(ac).bind(alive)
        .execute(&mut *conn).await?;
    }
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
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits
         from combatants where id = $1"#,
    )
    .bind(id)
    .fetch_one(db)
    .await
    .map_err(|_| crate::error::AppError::NotFound)
}
