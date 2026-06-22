// update_combatant — PATCH /combatants/{id} with RBAC + cosmetic-only restriction.
use super::*;
use super::super::actions::sync::sync_combatant_hp_to_sheet;
use super::types::CombatantUpdate;
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde_json::json;
use uuid::Uuid;

pub async fn update_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CombatantUpdate>,
) -> AppResult<Json<Combatant>> {
    body.validate()?;
    let row: (Uuid, Uuid, i32, String, Option<Uuid>) = sqlx::query_as(
        "select c.id, e.campaign_id, c.hp_current, c.ref_type::text, ch.owner_id \
         from combatants c \
         join encounters e on e.id = c.encounter_id \
         left join characters ch on ch.id = c.character_id \
         where c.id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let campaign_id = row.1;
    let ref_type = row.3;
    let owner = row.4;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        if ref_type != "character" || owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
        if body.faction.is_some() {
            return Err(AppError::Forbidden);
        }
        let cosmetic_only = body.display_name.is_none()
            && body.initiative.is_none()
            && body.dex_tiebreaker.is_none()
            && body.hp_current.is_none()
            && body.hp_max.is_none()
            && body.temp_hp.is_none()
            && body.ac.is_none()
            && body.conditions.is_none()
            && body.notes.is_none()
            && body.is_visible.is_none()
            && body.token_x.is_none()
            && body.token_y.is_none()
            && body.token_on_map.is_none()
            && body.token_color.is_none()
            && body.token_image.is_none()
            && body.action_used.is_none()
            && body.bonus_action_used.is_none()
            && body.reaction_used.is_none()
            && body.movement_used_ft.is_none()
            && body.legendary_actions_used.is_none()
            && body.legendary_resistances_used.is_none();
        if !cosmetic_only {
            return Err(AppError::Forbidden);
        }
    }
    if ref_type == "character"
        && (body.hp_current.is_some()
            || body.hp_max.is_some()
            || body.temp_hp.is_some()
            || body.ac.is_some())
    {
        return Err(AppError::BadRequest(
            "character HP/AC is owned by the player sheet".into(),
        ));
    }
    let prev_hp = row.2;
    let clear_token_image = body.clear_token_image.unwrap_or(false);
    // MED-8: NaN / +inf / -inf in token_x/y would propagate through every
    // `sqrt((dx*dx+dy*dy))` distance call → NaN distances → all positioning
    // broken. Pre-fix `move_combatant` clamps 0..100; the PATCH path didn't.
    let safe_token_x = body
        .token_x
        .map(|v| if v.is_finite() { v.clamp(0.0, 100.0) } else { 50.0 });
    let safe_token_y = body
        .token_y
        .map(|v| if v.is_finite() { v.clamp(0.0, 100.0) } else { 50.0 });
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             display_name   = coalesce($2, display_name),
             initiative     = coalesce($3, initiative),
             dex_tiebreaker = coalesce($4, dex_tiebreaker),
             hp_current     = coalesce($5, hp_current),
             hp_max         = coalesce($6, hp_max),
             temp_hp        = case when $7 is not null and $7 > temp_hp then $7 else temp_hp end,
             ac             = coalesce($8, ac),
             conditions     = coalesce($9, conditions),
             notes          = coalesce($10, notes),
             is_visible     = coalesce($11, is_visible),
             token_x        = coalesce($12, token_x),
             token_y        = coalesce($13, token_y),
             token_color    = coalesce($14, token_color),
             token_on_map   = coalesce($15, token_on_map),
             token_image    = case when $17 then null else coalesce($16, token_image) end,
             action_used    = coalesce($18, action_used),
             bonus_action_used = coalesce($19, bonus_action_used),
             reaction_used  = coalesce($20, reaction_used),
             movement_used_ft = coalesce($21, movement_used_ft),
             legendary_actions_used = coalesce($22, legendary_actions_used),
             legendary_resistances_used = coalesce($23, legendary_resistances_used),
             readied_action = coalesce($24, readied_action),
             cover_bonus = coalesce($25, cover_bonus),
             delayed_turn = coalesce($26, delayed_turn),
             faction = coalesce($27, faction)
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
    )
    .bind(id).bind(body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.temp_hp).bind(body.ac)
    .bind(body.conditions).bind(body.notes).bind(body.is_visible)
    .bind(safe_token_x).bind(safe_token_y).bind(body.token_color).bind(body.token_on_map)
    .bind(body.token_image).bind(clear_token_image)
    .bind(body.action_used).bind(body.bonus_action_used).bind(body.reaction_used)
    .bind(body.movement_used_ft).bind(body.legendary_actions_used).bind(body.legendary_resistances_used)
    .bind(body.readied_action).bind(body.cover_bonus).bind(body.delayed_turn)
    .bind(body.faction)
    .fetch_one(&s.db).await?;
    ws::publish(
        campaign_id,
        json!({"type":"combatant_updates","id":id}).to_string(),
    );

    // Sync HP back to character sheet if it changed
    if (c.hp_current != prev_hp || c.temp_hp > 0) && c.character_id.is_some() {
        if let Err(e) = sync_combatant_hp_to_sheet(&s.db, id, c.hp_current, c.temp_hp).await {
            tracing::error!(combatant_id = %id, "sync sheet HP: {e}");
        }
    }
    Ok(Json(c))
}
