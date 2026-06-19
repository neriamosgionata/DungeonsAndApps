// parse_npc_multiattack, try_parse_npc_multiattack, parse_multiattack.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct ParsedMultiAttack {
    pub attacks: Vec<ParsedSubAttack>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ParsedSubAttack {
    pub name: String,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    #[serde(default)]
    pub damage_type: String,
    pub label: Option<String>,
}

pub fn parse_npc_multiattack(
    description: &str,
    actions: &[serde_json::Value],
) -> Vec<ParsedSubAttack> {
    let desc = description.to_lowercase();
    let mut attack_names: Vec<(u32, String)> = Vec::new();

    if desc.contains('+') || desc.chars().filter(|&c| c.is_ascii_digit()).count() > 0 {
        for part in desc.split('+') {
            let part = part.trim();
            let (cnt, nm): (u32, String) =
                if let Some(d) = part.chars().next().and_then(|c| c.to_digit(10)) {
                    (
                        d,
                        part.chars().skip(1).collect::<String>().trim().to_string(),
                    )
                } else {
                    let words: Vec<&str> = part.split_whitespace().collect();
                    if words.len() >= 2 {
                        let c = match words[0] {
                            "one" | "a" | "an" => 1,
                            "two" => 2,
                            "three" => 3,
                            "four" => 4,
                            "five" => 5,
                            _ => 1,
                        };
                        (c, words[1..].join(" "))
                    } else {
                        (1, part.to_string())
                    }
                };
            if !nm.is_empty() {
                attack_names.push((cnt, nm));
            }
        }
    }

    if attack_names.is_empty() {
        if let Some(attacks_part) = desc.split(':').nth(1) {
            for segment in attacks_part.split(',') {
                let seg = segment.trim();
                for prefix in &["one with its ", "one with his ", "one with her ", "one "] {
                    if let Some(rest) = seg.strip_prefix(prefix) {
                        let name = rest.trim_end_matches(&['.', ' '][..]).to_string();
                        if !name.is_empty() {
                            attack_names.push((1, name));
                        }
                        break;
                    }
                }
            }
        }
    }

    if attack_names.is_empty() {
        let p3_count = desc
            .split_whitespace()
            .find_map(|w| w.chars().next().and_then(|c| c.to_digit(10)))
            .unwrap_or(1);
        if let Some(first_atk) = actions.iter().find(|a| {
            let aname = a
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            aname != "multiattack"
        }) {
            let aname = first_atk
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("attack")
                .to_string();
            attack_names.push((p3_count, aname));
        }
    }

    let mut results: Vec<ParsedSubAttack> = Vec::new();
    let actions_lower: Vec<(String, &serde_json::Value)> = actions
        .iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str())?;
            Some((name.to_lowercase(), a))
        })
        .collect();

    for (count, name_hint) in attack_names {
        let hint = name_hint.trim().to_lowercase();
        let found = actions_lower
            .iter()
            .find(|(n, _)| *n == hint)
            .or_else(|| {
                actions_lower
                    .iter()
                    .find(|(n, _)| n.contains(&hint) || hint.contains(n))
            })
            .or_else(|| actions_lower.iter().find(|(n, _)| n != &"multiattack"));

        if let Some((_, action)) = found {
            let atk_bonus = action
                .get("attack_bonus")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let dam = action
                .get("damage")
                .and_then(|v| v.as_str())
                .unwrap_or("1d4");
            let dtype = action
                .get("damage_type")
                .and_then(|v| v.as_str())
                .unwrap_or("bludgeoning");
            let aname = action
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Attack");
            for _ in 0..count {
                results.push(ParsedSubAttack {
                    name: aname.to_string(),
                    attack_expression: Some(format!("1d20+{}", atk_bonus)),
                    damage_expression: Some(dam.to_string()),
                    damage_type: dtype.to_string(),
                    label: Some(aname.to_string()),
                });
            }
        }
    }

    results
}

pub async fn try_parse_npc_multiattack(
    db: &sqlx::PgPool,
    combatant_id: Uuid,
) -> Result<ParsedMultiAttack, String> {
    let npc_id: Option<Uuid> = sqlx::query_scalar("select npc_id from combatants where id = $1")
        .bind(combatant_id)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .flatten()
        .ok_or_else(|| "not an NPC combatant".to_string())?;

    let npc_stats: Option<serde_json::Value> =
        sqlx::query_scalar("select stats from npcs where id = $1")
            .bind(npc_id)
            .fetch_optional(db)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "NPC not found".to_string())?;

    let stats = npc_stats.ok_or_else(|| "NPC has no stats".to_string())?;
    let actions: Vec<serde_json::Value> = stats
        .get("actions")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let multiattack_action = actions
        .iter()
        .find(|a| {
            a.get("name")
                .and_then(|v| v.as_str())
                .map(|n| n.to_lowercase() == "multiattack")
                .unwrap_or(false)
        })
        .ok_or_else(|| "NPC has no Multiattack action".to_string())?;

    let description = multiattack_action
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if description.is_empty() {
        return Err("Multiattack action has no description".to_string());
    }

    let attacks = parse_npc_multiattack(description, &actions);
    if attacks.is_empty() {
        return Err(format!(
            "could not parse multiattack description: {}",
            description
        ));
    }

    Ok(ParsedMultiAttack { attacks })
}

pub async fn parse_multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ParsedMultiAttack>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        r#"select e.campaign_id from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let parsed = try_parse_npc_multiattack(&s.db, id)
        .await
        .map_err(|e| AppError::BadRequest(e))?;
    Ok(Json(parsed))
}
