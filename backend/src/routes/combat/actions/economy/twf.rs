// two_weapon_fight — off-hand bonus-action attack.
use super::*;
use super::super::sync_combatant_hp_to_sheet;
use super::super::combat::ammo::decrement_thrown_weapon;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct TwoWeaponFightBody {
    pub target_id: Uuid,
    pub offhand_weapon_id: String,
}

pub async fn two_weapon_fight(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<TwoWeaponFightBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest(
            "attacker and target not in same encounter".into(),
        ));
    }

    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    if attacker_snap.hp_current <= 0 {
        return Err(AppError::BadRequest("cannot act while at 0 HP".into()));
    }
    let incapacitated = attacker_snap.conditions.iter().any(|c| {
        let cl = c.to_lowercase();
        cl.starts_with("incapacitated")
            || cl.starts_with("paralyzed")
            || cl.starts_with("petrified")
            || cl.starts_with("stunned")
            || cl.starts_with("unconscious")
    });
    if incapacitated {
        return Err(AppError::BadRequest(
            "cannot act while incapacitated".into(),
        ));
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let target_stats = combat_engine::compute_stats(&target_snap);

    let twf_style = attacker_snap
        .sheet_raw
        .get("features")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter().any(|f| {
                f.get("name")
                    .and_then(|v| v.as_str())
                    .map(|n| n.to_lowercase().contains("two-weapon fighting"))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false);

    let offhand_weapon = combat_engine::find_weapon(&attacker_snap, &body.offhand_weapon_id);
    let offhand_props = offhand_weapon
        .as_ref()
        .map(|(_, p)| p.clone())
        .unwrap_or_default();

    // HIGH-6: PHB p.195 — TWF requires BOTH weapons to have the light property.
    // Off-hand `light` check is enforced by `resolve_two_weapon_attack`; here
    // we additionally verify the main-hand weapon (any other weapon in the
    // sheet's weapons array) also has the light property.
    if let Some(weapons) = attacker_snap.weapons.as_array() {
        let main_hand = weapons.iter().find(|w| {
            w.get("id").and_then(|v| v.as_str()) != Some(body.offhand_weapon_id.as_str())
                && w.get("name").and_then(|v| v.as_str()) != Some(body.offhand_weapon_id.as_str())
        });
        match main_hand {
            None => {
                return Err(AppError::BadRequest(
                    "TWF requires a main-hand weapon in addition to the off-hand".into(),
                ));
            }
            Some(w) => {
                let main_light = w
                    .get("properties")
                    .and_then(|v| v.as_str())
                    .map(|p| p.to_lowercase().contains("light"))
                    .unwrap_or(false);
                if !main_light {
                    return Err(AppError::BadRequest(
                        "main-hand weapon must have the 'light' property (PHB p.195)".into(),
                    ));
                }
            }
        }
    } else {
        return Err(AppError::BadRequest(
            "TWF requires two weapons in the character sheet".into(),
        ));
    }

    if (offhand_props.ranged || offhand_props.thrown)
        && let (Some((w, _)), Some(tx), Some(ty)) =
            (&offhand_weapon, target_snap.token_x, target_snap.token_y)
        && let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y)
        && let Some(range_str) = w.get("range").and_then(|v| v.as_str())
    {
        let parts: Vec<&str> = range_str.split('/').collect();
        if parts.len() == 2 {
            if let Ok(_normal_range) = parts[0].trim().parse::<f32>() {
                if let Ok(long_range) = parts[1].trim().trim_end_matches("ft").trim().parse::<f32>()
                {
                    // HIGH-4: 1 cell = 5ft = 20% of the map.
                    let dx = (ax - tx) as f32;
                    let dy = (ay - ty) as f32;
                    let dist_ft = (dx * dx + dy * dy).sqrt() * 0.25;
                    if dist_ft > long_range {
                        return Err(AppError::BadRequest(format!(
                            "target out of off-hand weapon range ({} ft > {} ft max)",
                            dist_ft as i32, long_range as i32
                        )));
                    }
                }
            }
        }
    }

    let result = combat_engine::resolve_two_weapon_attack(
        &attacker_snap,
        &target_snap,
        &body.offhand_weapon_id,
        &attacker_stats,
        &target_stats,
        twf_style,
    )
    .map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let mut tx = s.db.begin().await?;

    let bonus_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if bonus_consumed.is_none() {
        return Err(AppError::BadRequest("bonus action already used".into()));
    }

    if let Some((w, _)) = &offhand_weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("thrown") {
            if let Some(chid) = attacker_snap.character_id {
                let _ = decrement_thrown_weapon(&mut *tx, chid, wname).await?;
            }
        }
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx)
            .await?;
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id).execute(&mut *tx).await?;
        }
    }

    let event_action = if result.hit {
        format!(
            "{} TWF {}: {} damage",
            attacker_snap.display_name, target_snap.display_name, result.damage_applied
        )
    } else {
        format!(
            "{} TWF {}: missed ({} vs AC {})",
            attacker_snap.display_name,
            target_snap.display_name,
            result.attack_total,
            result.target_ac
        )
    };
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(body.target_id)
        .bind(&event_action)
        .bind(if result.hit { -result.damage_applied } else { 0 })
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        if let Err(e) = sync_combatant_hp_to_sheet(
            &s.db,
            body.target_id,
            result.target_hp_after,
            result.target_temp_hp_after,
        )
        .await
        {
            tracing::error!(combatant_id = %body.target_id, "sync sheet HP: {e}");
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_two_weapon_fights",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_breaks": if result.hit { Some(result.concentration_broken) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
    }).to_string());

    Ok(Json(result))
}
