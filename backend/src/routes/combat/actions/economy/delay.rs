// delay_turn — defer turn to a later position in the initiative order.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct DelayBody {
    pub insert_after_turn_index: i32,
}

pub async fn delay_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DelayBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, i32, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, c.turn_order, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, current_turn, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::Conflict("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    // HIGH-11: lock the encounter row so two concurrent delay_turn calls for
    // different combatants can't interleave the encounter-wide turn_order
    // UPDATE (TOCTOU between the per-row UPDATE and the SELECT-of-others).
    sqlx::query("select id from encounters where id = $1 for update")
        .bind(encounter_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    let c: Option<Combatant> = sqlx::query_as::<_, Combatant>(
        r#"update combatants set delayed_turn = true, action_used = true, readied_action = null
           where id = $1 and action_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?;

    let c = c.ok_or_else(|| AppError::BadRequest("action already used this turn".into()))?;

    sqlx::query(
        r#"update combatants set turn_order = case
            when turn_order > $1 and turn_order <= $2 then turn_order - 1
            when turn_order = $1 then $2
            else turn_order
           end
           where encounter_id = $3"#,
    )
    .bind(current_turn)
    .bind(body.insert_after_turn_index)
    .bind(encounter_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_delays",
            "id": id,
            "insert_after": body.insert_after_turn_index,
        }),
    )
    .await;

    Ok(Json(c))
}
