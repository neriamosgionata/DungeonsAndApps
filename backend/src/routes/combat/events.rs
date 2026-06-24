use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac, ws,
};
use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Serialize, FromRow)]
pub struct CombatEvent {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub round: i32,
    pub actor_combatant: Option<Uuid>,
    pub target_combatant: Option<Uuid>,
    pub action: String,
    pub roll_id: Option<Uuid>,
    pub delta_hp: Option<i32>,
    pub note: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
pub struct EventListQ {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

pub async fn list_events(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Query(q): Query<EventListQ>,
) -> AppResult<Json<Vec<CombatEvent>>> {
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let rows: Vec<CombatEvent> = sqlx::query_as::<_, CombatEvent>(
        "select id, encounter_id, round, actor_combatant, target_combatant, action, roll_id, delta_hp, note, created_at
         from combat_events where encounter_id = $1 order by created_at desc limit $2 offset $3")
        .bind(encounter_id).bind(limit).bind(offset).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

pub async fn delete_event(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(event_id): Path<Uuid>,
) -> AppResult<Json<()>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        r#"select e.campaign_id from combat_events ce
           join encounters e on e.id = ce.encounter_id
           where ce.id = $1"#,
    )
    .bind(event_id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    sqlx::query("delete from combat_events where id = $1")
        .bind(event_id)
        .execute(&s.db)
        .await?;
    Ok(Json(()))
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

pub async fn patch_effects(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<PatchEffectsBody>,
) -> AppResult<Json<PatchEffectsResult>> {
    let (campaign_id, enc_round, enc_turn_index): (Uuid, i32, i32) =
        sqlx::query_as("select campaign_id, round, turn_index from encounters where id = $1")
            .bind(encounter_id)
            .fetch_one(&s.db)
            .await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    // M-P2: wrap in tx + batch each branch via unnest. Pre-fix: 3 separate
    // per-row loops in autocommit. 50 combatants = 150 round-trips, not
    // atomic (partial state visible if commit fails mid-loop). Post-fix:
    // 1 tx with 3 batched queries + 1 batched WS event.
    let mut tx = s.db.begin().await?;

    let mut affected = 0usize;

    if let Some(ref name) = body.remove_by_name {
        let r = sqlx::query(
            "update combatant_effects set active = false
             where name = $1 and combatant_id = ANY($2::uuid[]) and active = true")
            .bind(name).bind(&body.combatant_ids).execute(&mut *tx).await?;
        affected += r.rows_affected() as usize;
    }

    if let Some(active) = body.set_active {
        let r = if let Some(ref name) = body.remove_by_name {
            sqlx::query(
                "update combatant_effects set active = $1
                 where combatant_id = ANY($2::uuid[]) and name = $3")
                .bind(active).bind(&body.combatant_ids).bind(name)
                .execute(&mut *tx).await?
        } else {
            sqlx::query(
                "update combatant_effects set active = $1
                 where combatant_id = ANY($2::uuid[]) and active != $1")
                .bind(active).bind(&body.combatant_ids)
                .execute(&mut *tx).await?
        };
        affected += r.rows_affected() as usize;
    }

    if let Some(ref eff) = body.add_effect {
        let name = eff.get("name").and_then(|v| v.as_str()).unwrap_or("Effect");
        let modifiers = eff.get("modifiers").cloned().unwrap_or(json!({}));
        let kind = eff.get("kind").and_then(|v| v.as_str()).unwrap_or("buff");
        let icon = eff
            .get("icon")
            .and_then(|v| v.as_str())
            .unwrap_or("sparkles");
        // 1 batched INSERT via unnest (was N INSERTs).
        let r = sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type, applied_at_round, applied_at_turn_index)
               select v.cid, $1, $2::effect_kind, $3, 'permanent', null, null, 'round_end',
                      false, true, $4, 'manual', $6, $7
               from unnest($5::uuid[]) as v(cid)"#,
        )
        .bind(name).bind(kind).bind(icon).bind(&modifiers)
        .bind(&body.combatant_ids)
        .bind(enc_round).bind(enc_turn_index)
        .execute(&mut *tx)
        .await?;
        affected += r.rows_affected() as usize;
    }

    tx.commit().await?;

    if affected > 0 {
        // 1 batched WS event (was N per-combatant publishes).
        ws::publish_persist(
            &s.db,
            campaign_id,
            json!({
                "type": "effects_change",
                "combatant_ids": body.combatant_ids,
            }),
        )
        .await;
    }

    Ok(Json(PatchEffectsResult { affected }))
}
