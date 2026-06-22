// Condition management: check_condition_immunity, add_condition, PatchEffects types.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use super::super::{has_condition, remove_condition};

pub async fn check_condition_immunity(
    db: &sqlx::PgPool,
    combatant_id: Uuid,
    condition: &str,
) -> Result<bool, crate::error::AppError> {
    let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
        r#"select n.stats, ch.sheet
           from combatants c
           left join npcs n on n.id = c.npc_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(combatant_id)
    .fetch_optional(db)
    .await?;

    let (npc_stats_raw, char_sheet) = row.unwrap_or((None, None));

    if let Some(ref raw) = npc_stats_raw {
        if let Some(npc) = combat_engine::NpcStats::from_value(raw) {
            if npc
                .condition_immunities
                .iter()
                .any(|c| c.to_lowercase() == condition)
            {
                return Ok(true);
            }
            let creature_type = raw
                .get("creature_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            if is_immune_by_type(&creature_type, condition) {
                return Ok(true);
            }
        }
    }

    if let Some(ref sheet) = char_sheet {
        if let Some(arr) = sheet.get("condition_immunities").and_then(|v| v.as_array()) {
            if arr.iter().any(|c| {
                c.as_str()
                    .map(|s| s.to_lowercase() == condition)
                    .unwrap_or(false)
            }) {
                return Ok(true);
            }
        }
        let creature_type = sheet
            .get("creature_type")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();
        if !creature_type.is_empty() && is_immune_by_type(&creature_type, condition) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_immune_by_type(creature_type: &str, condition: &str) -> bool {
    match condition {
        "poisoned" | "exhaustion" | "frightened" | "charmed" => {
            matches!(creature_type, "undead" | "construct" | "plant")
        }
        "paralyzed" | "petrified" => creature_type == "construct",
        "blinded" | "deafened" => creature_type == "plant",
        _ => false,
    }
}

#[derive(Debug, Deserialize)]
pub struct ConditionBody {
    pub condition: String,
    pub remove: Option<bool>,
    pub duration_rounds: Option<i32>,
}

pub async fn add_condition(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ConditionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, Vec<String>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, c.conditions
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, _status, owner, conditions) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }

    let condition = body.condition.to_lowercase();
    let removing = body.remove == Some(true);

    if !removing {
        let immune = check_condition_immunity(&s.db, id, &condition).await?;
        if immune {
            return Err(AppError::BadRequest(format!("immune to {}", condition)));
        }
    }
    let new_conditions: Vec<String> = if removing {
        conditions
            .into_iter()
            .filter(|c| {
                let name = c.split(':').next().unwrap_or(c).to_lowercase();
                name != condition
            })
            .collect()
    } else {
        let mut c = conditions;
        let already = c.iter().any(|existing| {
            existing
                .split(':')
                .next()
                .unwrap_or(existing)
                .to_lowercase()
                == condition
        });
        if !already {
            let entry = if let Some(dur) = body.duration_rounds.filter(|&d| d > 0) {
                format!("{}:{}", condition, dur)
            } else {
                condition.clone()
            };
            c.push(entry);
        }
        c
    };

    let breaks_concentration = !removing
        && matches!(
            condition.as_str(),
            "incapacitated" | "paralyzed" | "stunned" | "unconscious"
        );
    let releases_grapple = !removing
        && matches!(
            condition.as_str(),
            "incapacitated" | "paralyzed" | "stunned" | "unconscious" | "dead"
        );

    let mut tx = s.db.begin().await?;

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set conditions = $1 where id = $2
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
    )
    .bind(&new_conditions)
    .bind(id)
    .fetch_one(&mut *tx).await?;

    if breaks_concentration {
        sqlx::query(
            "update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true"
        )
        .bind(id)
        .execute(&mut *tx).await?;
    }

    if releases_grapple {
        let grappler_conds: Vec<String> =
            sqlx::query_scalar("select conditions from combatants where id = $1")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .unwrap_or_default();
        if has_condition(&grappler_conds, "grappling") {
            let freed = remove_condition(grappler_conds, "grappling");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&freed)
                .bind(id)
                .execute(&mut *tx)
                .await?;
            let enc_combatants: Vec<(Uuid, Vec<String>)> = sqlx::query_as(
                "select id, conditions from combatants
                 where encounter_id = (select encounter_id from combatants where id = $1)
                   and id != $1",
            )
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
            for (gid, gconds) in enc_combatants {
                if has_condition(&gconds, "grappled") {
                    let new_gconds = remove_condition(gconds, "grappled");
                    sqlx::query("update combatants set conditions = $1 where id = $2")
                        .bind(&new_gconds)
                        .bind(gid)
                        .execute(&mut *tx)
                        .await?;
                    ws::publish(
                        campaign_id,
                        json!({
                            "type": "combatant_loses_condition",
                            "combatant_id": gid,
                            "condition": "grappled",
                            "reason": "grappler incapacitated",
                        })
                        .to_string(),
                    );
                }
            }
        }
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": if removing { "combatant_loses_condition" } else { "combatant_gains_condition" },
        "combatant_id": id,
        "condition": body.condition,
    }).to_string());

    if breaks_concentration {
        ws::publish(
            campaign_id,
            json!({
                "type": "concentration_breaks",
                "combatant_id": id,
                "reason": condition,
            })
            .to_string(),
        );
    }

    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct PatchEffectsBody {
    pub combatant_ids: Vec<Uuid>,
    pub remove_by_name: Option<String>,
    pub set_active: Option<bool>,
    pub add_effect: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct PatchEffectsResult {
    pub affected: usize,
}

#[cfg(test)]
mod tests {
    use super::is_immune_by_type;

    // PHB: undead immune to poison, exhaustion, frightened, charmed.
    // PHB: construct immune to poison, exhaustion, frightened, charmed, paralyzed, petrified.
    // PHB: plant immune to poison, exhaustion, frightened, charmed, blinded, deafened.
    #[test]
    fn undead_immune_to_poison_exhaustion_frightened_charmed() {
        for cond in &["poisoned", "exhaustion", "frightened", "charmed"] {
            assert!(
                is_immune_by_type("undead", cond),
                "undead should be immune to {cond}"
            );
        }
    }

    #[test]
    fn construct_immune_to_paralyzed_and_petrified() {
        assert!(is_immune_by_type("construct", "paralyzed"));
        assert!(is_immune_by_type("construct", "petrified"));
    }

    #[test]
    fn plant_immune_to_blinded_and_deafened() {
        assert!(is_immune_by_type("plant", "blinded"));
        assert!(is_immune_by_type("plant", "deafened"));
    }

    #[test]
    fn humanoid_not_immune_to_any() {
        for cond in &["poisoned", "blinded", "charmed", "paralyzed", "stunned"] {
            assert!(
                !is_immune_by_type("humanoid", cond),
                "humanoid should NOT be immune to {cond}"
            );
        }
    }

    #[test]
    fn non_type_specific_conditions_unaffected_by_type() {
        // Restrained, prone, stunned are NOT in the immunity table.
        for ct in &["undead", "construct", "plant", "humanoid"] {
            for cond in &["restrained", "prone", "stunned"] {
                assert!(
                    !is_immune_by_type(ct, cond),
                    "{ct} should NOT be immune to {cond} (creature-type immune list)"
                );
            }
        }
    }
}
