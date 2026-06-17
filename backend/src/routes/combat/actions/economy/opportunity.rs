// opportunity_attack — free reaction attack with reach + wall + line-of-effect checks.
use super::*;
use super::auth::consume_action_or_bonus;
use super::super::sync_combatant_hp_to_sheet;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct OppAttackBody {
    pub target_id: Uuid,
}

pub async fn opportunity_attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<OppAttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    if attacker_stats.incapacitated {
        return Err(AppError::BadRequest("attacker is incapacitated".into()));
    }

    let target_stats = combat_engine::compute_stats(&target_snap);
    let has_disengage = target_snap.active_effects.iter().any(|e| {
        e.modifiers
            .as_object()
            .map(|m| m.get("disengage").and_then(|v| v.as_bool()) == Some(true))
            .unwrap_or(false)
    });
    if has_disengage {
        return Err(AppError::BadRequest("target has disengaged".into()));
    }

    // PHB p.195: OA range = melee reach (5ft, 10ft for reach weapons). If both tokens
    // are placed, reject if target is beyond the attacker's reach. OA also requires
    // line of effect: a wall obstacle between attacker and target blocks the OA.
    let attacker_reach_ft: f32 = if attacker_snap
        .weapons
        .as_array()
        .and_then(|arr| {
            arr.iter().find(|w| {
                let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
                props
                    .split(',')
                    .any(|p| p.trim().eq_ignore_ascii_case("reach"))
            })
        })
        .is_some()
    {
        10.0
    } else {
        5.0
    };
    if let (Some(ax), Some(ay), Some(tx), Some(ty)) = (
        attacker_snap.token_x,
        attacker_snap.token_y,
        target_snap.token_x,
        target_snap.token_y,
    ) {
        // MED-6: combine map_grid_size + walls in one query (was 2 RT).
        let row: (i32, Vec<(f32, f32, f32, f32)>) = sqlx::query_as(
            r#"select e.map_grid_size,
                      coalesce(array_agg(row(o.origin_x, o.origin_y,
                                            coalesce(o.end_x, o.origin_x),
                                            coalesce(o.end_y, o.origin_y + 5))
                                         order by o.id)
                              filter (where o.id is not null),
                              '{}'::_float4[]) as walls
               from encounters e
               left join encounter_overlays o
                 on o.encounter_id = e.id
                and o.active = true
                and o.zone_type = 'wall'
                and o.shape = 'line'
               where e.id = $1
               group by e.map_grid_size"#,
        )
        .bind(attacker_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
        let (g_size, walls) = row;
        let cell_pct = (g_size as f32) / 6.0;
        let dist_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
        let dist_ft = dist_pct / cell_pct * 5.0;
        if dist_ft > attacker_reach_ft {
            return Err(AppError::BadRequest(format!(
                "opportunity attack out of reach ({} ft > {} ft)",
                dist_ft as i32, attacker_reach_ft as i32
            )));
        }
        for (wx1, wy1, wx2, wy2) in &walls {
            if super::super::super::tactical::positioning::segments_intersect(ax, ay, tx, ty, *wx1, *wy1, *wx2, *wy2) {
                return Err(AppError::BadRequest(
                    "opportunity attack blocked by wall obstacle".into(),
                ));
            }
        }
    }

    let req = combat_engine::AttackReq {
        target_id: body.target_id,
        attack_expression: None,
        damage_expression: None,
        damage_type: "bludgeoning".to_string(),
        damage_die: None,
        ability: Some("str".to_string()),
        proficient: Some(true),
        advantage: false,
        disadvantage: false,
        cover: None,
        is_spell_attack: false,
        is_magical: false,
        label: Some("Opportunity Attack".to_string()),
        weapon_id: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    let result = combat_engine::resolve_attack(
        &attacker_snap,
        &target_snap,
        &req,
        &attacker_stats,
        &target_stats,
    )
    .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    let reaction_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set reaction_used = true where id = $1 and reaction_used = false and hp_current > 0 returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if reaction_consumed.is_none() {
        return Err(AppError::BadRequest("reaction already used".into()));
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
        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#,
                )
                .bind(chid)
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

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

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_opportunity_attacks",
            "attacker_id": id,
            "target_id": body.target_id,
            "hit": result.hit,
            "damage": result.damage_applied,
            "instant_death": result.instant_death,
        })
        .to_string(),
    );

    Ok(Json(result))
}
