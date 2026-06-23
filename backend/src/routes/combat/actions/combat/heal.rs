// heal — friendly-only check (HIGH-3 fix), source ownership, sheet sync.
use super::*;
use super::super::economy::require_action_auth;
use super::super::sync_combatant_hp_to_sheet;
use crate::rbac::Role;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct HealBody {
    #[validate(range(min = -1000, max = 10000))]
    pub amount: i32,
    pub source_combatant_id: Option<Uuid>,
    #[validate(length(max = 80))]
    pub label: Option<String>,
}

pub async fn heal(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<HealBody>,
) -> AppResult<Json<combat_engine::HealResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    // MED-5: auth + encounter status + round + role in one query (was 3).
    // The standard helper enforces target ownership for non-master; the
    // H4 faction check below extends it with "no enemy-faction heal".
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let role = auth.role;

    if role != Role::Master {
        // HIGH-4: target-only faction check (catches "own character placed as enemy"
        // when no source_combatant_id is provided). If the master has marked the
        // target's faction as 'enemy' (or it's an NPC and derived to 'enemy'),
        // a non-master cannot heal it. Master can override per-combatant via
        // PATCH /combatants/{id}.
        let target_faction_row: (String, String) = sqlx::query_as(
            "select faction, ref_type::text from combatants where id = $1")
            .bind(id).fetch_one(&s.db).await?;
        let derived_target = if target_faction_row.0 != "auto" {
            target_faction_row.0.clone()
        } else if target_faction_row.1 == "character" {
            "ally".to_string()
        } else {
            "enemy".to_string()
        };
        if derived_target == "enemy" {
            return Err(AppError::Forbidden);
        }
        if let Some(sid) = body.source_combatant_id {
            let source_owner: Option<Uuid> = sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?;
            if source_owner != Some(uid) {
                return Err(AppError::Forbidden);
            }
            let factions: (String, String, String, String) = sqlx::query_as(
                r#"select s.faction, s.ref_type::text, t.faction, t.ref_type::text
                   from combatants s, combatants t
                   where s.id = $1 and t.id = $2"#)
                .bind(sid).bind(id).fetch_one(&s.db).await?;
            let derived = |f: &str, r: &str| -> String {
                if f != "auto" { f.to_string() } else if r == "character" { "ally".to_string() } else { "enemy".to_string() }
            };
            if derived(&factions.0, &factions.1) != derived(&factions.2, &factions.3) {
                return Err(AppError::Forbidden);
            }
        }
    }

    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;

    let req = combat_engine::HealReq {
        amount: body.amount,
        source_combatant_id: body.source_combatant_id,
        label: body.label,
    };
    let result = combat_engine::resolve_heal(&target_snap, &req);
    let reviving_from_zero = target_snap.hp_current <= 0 && result.hp_after > 0;

    let mut tx = s.db.begin().await?;
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx)
        .await?;

    if reviving_from_zero {
        if let Some(chid) = target_snap.character_id {
            sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', true,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                   where id = $1"#,
            )
            .bind(chid)
            .execute(&mut *tx)
            .await?;
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id)
        .fetch_one(&s.db)
        .await?;

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
        .bind(format!("{} healed {} for {} HP", source_name, target_snap.display_name, result.amount))
        .bind(result.amount)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await?;

    tx.commit().await?;

    if let Err(e) =
        sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, target_snap.temp_hp).await
    {
        tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
    }

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_heals",
            "target_id": id,
            "amount": result.amount,
            // MED-12: drop hp_after (visibility leak).
            "stabilized": result.stabilized,
            "revived": reviving_from_zero,
        }),
    )
    .await;

    Ok(Json(result))
}
