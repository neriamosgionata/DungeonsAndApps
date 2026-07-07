// DB loading helpers: load_snapshot + load_snapshots_batch.
// Extracted from combat_engine.rs to keep the file under the 500-line
// guideline (per AGENTS.md §1.4).

use super::types::{CombatantSnapshot, EffectSnapshot, NpcStats};
use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct SnapRow {
    id: uuid::Uuid,
    encounter_id: uuid::Uuid,
    display_name: String,
    character_id: Option<uuid::Uuid>,
    npc_id: Option<uuid::Uuid>,
    hp_current: i32,
    hp_max: i32,
    temp_hp: i32,
    ac: i32,
    level_override: i32,
    token_x: Option<f32>,
    token_y: Option<f32>,
    abilities: serde_json::Value,
    saves: serde_json::Value,
    skills: serde_json::Value,
    casting: serde_json::Value,
    conditions: Vec<String>,
    weapons: serde_json::Value,
    level_total: i32,
    equipment: serde_json::Value,
    npc_stats_raw: Option<serde_json::Value>,
    race: Option<String>,
    classes: serde_json::Value,
    sheet_raw: serde_json::Value,
}

pub async fn load_snapshot(
    db: &PgPool,
    combatant_id: uuid::Uuid,
) -> Result<CombatantSnapshot, crate::error::AppError> {
    let row: SnapRow = sqlx::query_as(
        r#"select
            c.id, c.encounter_id, c.display_name, c.character_id, c.npc_id,
            c.hp_current, c.hp_max, c.temp_hp, c.ac, c.level_override,
            c.token_x, c.token_y,
            coalesce(ch.sheet->'abilities', n.stats->'abilities', '{}'::jsonb) as abilities,
            coalesce(ch.sheet->'saves', n.stats->'saves', '{}'::jsonb) as saves,
            coalesce(ch.sheet->'skills', n.stats->'skills', '{}'::jsonb) as skills,
            coalesce(ch.sheet->'casting', n.stats->'casting', '{}'::jsonb) as casting,
            c.conditions,
            coalesce(ch.sheet->'weapons', n.stats->'weapons', '[]'::jsonb) as weapons,
            coalesce((ch.sheet->>'level_total')::int, (n.stats->>'pb')::int, 1) as level_total,
            coalesce(ch.sheet->'equipment', n.stats->'equipment', '[]'::jsonb) as equipment,
            n.stats as npc_stats_raw,
            ch.race,
            coalesce(ch.sheet->'classes', n.stats->'classes', '[]'::jsonb) as classes,
            coalesce(ch.sheet, n.stats, '{}'::jsonb) as sheet_raw
         from combatants c
         left join characters ch on ch.id = c.character_id
         left join npcs n on n.id = c.npc_id
         where c.id = $1"#,
    )
    .bind(combatant_id)
    .fetch_optional(db)
    .await?
    .ok_or(crate::error::AppError::NotFound)?;

    let effects: Vec<(uuid::Uuid, String, serde_json::Value, bool, String, Option<uuid::Uuid>)> = sqlx::query_as(
        r#"select id, name, modifiers, concentration, source_type::text, caster_combatant_id
           from combatant_effects
           where combatant_id = $1 and active = true"#,
    )
    .bind(combatant_id)
    .fetch_all(db)
    .await?;

    // If this is an NPC combatant (no linked character), parse structured stats
    // and use them to populate the snapshot fields.
    let npc_stats = row.npc_stats_raw.as_ref().and_then(NpcStats::from_value);
    let is_npc = row.character_id.is_none() && row.npc_id.is_some();

    let abilities = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.abilities_value())
            .unwrap_or(row.abilities)
    } else {
        row.abilities
    };
    let saves = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.saves_value())
            .unwrap_or(row.saves)
    } else {
        row.saves
    };
    let skills = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.skills_value())
            .unwrap_or(row.skills)
    } else {
        row.skills
    };
    let casting = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.casting_value())
            .unwrap_or(row.casting)
    } else {
        row.casting
    };
    let weapons = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.weapons_value())
            .unwrap_or(row.weapons)
    } else {
        row.weapons
    };
    let equipment = if is_npc {
        npc_stats
            .as_ref()
            .map(|n| n.equipment_value())
            .unwrap_or(row.equipment)
    } else {
        row.equipment
    };

    let base_speed = if is_npc {
        npc_stats.as_ref().map(|n| n.speed).unwrap_or(30)
    } else {
        row.sheet_raw
            .get("speed")
            .and_then(|v| v.as_i64())
            .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            .unwrap_or(30)
    };

    let level_total = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
        if stats.pb > 0 {
            // Use proficiency bonus directly; the combat engine only uses pb for
            // save/skill/spell calculations, so we can set a synthetic level.
            // pb=2 → level 1-4, pb=3 → 5-8, etc.
            let pb = stats.pb;
            ((pb - 2) * 4 + 1).max(1)
        } else {
            // L2: clamp to i16 range (level_override column is i16).
            row.level_total.clamp(i16::MIN as i32, i16::MAX as i32)
        }
    } else {
        // L2: clamp to i16 range (level_override column is i16).
        row.level_total.clamp(i16::MIN as i32, i16::MAX as i32)
    };

    let proficiency_bonus = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
        if stats.pb > 0 {
            stats.pb
        } else {
            row.level_override
        }
    } else {
        row.level_override
    };

    let race = if is_npc { None } else { row.race };
    let classes = if is_npc { row.classes } else { row.classes };
    let sheet_raw = if is_npc {
        row.npc_stats_raw.unwrap_or(row.sheet_raw)
    } else {
        row.sheet_raw
    };

    Ok(CombatantSnapshot {
        id: row.id,
        encounter_id: row.encounter_id,
        display_name: row.display_name,
        character_id: row.character_id,
        npc_id: row.npc_id,
        hp_current: row.hp_current,
        hp_max: row.hp_max,
        temp_hp: row.temp_hp,
        base_ac: row.ac,
        base_speed: base_speed.max(0),
        level_total,
        token_x: row.token_x,
        token_y: row.token_y,
        abilities,
        saves,
        skills,
        proficiency_bonus,
        conditions: row.conditions,
        active_effects: effects
            .into_iter()
            .map(|(id, name, mods, conc, st, src)| EffectSnapshot {
                id,
                name,
                modifiers: mods,
                concentration: conc,
                source_type: st,
                source_combatant_id: src,
            })
            .collect(),
        casting,
        weapons,
        equipment,
        race,
        classes,
        sheet_raw,
    })
}

pub async fn load_snapshots_batch(
    db: &PgPool,
    combatant_ids: &[uuid::Uuid],
) -> Result<std::collections::HashMap<uuid::Uuid, CombatantSnapshot>, crate::error::AppError> {
    if combatant_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let rows: Vec<SnapRow> = sqlx::query_as(
        r#"select
            c.id, c.encounter_id, c.display_name, c.character_id, c.npc_id,
            c.hp_current, c.hp_max, c.temp_hp, c.ac, c.level_override,
            c.token_x, c.token_y,
            coalesce(ch.sheet->'abilities', n.stats->'abilities', '{}'::jsonb) as abilities,
            coalesce(ch.sheet->'saves', n.stats->'saves', '{}'::jsonb) as saves,
            coalesce(ch.sheet->'skills', n.stats->'skills', '{}'::jsonb) as skills,
            coalesce(ch.sheet->'casting', n.stats->'casting', '{}'::jsonb) as casting,
            c.conditions,
            coalesce(ch.sheet->'weapons', n.stats->'weapons', '[]'::jsonb) as weapons,
            coalesce((ch.sheet->>'level_total')::int, (n.stats->>'pb')::int, 1) as level_total,
            coalesce(ch.sheet->'equipment', n.stats->'equipment', '[]'::jsonb) as equipment,
            n.stats as npc_stats_raw,
            ch.race,
            coalesce(ch.sheet->'classes', n.stats->'classes', '[]'::jsonb) as classes,
            coalesce(ch.sheet, n.stats, '{}'::jsonb) as sheet_raw
         from combatants c
         left join characters ch on ch.id = c.character_id
         left join npcs n on n.id = c.npc_id
         where c.id = ANY($1)"#,
    )
    .bind(combatant_ids)
    .fetch_all(db)
    .await?;

    let effects_rows: Vec<(
        uuid::Uuid,
        uuid::Uuid,
        String,
        serde_json::Value,
        bool,
        String,
        Option<uuid::Uuid>,
    )> = sqlx::query_as(
        r#"select combatant_id, id, name, modifiers, concentration, source_type::text, caster_combatant_id
           from combatant_effects
           where combatant_id = ANY($1) and active = true"#,
    )
    .bind(combatant_ids)
    .fetch_all(db)
    .await?;

    let mut effects_map: std::collections::HashMap<uuid::Uuid, Vec<EffectSnapshot>> =
        std::collections::HashMap::new();
    for (cid, id, name, mods, conc, st, src) in effects_rows {
        effects_map.entry(cid).or_default().push(EffectSnapshot {
            id,
            name,
            modifiers: mods,
            concentration: conc,
            source_type: st,
            source_combatant_id: src,
        });
    }

    let mut results = std::collections::HashMap::new();
    for row in rows {
        let npc_stats = row.npc_stats_raw.as_ref().and_then(NpcStats::from_value);
        let is_npc = row.character_id.is_none() && row.npc_id.is_some();

        let abilities = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.abilities_value())
                .unwrap_or(row.abilities)
        } else {
            row.abilities
        };
        let saves = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.saves_value())
                .unwrap_or(row.saves)
        } else {
            row.saves
        };
        let skills = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.skills_value())
                .unwrap_or(row.skills)
        } else {
            row.skills
        };
        let casting = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.casting_value())
                .unwrap_or(row.casting)
        } else {
            row.casting
        };
        let weapons = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.weapons_value())
                .unwrap_or(row.weapons)
        } else {
            row.weapons
        };
        let equipment = if is_npc {
            npc_stats
                .as_ref()
                .map(|n| n.equipment_value())
                .unwrap_or(row.equipment)
        } else {
            row.equipment
        };
        let base_speed = if is_npc {
            npc_stats.as_ref().map(|n| n.speed).unwrap_or(30)
        } else {
            row.sheet_raw
                .get("speed")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(30)
        };

        let level_total = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
            if stats.pb > 0 {
                let pb = stats.pb;
                ((pb - 2) * 4 + 1).max(1)
            } else {
                row.level_total
            }
        } else {
            row.level_total
        };

        let proficiency_bonus = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
            if stats.pb > 0 {
                stats.pb
            } else {
                row.level_override
            }
        } else {
            row.level_override
        };

        let race = if is_npc { None } else { row.race };
        let classes = row.classes;
        let sheet_raw = if is_npc {
            row.npc_stats_raw.unwrap_or(row.sheet_raw)
        } else {
            row.sheet_raw
        };

        let effects = effects_map.remove(&row.id).unwrap_or_default();
        results.insert(
            row.id,
            CombatantSnapshot {
                id: row.id,
                encounter_id: row.encounter_id,
                display_name: row.display_name,
                character_id: row.character_id,
                npc_id: row.npc_id,
                hp_current: row.hp_current,
                hp_max: row.hp_max,
                temp_hp: row.temp_hp,
                base_ac: row.ac,
                base_speed: base_speed.max(0),
                level_total,
                token_x: row.token_x,
                token_y: row.token_y,
                abilities,
                saves,
                skills,
                proficiency_bonus,
                conditions: row.conditions,
                active_effects: effects,
                casting,
                weapons,
                equipment,
                race,
                classes,
                sheet_raw,
            },
        );
    }

    Ok(results)
}
