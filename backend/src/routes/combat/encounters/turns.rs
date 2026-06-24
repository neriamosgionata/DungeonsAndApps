// turn management: next_turn, prev_turn, goto_turn.
use super::read::fetch;
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::super::{tick_effects, notify_turn};
use super::types::{Encounter, GotoTurnBody};
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn next_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    // MED-11: re-fetch + FOR UPDATE the encounter row inside the tx so the
    // status/turn_index/round read is consistent with the writes. Pre-fix
    // the encounter was read outside the tx, leaving a TOCTOU window where
    // the encounter could be `ended` mid-flight but next_turn would still
    // proceed.
    let e0 = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e0.campaign_id).await?;
    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1 for update",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if e.status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if rolled == 0 {
        tx.rollback().await?;
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }
    let prev_turn_index = e.turn_index;
    let next_idx = e.turn_index + 1;
    let (new_idx, new_round) = if (next_idx as i64) >= rolled {
        (0, e.round + 1)
    } else {
        (next_idx, e.round)
    };
    let prev_round = e.round;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    if new_round > prev_round {
        sqlx::query(
            "update combatants set token_moved_round = null, reaction_used = false
             where encounter_id = $1"
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query("update encounters set lair_action_used = false where id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        // PHB: readied actions expire at end of next round (set_at_round + 1).
        sqlx::query(
            "update combatants set readied_action = null
             where encounter_id = $1
               and readied_action is not null
               and coalesce((readied_action->>'expires_at_round')::int, 0) < $2"
        )
        .bind(id)
        .bind(new_round)
        .execute(&mut *tx)
        .await?;
    }
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order"
    )
    .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == new_idx) {
        sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb where id = $1"
        )
        .bind(cid).execute(&mut *tx).await?;
    }
    let events = tick_effects(
        &mut tx,
        id,
        prev_round,
        prev_turn_index,
        new_round,
        new_idx,
    )
    .await?;
    tx.commit().await?;
    for ev in events {
        let v: serde_json::Value = serde_json::from_str(&ev)
            .expect("tick_effects emits valid JSON");
        ws::publish_persist(&s.db, e.campaign_id, v).await;
    }
    ws::publish_persist(
        &s.db,
        e.campaign_id,
        json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}),
    )
    .await;
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

pub async fn prev_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    // MED-11: re-fetch + FOR UPDATE the encounter row inside the tx so the
    // status/turn_index/round read is consistent with the writes. Mirrors
    // the next_turn fix. Pre-fix the encounter was read outside the tx,
    // leaving a TOCTOU window where the encounter could be `ended` mid-flight.
    let e0 = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e0.campaign_id).await?;
    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1 for update"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if e.status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    if e.round == 0 && e.turn_index == 0 {
        return Err(AppError::BadRequest("already at first turn".into()));
    }

    let prev_round = e.round;
    let prev_idx = e.turn_index - 1;
    let new_idx = if prev_idx < 0 { 0 } else { prev_idx };
    let new_round = if prev_idx < 0 { e.round - 1 } else { e.round };
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    if new_round < prev_round {
        sqlx::query(
            "update combatants set token_moved_round = null, reaction_used = false
             where encounter_id = $1"
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order"
    )
    .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == new_idx) {
        sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb where id = $1"
        )
        .bind(cid).execute(&mut *tx).await?;
    }
    let events = tick_effects(
        &mut tx,
        id,
        prev_round,
        prev_idx,
        new_round,
        new_idx,
    )
    .await?;
    tx.commit().await?;
    for ev in events {
        let v: serde_json::Value = serde_json::from_str(&ev)
            .expect("tick_effects emits valid JSON");
        ws::publish_persist(&s.db, e.campaign_id, v).await;
    }
    ws::publish_persist(
        &s.db,
        e.campaign_id,
        json!({"type":"prev_turn","id":id,"round":new_round,"turn_index":new_idx}),
    )
    .await;
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

pub async fn goto_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GotoTurnBody>,
) -> AppResult<Json<Encounter>> {
    // MED-11: re-fetch + FOR UPDATE the encounter row inside the tx. Mirrors
    // next_turn + prev_turn fix. Pre-fix the encounter was read outside the
    // tx, leaving a TOCTOU window where status could flip mid-flight.
    let e0 = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e0.campaign_id).await?;
    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1 for update"
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::NotFound)?;
    if e.status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }
    let prev_round = e.round;
    let prev_turn_index = e.turn_index;
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if rolled == 0 || body.turn_index < 0 || (body.turn_index as i64) >= rolled {
        tx.rollback().await?;
        return Err(AppError::BadRequest("turn_index out of range".into()));
    }
    if let Some(r) = body.round {
        if r < 1 {
            tx.rollback().await?;
            return Err(AppError::BadRequest("round must be >= 1".into()));
        }
    }
    let new_round = body.round.unwrap_or(prev_round);
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(id).bind(body.turn_index).bind(new_round).fetch_one(&mut *tx).await?;
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order"
    )
    .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == body.turn_index) {
        sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb where id = $1"
        )
        .bind(cid).execute(&mut *tx).await?;
    }
    let events = tick_effects(
        &mut tx,
        id,
        prev_round,
        prev_turn_index,
        e.round,
        body.turn_index,
    )
    .await?;
    tx.commit().await?;
    for ev in events {
        let v: serde_json::Value = serde_json::from_str(&ev)
            .expect("tick_effects emits valid JSON");
        ws::publish_persist(&s.db, e.campaign_id, v).await;
    }
    // L-WS3: goto_turn emits a distinct event type. The previous shape
    // was `next_turn` for both next + goto, which made the UI unable
    // to distinguish a normal advance from a jump (e.g. to undo a
    // misclick or to set up a specific turn order). New type = `goto_turn`.
    ws::publish_persist(
        &s.db,
        e.campaign_id,
        json!({"type":"goto_turn","id":id,"round":e.round,"turn_index":body.turn_index}),
    )
    .await;
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}
