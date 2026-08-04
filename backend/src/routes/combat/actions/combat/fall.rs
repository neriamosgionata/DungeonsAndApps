// falling damage — POST /combatants/{id}/fall (PHB p.183: 1d6 bludgeoning
// per 10 feet fallen, minimum 10 ft to take damage).
use super::*;
use super::super::sync_combatant_hp_to_sheet;
use crate::AppState;
use crate::rbac::Role;
use axum::Json;
use axum::extract::{Path, State};
use rand::SeedableRng;
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct FallBody {
    #[validate(range(min = 0, max = 2000))]
    pub distance_ft: i32,
    pub source_combatant_id: Option<Uuid>,
}

pub async fn fall(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<FallBody>,
) -> AppResult<Json<combat_engine::DamageResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id)
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
    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let target_stats = combat_engine::compute_stats(&target_snap);
    if target_stats.exhaustion_dead {
        return Err(AppError::BadRequest("target is dead".into()));
    }

    // A16: 1d6 bludgeoning per 10 ft, rounded down (PHB p.183).
    let dice = body.distance_ft / 10;
    let amount = if dice > 0 {
        let mut rng = rand::rngs::StdRng::from_os_rng();
        crate::dice::roll(&format!("{}d6", dice), &mut rng)
            .map_err(|e| AppError::BadRequest(e.to_string()))?
            .total
    } else {
        0
    };

    let req = combat_engine::DamageReq {
        amount,
        damage_type: "bludgeoning".into(),
        source_combatant_id: body.source_combatant_id,
        label: Some(format!("falling {} ft", body.distance_ft)),
        is_magical: false,
    };
    let result = combat_engine::resolve_damage(&target_snap, &req, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
        .bind(result.hp_after)
        .bind(result.temp_hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;
    if result.concentration_broken {
        sqlx::query(
            "update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
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
    if !result.instant_death
        && target_snap.hp_current <= 0
        && result.damage_applied > 0
        && result.hp_after <= 0
        && let Some(chid) = target_snap.character_id
    {
        sqlx::query(
            r#"update characters set sheet =
                coalesce(sheet, '{}'::jsonb)
                || jsonb_build_object(
                    'death_saves', jsonb_build_object(
                        'successes', coalesce((sheet->'death_saves'->>'successes')::int, 0),
                        'failures', least(3,
                            coalesce((sheet->'death_saves'->>'failures')::int, 0) + 1
                        )
                    )
                )
               where id = $1"#,
        )
        .bind(chid)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!(
            "{} fell {} ft and took {} bludgeoning damage",
            target_snap.display_name, body.distance_ft, result.damage_applied
        ))
        .bind(-result.damage_applied)
        .bind(result.damage_applied as i32 > 0)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    if let Err(e) =
        sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await
    {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_falls",
            "target_id": id,
            "distance_ft": body.distance_ft,
            "damage": result.damage_applied,
            "concentration_breaks": result.concentration_broken,
            "instant_death": result.instant_death,
        }),
    )
    .await;

    Ok(Json(result))
}
