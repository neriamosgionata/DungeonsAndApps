// list_combatants — list all combatants in an encounter (master sees hidden).
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use uuid::Uuid;

pub async fn list_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Combatant>>> {
    let e = super::super::fetch(&s, encounter_id).await?;
    let role = rbac::require_member(&s.db, uid, e.campaign_id).await?;
    let rows: Vec<Combatant> = if role == Role::Master {
        sqlx::query_as::<_, Combatant>(
            "select id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                    initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                    token_x, token_y, token_color, token_on_map, token_image,
                    coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url,
                    token_moved_round,
                    action_used, bonus_action_used, reaction_used, movement_used_ft,
                    legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits
              from combatants where encounter_id = $1 order by turn_order, -initiative, -dex_tiebreaker")
            .bind(encounter_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Combatant>(
            "select c.id, c.encounter_id, c.ref_type::text as ref_type, c.character_id, c.npc_id, c.display_name,
                    c.initiative, c.dex_tiebreaker,
                    case when ch.owner_id = $2 then c.hp_current else 0 end as hp_current,
                    case when ch.owner_id = $2 then c.hp_max     else 0 end as hp_max,
                    case when ch.owner_id = $2 then c.temp_hp    else 0 end as temp_hp,
                    case when ch.owner_id = $2 then c.ac         else 0 end as ac,
                    c.conditions, c.notes, c.is_visible, c.turn_order, c.initiative_rolled,
                    c.token_x, c.token_y, c.token_color, c.token_on_map, c.token_image,
                    coalesce(c.token_image, ch.portrait_url, (select image_key from npcs where id = c.npc_id)) as portrait_url,
                    c.token_moved_round,
                    c.action_used, c.bonus_action_used, c.reaction_used, c.movement_used_ft,
                    c.legendary_actions_max, c.legendary_actions_used, c.legendary_resistances_max, c.legendary_resistances_used,
                      c.readied_action, c.cover_bonus, c.delayed_turn, c.action_spell_level, c.bonus_action_spell_level, c.last_hit_attack_total, c.last_hit_damage, c.spell_being_cast, c.level_override, c.vision_range, c.faction, c.pending_hits
              from combatants c
             left join characters ch on ch.id = c.character_id
             where c.encounter_id = $1
               and (c.is_visible = true or ch.owner_id = $2)
             order by c.turn_order, -c.initiative, -c.dex_tiebreaker")
            .bind(encounter_id).bind(uid).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}
