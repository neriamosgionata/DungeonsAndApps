// add_combatant — add single combatant (master only).
use super::*;
use super::types::CombatantCreate;
use super::Combatant;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde_json::json;
use uuid::Uuid;

pub async fn add_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<CombatantCreate>,
) -> AppResult<(StatusCode, Json<Combatant>)> {
    body.validate()?;
    let e: Encounter = super::super::fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if body.ref_type != "character" && body.ref_type != "npc" {
        return Err(AppError::BadRequest(
            "ref_type must be character|npc".into(),
        ));
    }
    if body.ref_type == "character" {
        if let Some(chid) = body.character_id {
            let dead: Option<bool> = sqlx::query_scalar(
                "select (sheet->>'alive')::boolean from characters where id = $1 and campaign_id = $2")
                .bind(chid).bind(e.campaign_id).fetch_optional(&s.db).await?.flatten();
            if dead == Some(false) {
                return Err(AppError::BadRequest("character is dead".into()));
            }
        }
    }

    let mut npc_stats: Option<combat_engine::NpcStats> = None;
    if body.ref_type == "npc" && body.npc_id.is_some() {
        let raw: Option<serde_json::Value> =
            sqlx::query_scalar("select stats from npcs where id = $1 and campaign_id = $2")
                .bind(
                    body.npc_id
                        .ok_or(AppError::BadRequest("npc_id required".into()))?,
                )
                .bind(e.campaign_id)
                .fetch_optional(&s.db)
                .await?;
        npc_stats = raw.as_ref().and_then(combat_engine::NpcStats::from_value);
    }

    let default_hp_max = npc_stats.as_ref().and_then(|n| n.hp.max).unwrap_or(0);
    let default_hp_current = npc_stats
        .as_ref()
        .and_then(|n| n.hp.current)
        .unwrap_or(default_hp_max);
    let default_ac = npc_stats.as_ref().and_then(|n| n.ac).unwrap_or(10);
    let default_dex = npc_stats.as_ref().map(|n| n.abilities.dex).unwrap_or(10);
    let default_legendary_actions = npc_stats
        .as_ref()
        .and_then(|n| n.legendary_actions.first())
        .map(|_| 3)
        .unwrap_or(0);
    let default_legendary_resistances = npc_stats
        .as_ref()
        .and_then(|n| {
            n.traits
                .iter()
                .find(|t| t.name.to_lowercase().contains("legendary resistance"))
        })
        .map(|_| 3)
        .unwrap_or(0);

    let default_rolled = body.ref_type != "character";
    // LOW-4: prevent duplicate combatants in the same encounter.
    if let Some(chid) = body.character_id {
        let dup: Option<Uuid> = sqlx::query_scalar(
            "select id from combatants where encounter_id = $1 and character_id = $2"
        )
        .bind(encounter_id).bind(chid).fetch_optional(&s.db).await?;
        if dup.is_some() {
            return Err(AppError::Conflict(
                "this character is already in the encounter".into(),
            ));
        }
    }
    if let Some(nid) = body.npc_id {
        let dup: Option<Uuid> = sqlx::query_scalar(
            "select id from combatants where encounter_id = $1 and npc_id = $2"
        )
        .bind(encounter_id).bind(nid).fetch_optional(&s.db).await?;
        if dup.is_some() {
            return Err(AppError::Conflict(
                "this NPC is already in the encounter".into(),
            ));
        }
    }
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"insert into combatants
           (encounter_id, ref_type, character_id, npc_id, display_name, initiative, dex_tiebreaker,
            hp_current, hp_max, ac, is_visible, initiative_rolled,
            legendary_actions_max, legendary_resistances_max)
           values ($1, $2::combatant_ref, $3, $4, $5, coalesce($6, 0), coalesce($7, $14),
                  coalesce($8, $15), coalesce($9, $16), coalesce($10, $17), coalesce($11, true), coalesce($12, $13),
                  coalesce($18, 0), coalesce($19, 0))
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, spell_being_cast, level_override, vision_range, faction, pending_hits"#,
    )
    .bind(encounter_id).bind(&body.ref_type).bind(body.character_id).bind(body.npc_id)
    .bind(&body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.ac)
    .bind(body.is_visible).bind(body.initiative_rolled).bind(default_rolled)
    .bind(default_dex as i16).bind(default_hp_current).bind(default_hp_max)
    .bind(default_ac).bind(default_legendary_actions).bind(default_legendary_resistances)
    .fetch_one(&s.db).await?;
    ws::publish(
        e.campaign_id,
        json!({"type":"combatant_joins","encounter_id":encounter_id,"id":c.id}).to_string(),
    );
    crate::routes::notifications::emit_campaign(
        &s.db,
        e.campaign_id,
        Some(uid),
        "combat.joined",
        &format!("{} joined combat", c.display_name),
        Some(&format!(
            "Init {} · HP {}/{} · AC {}",
            c.initiative, c.hp_current, c.hp_max, c.ac
        )),
        Some("encounter"),
        Some(encounter_id),
    )
    .await;
    Ok((StatusCode::CREATED, Json(c)))
}
