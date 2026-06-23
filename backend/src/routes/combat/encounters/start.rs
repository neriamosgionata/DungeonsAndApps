// start encounter — set status=active, roll initiative, set turn_index=0.
use crate::rbac;
use crate::ws;
use crate::AppState;
use crate::error::{AppError, AppResult};
use crate::extract::AuthUser;
use super::read::fetch;
use super::types::Encounter;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn start(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status == "active" {
        return Err(AppError::Conflict("encounter already active".into()));
    }

    let mut tx = s.db.begin().await?;

    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    if rolled == 0 {
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }

    // Sort combatants by initiative (desc) then dex_tiebreaker (desc). Stable sort.
    let combatants: Vec<(i32, i16, Uuid)> = sqlx::query_as(
        "select initiative, dex_tiebreaker, id from combatants where encounter_id = $1 and initiative_rolled = true"
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let mut sorted: Vec<(i32, i16, Uuid)> = combatants;
    sorted.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));

    // Single batched UPDATE assigns turn_order via ROW_NUMBER sorted by
    // initiative DESC, dex_tiebreaker DESC. Replaces the per-combatant
    // UPDATE loop (N round-trips for N combatants → 1).
    sqlx::query(
        r#"update combatants c
           set turn_order = sub.new_order
           from (
             select id, (row_number() over (order by initiative desc, dex_tiebreaker desc) - 1)::int as new_order
             from combatants
             where encounter_id = $1 and initiative_rolled = true
           ) sub
           where c.id = sub.id"#,
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'active', round = 1, turn_index = (
            select turn_order from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order asc limit 1
         ) where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let start_idx = e.turn_index;

    // Reset all per-turn flags for the new encounter.
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

    // L11: reset per-turn flags for ALL combatants in the encounter, not just
    // the first one. Pre-fix only the active-turn combatant was reset, leaving
    // stale `action_used/bonus_action_used/movement_used_ft/...` on combatants
    // 2+ before the first next_turn. next_turn would re-reset the active
    // combatant but the others could see stale state.
    sqlx::query(
        "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, spell_being_cast = null, legendary_actions_used = 0, pending_hits = '[]'::jsonb
         where encounter_id = $1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    ws::publish_persist(
        &s.db,
        e.campaign_id,
        json!({"type":"encounter_starts","id":id,"round":1,"turn_index":start_idx}),
    )
    .await;
    for (i, (_, _, cid)) in sorted.iter().enumerate() {
        ws::publish_persist(
            &s.db,
            e.campaign_id,
            json!({"type":"combatant_updates","id":cid,"initiative":sorted[i].0,"initiative_rolled":true}),
        )
        .await;
    }
    Ok(Json(e))
}
