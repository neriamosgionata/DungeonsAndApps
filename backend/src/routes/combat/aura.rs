// Aura of Protection (PHB p.85): a Paladin 6+ grants its CHA modifier to
// the saving throws of allies within 10 ft (30 ft at level 18). M21 — the
// previous implementation added CHA to the paladin's own saves
// unconditionally (self-only, no radius). This resolves the bonus against
// the live encounter: every character-bound combatant in the encounter
// counts, ranged by token distance (same 5 ft = 20% convention as the
// attack handler).
use crate::combat_engine::{CombatantSnapshot, apply_racial_bonuses};
use crate::error::AppResult;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(sqlx::FromRow)]
struct AuraRow {
    id: Uuid,
    token_x: Option<f32>,
    token_y: Option<f32>,
    race: Option<String>,
    classes: serde_json::Value,
    abilities: serde_json::Value,
    abilities_override: serde_json::Value,
}

/// Highest CHA mod among friendly paladins (6+) within aura range of the
/// target. 0 when the target is hostile or no paladin is in range.
/// Tokens not placed on the map → assumed in range (theater of mind,
/// mirroring the within-5-ft fallback in the attack resolver).
pub async fn aura_of_protection_bonus(
    db: &PgPool,
    target_id: Uuid,
    target_encounter: Uuid,
    target_x: Option<f32>,
    target_y: Option<f32>,
) -> AppResult<i32> {
    // H-8: exclude hostiles the same way heal.rs derives sides — the
    // default faction is 'auto' (migration 20260617000001) and auto+NPC
    // resolves to enemy; only literal 'hostile' was blocked before, so
    // every default enemy NPC near a paladin got the aura.
    let target_side: (Option<String>, String) = sqlx::query_as(
        "select faction, ref_type::text from combatants where id = $1",
    )
    .bind(target_id)
    .fetch_optional(db)
    .await?
    .ok_or(crate::error::AppError::NotFound)?;
    let (faction, ref_type) = target_side;
    let hostile = faction.as_deref() == Some("hostile")
        || faction.as_deref() == Some("enemy")
        || (faction.as_deref() == Some("auto") && ref_type == "npc");
    if hostile {
        return Ok(0);
    }

    let rows: Vec<AuraRow> = sqlx::query_as(
        "select c.id, c.token_x, c.token_y, ch.race,
                coalesce(ch.sheet->'classes', '[]'::jsonb) as classes,
                coalesce(ch.sheet->'abilities', '{}'::jsonb) as abilities,
                coalesce(ch.sheet->'abilities_override', '{}'::jsonb) as abilities_override
         from combatants c
         join characters ch on ch.id = c.character_id
         where c.encounter_id = $1",
    )
    .bind(target_encounter)
    .fetch_all(db)
    .await?;

    let mut best = 0i32;
    for r in rows {
        // H-8: the paladin's own CHA is already folded into its save_mods
        // (compute_stats); counting itself here would double-dip.
        if r.id == target_id {
            continue;
        }
        let pal_level: i32 = r
            .classes
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|c| {
                        c.get("name")
                            .and_then(|n| n.as_str())
                            .map(|n| n.eq_ignore_ascii_case("paladin"))
                            .unwrap_or(false)
                    })
                    .filter_map(|c| c.get("level").and_then(|l| l.as_i64()))
                    .sum::<i64>() as i32
            })
            .unwrap_or(0);
        if pal_level < 6 {
            continue;
        }
        let aura_ft = if pal_level >= 18 { 30.0 } else { 10.0 };
        match (target_x, target_y, r.token_x, r.token_y) {
            (Some(cx), Some(cy), Some(px), Some(py)) => {
                let dx = (cx - px) as f32;
                let dy = (cy - py) as f32;
                // 1 cell = 5 ft = 20% of the map; dist_pct × 0.25 → feet.
                let dist_ft = (dx * dx + dy * dy).sqrt() * 0.25;
                if dist_ft > aura_ft + 2.5 {
                    continue;
                }
            }
            _ => {}
        }
        let cha = r
            .abilities_override
            .get("cha")
            .and_then(|v| v.as_i64())
            .unwrap_or_else(|| {
                let base = r.abilities.get("cha").and_then(|v| v.as_i64()).unwrap_or(10);
                let mut snap = CombatantSnapshot::default();
                snap.race = r.race.clone();
                let racial = apply_racial_bonuses(&snap).get("cha").copied().unwrap_or(0);
                (base + racial as i64).clamp(1, 30)
            })
            .clamp(1, 30);
        let cha_mod = ((cha - 10) as f32 / 2.0).floor() as i32;
        best = best.max(cha_mod);
    }
    Ok(best)
}
