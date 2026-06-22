// deal_damage — explicit damage application endpoint.
use super::*;
use crate::rbac::Role;
use super::super::sync_combatant_hp_to_sheet;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct DamageBody {
    #[validate(range(min = -1000, max = 10000))]
    pub amount: i32,
    #[validate(length(min = 1, max = 32))]
    pub damage_type: String,
    pub source_combatant_id: Option<Uuid>,
    #[validate(length(max = 80))]
    pub label: Option<String>,
    pub is_magical: bool,
}

pub async fn deal_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DamageBody>,
) -> AppResult<Json<combat_engine::DamageResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    // deal_damage keeps its bespoke auth: non-master can deal damage if they
    // own EITHER the target OR the source (so a player can cast Magic Missile
    // from their Wizard at an enemy combatant they don't own). The standard
    // require_action_auth enforces target-only ownership, which would
    // regress this case. The owner check stays as 2 separate queries.
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
        let source_owner: Option<Uuid> = if let Some(sid) = body.source_combatant_id {
            sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?
        } else {
            None
        };
        if owner != Some(uid) && source_owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

    let target_stats = combat_engine::compute_stats(&target_snap);
    let req = combat_engine::DamageReq {
        amount: body.amount,
        damage_type: body.damage_type,
        source_combatant_id: body.source_combatant_id,
        label: body.label,
        is_magical: body.is_magical,
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
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(id).execute(&mut *tx).await?;
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

    // MED-7: PHB p.197 — any damage taken while at 0 HP = 1 death-save failure.
    // Instant death already set failures=3 above; this branch is for non-instant.
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

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid)
            .fetch_optional(&s.db)
            .await?
            .unwrap_or_else(|| "Unknown".into())
    } else {
        "DM".into()
    };

    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!("{} dealt {} {} damage to {}{}", source_name, result.damage_applied, req.damage_type, target_snap.display_name, if result.instant_death { " — INSTANT DEATH" } else { "" }))
        .bind(-result.damage_applied)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) =
        sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await
    {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_damages",
            "target_id": id,
            "damage": result.damage_applied,
            // MED-12: drop hp_after/temp_hp_after (visibility leak).
            "concentration_breaks": result.concentration_broken,
            "instant_death": result.instant_death,
        })
        .to_string(),
    );

    Ok(Json(result))
}
