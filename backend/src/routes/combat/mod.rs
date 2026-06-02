pub mod actions;
pub mod combatants;
pub mod encounters;
pub mod events;
pub mod special;
pub mod spells;
pub mod tactical;

use crate::{
    AppState,
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    routes::notifications::{emit, emit_campaign, NewNotif},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use serde_json::Value;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

use self::spells::cast_spell;
use self::tactical::{
    add_condition, calculate_cover, check_flanking, create_overlay, delete_overlay,
    encounter_difficulty, is_between, is_flanking, list_overlays, overlay_damage,
    segments_intersect, surprise_auto, surprise_round,
    PatchEffectsBody, PatchEffectsResult,
};
use self::actions::*;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/encounters", get(list).post(create))
        .route("/encounters/{id}", get(read).patch(update).delete(delete))
        .route("/encounters/{id}/combatants", get(list_combatants).post(add_combatant))
        .route("/encounters/{id}/combatants/bulk", post(bulk_add_combatants))
        .route("/combatants/{id}", axum::routing::patch(update_combatant).delete(delete_combatant))
        .route("/combatants/{id}/move", post(move_combatant))
        .route("/combatants/{id}/use-action", post(use_action))
        .route("/encounters/{id}/next-turn", post(next_turn))
        .route("/encounters/{id}/prev-turn", post(prev_turn))
        .route("/encounters/{id}/goto-turn", post(goto_turn))
        .route("/encounters/{id}/start", post(start))
        .route("/encounters/{id}/end", post(end_encounter))
        .route("/encounters/{id}/set-initiative", post(set_initiative))
        .route("/encounters/{id}/overlays", get(list_overlays).post(create_overlay))
        .route("/encounters/{id}/overlays/{overlay_id}", axum::routing::delete(delete_overlay))
        .route("/combatants/{id}/attack", post(attack))
        .route("/combatants/{id}/damage", post(deal_damage))
        .route("/combatants/{id}/save", post(roll_save))
        .route("/combatants/{id}/computed-stats", get(computed_stats))
        .route("/combatants/{id}/react", post(react))
        .route("/combatants/{id}/cast-spell", post(cast_spell))
        .route("/combatants/{id}/dodge", post(dodge))
        .route("/combatants/{id}/disengage", post(disengage))
        .route("/combatants/{id}/help", post(help_action))
        .route("/combatants/{id}/opportunity-attack", post(opportunity_attack))
        .route("/combatants/{id}/ready", post(ready_action))
        .route("/combatants/{id}/delay", post(delay_turn))
        .route("/combatants/{id}/grapple", post(grapple))
        .route("/combatants/{id}/grapple-escape", post(grapple_escape))
        .route("/combatants/{id}/shove", post(shove))
        .route("/combatants/{id}/stand-up", post(stand_up))
        .route("/combatants/{id}/heal", post(heal))
        .route("/combatants/{id}/death-save", post(death_save))
        .route("/combatants/{id}/skill-check", post(skill_check))
        .route("/encounters/{id}/lair-action", post(lair_action))
        .route("/combatants/{id}/legendary-action", post(legendary_action))
        .route("/combatants/{id}/multiattack", post(multiattack))
        .route("/combatants/{id}/parse-multiattack", get(parse_multiattack))
        .route("/combatants/{id}/trigger-ready", post(trigger_ready))
        .route("/combatants/{id}/class-feature", post(class_feature))
        .route("/combatants/{id}/two-weapon-fight", post(two_weapon_fight))
        .route("/combatants/{id}/dash", post(dash))
        .route("/combatants/{id}/hide", post(hide))
        .route("/combatants/{id}/contested-hide", post(contested_hide))
        .route("/combatants/{id}/search", post(search_action))
        .route("/combatants/{id}/use-object", post(use_object))
        .route("/combatants/{id}/conditions", post(add_condition))
        .route("/encounters/{id}/effects", patch(patch_effects))
        .route("/encounters/{id}/overlay-damage", post(overlay_damage))
        .route("/encounters/{id}/surprise", post(surprise_round))
        .route("/encounters/{id}/surprise-auto", post(surprise_auto))
        .route("/encounters/{id}/difficulty", get(encounter_difficulty))
        .route("/encounters/{id}/flanking", get(check_flanking))
        .route("/encounters/{id}/cover", get(calculate_cover))
        .route("/encounters/{id}/events", get(list_events))
        .route("/combat-events/{event_id}", axum::routing::delete(delete_event))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Encounter {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub status: String,
    pub round: i32,
    pub turn_index: i32,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub map_grid_size: i32,
    pub show_grid: bool,
    pub grid_type: String,
    pub lair_action_used: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub clear_map_image: Option<bool>,
    #[validate(range(min = 20, max = 200))]
    pub map_grid_size: Option<i32>,
    pub show_grid: Option<bool>,
    pub grid_type: Option<String>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<Json<Vec<Encounter>>> {
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let rows: Vec<Encounter> = if role == Role::Master {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
             from encounters where campaign_id = $1 order by updated_at desc")
            .bind(campaign_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
             from encounters where campaign_id = $1 and status in ('planned','active') order by updated_at desc")
            .bind(campaign_id).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<EncounterCreate>,
) -> AppResult<(StatusCode, Json<Encounter>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "insert into encounters (campaign_id, name, notes) values ($1, $2, $3)
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(campaign_id).bind(&body.name).bind(&body.notes).fetch_one(&s.db).await?;

    // auto-add all LIVING campaign characters as pending combatants so
    // players can roll initiative as soon as the encounter exists. Dead
    // characters (sheet.alive = false) are skipped.
    sqlx::query(
        r#"insert into combatants
             (encounter_id, ref_type, character_id, display_name, initiative,
              hp_current, hp_max, ac, initiative_rolled)
           select $1, 'character'::combatant_ref, ch.id, ch.name,
                  0,
                  greatest(1, coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 10)),
                  greatest(1, coalesce((ch.sheet->'hp'->>'max')::int, 10)),
                  coalesce((ch.sheet->>'ac')::int, 10),
                  false
           from characters ch
           where ch.campaign_id = $2
             and coalesce((ch.sheet->>'alive')::boolean, true) = true"#,
    )
    .bind(e.id).bind(campaign_id).execute(&s.db).await?;

    ws::publish(campaign_id, json!({"type":"encounter_created","id":e.id,"name":e.name}).to_string());
    emit_campaign(&s.db, campaign_id, Some(uid),
        "combat.roll_initiative",
        &format!("Combat: {}", e.name),
        Some("Roll initiative to join!"),
        Some("encounter"), Some(e.id)).await;
    Ok((StatusCode::CREATED, Json(e)))
}

async fn fetch(s: &AppState, id: Uuid) -> AppResult<Encounter> {
    sqlx::query_as::<_, Encounter>(
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at
         from encounters where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_member(&s.db, uid, e.campaign_id).await?;
    Ok(Json(e))
}

async fn use_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseAction>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        "select c.id, e.campaign_id, c.ref_type::text, ch.owner_id \
         from combatants c \
         join encounters e on e.id = c.encounter_id \
         left join characters ch on ch.id = c.character_id \
         where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let campaign_id = row.1;
    let ref_type = row.2;
    let owner = row.3;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    // Master can toggle anyone; player can only toggle their own character.
    if role != Role::Master {
        if ref_type != "character" || owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let c: Combatant = match body.action.as_str() {
        "action" => sqlx::query_as::<_, Combatant>(
            "update combatants set action_used = not action_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast")
            .bind(id).fetch_one(&s.db).await?,
        "bonus_action" => sqlx::query_as::<_, Combatant>(
            "update combatants set bonus_action_used = not bonus_action_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast")
            .bind(id).fetch_one(&s.db).await?,
        "reaction" => sqlx::query_as::<_, Combatant>(
            "update combatants set reaction_used = not reaction_used where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast")
            .bind(id).fetch_one(&s.db).await?,
        "legendary_action" => sqlx::query_as::<_, Combatant>(
            "update combatants set legendary_actions_used = least(legendary_actions_max, legendary_actions_used + 1) where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast")
            .bind(id).fetch_one(&s.db).await?,
        "legendary_resistance" => sqlx::query_as::<_, Combatant>(
            "update combatants set legendary_resistances_used = least(legendary_resistances_max, legendary_resistances_used + 1) where id = $1
             returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                       initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                       token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                       action_used, bonus_action_used, reaction_used, movement_used_ft,
                       legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast")
            .bind(id).fetch_one(&s.db).await?,
        _ => return Err(AppError::BadRequest("action must be action|bonus_action|reaction|legendary_action|legendary_resistance".into())),
    };

    ws::publish(campaign_id, json!({"type":"combatant_updated","id":id}).to_string());
    Ok(Json(c))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<EncounterUpdate>,
) -> AppResult<Json<Encounter>> {
    body.validate()?;
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    let clear_map = body.clear_map_image.unwrap_or(false);
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        r#"update encounters set
             name           = coalesce($2, name),
             notes          = coalesce($3, notes),
             map_image      = case when $5 then null else coalesce($4, map_image) end,
             map_grid_size  = coalesce($6, map_grid_size),
             show_grid      = coalesce($7, show_grid),
             grid_type      = coalesce($8, grid_type)
           where id = $1
           returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at"#)
        .bind(id)
        .bind(body.name)
        .bind(body.notes)
        .bind(body.map_image)
        .bind(clear_map)
        .bind(body.map_grid_size)
        .bind(body.show_grid)
        .bind(body.grid_type)
        .fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_updated","id":id}).to_string());
    Ok(Json(e))
}

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

async fn list_events(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Query(q): Query<EventListQ>,
) -> AppResult<Json<Vec<CombatEvent>>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let offset = q.offset.unwrap_or(0).max(0);
    let rows: Vec<CombatEvent> = sqlx::query_as::<_, CombatEvent>(
        "select id, encounter_id, round, actor_combatant, target_combatant, action, roll_id, delta_hp, note, created_at
         from combat_events where encounter_id = $1 order by created_at desc limit $2 offset $3")
        .bind(encounter_id).bind(limit).bind(offset).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

async fn delete_event(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(event_id): Path<Uuid>,
) -> AppResult<Json<()>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        r#"select e.campaign_id from combat_events ce
           join encounters e on e.id = ce.encounter_id
           where ce.id = $1"#)
        .bind(event_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    sqlx::query("delete from combat_events where id = $1")
        .bind(event_id).execute(&s.db).await?;
    Ok(Json(()))
}

/// Extract condition name, stripping optional duration suffix ("poisoned:3" → "poisoned")
fn cond_name(c: &str) -> &str {
    c.split(':').next().unwrap_or(c)
}

fn has_condition(conditions: &[String], name: &str) -> bool {
    conditions.iter().any(|c| cond_name(c).eq_ignore_ascii_case(name))
}

fn remove_condition(conditions: Vec<String>, name: &str) -> Vec<String> {
    conditions.into_iter().filter(|c| !cond_name(c).eq_ignore_ascii_case(name)).collect()
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    sqlx::query("delete from encounters where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
pub struct Combatant {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub ref_type: String,
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    pub display_name: String,
    pub initiative: i32,
    pub dex_tiebreaker: i16,
    pub hp_current: i32,
    pub hp_max: i32,
    pub temp_hp: i32,
    pub ac: i32,
    pub conditions: Vec<String>,
    pub notes: Option<String>,
    pub is_visible: bool,
    pub turn_order: i32,
    pub initiative_rolled: bool,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    pub token_color: Option<String>,
    pub token_on_map: bool,
    pub token_image: Option<String>,
    pub portrait_url: Option<String>,
    pub token_moved_round: Option<i32>,
    pub action_used: bool,
    pub bonus_action_used: bool,
    pub reaction_used: bool,
    pub movement_used_ft: i32,
    pub legendary_actions_max: i32,
    pub legendary_actions_used: i32,
    pub legendary_resistances_max: i32,
    pub legendary_resistances_used: i32,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: i32,
    pub delayed_turn: bool,
    pub action_spell_level: i16,
    pub bonus_action_spell_level: i16,
    pub last_hit_attack_total: Option<i32>,
    pub last_hit_damage: Option<i32>,
    pub last_hit_attacker: Option<Uuid>,
    pub spell_being_cast: Option<String>,
}


#[derive(Debug, Deserialize, Validate)]
pub struct CombatantCreate {
    pub ref_type: String, // "character" | "npc"
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    #[validate(length(min = 1, max = 80))]
    pub display_name: String,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub ac: Option<i32>,
    pub is_visible: Option<bool>,
    pub initiative_rolled: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CombatantUpdate {
    #[validate(length(min = 1, max = 80))]
    pub display_name: Option<String>,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub temp_hp: Option<i32>,
    pub ac: Option<i32>,
    pub conditions: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_visible: Option<bool>,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    #[validate(length(min = 3, max = 20))]
    pub token_color: Option<String>,
    pub token_on_map: Option<bool>,
    pub token_image: Option<String>,
    pub clear_token_image: Option<bool>,
    pub action_used: Option<bool>,
    pub bonus_action_used: Option<bool>,
    pub reaction_used: Option<bool>,
    pub movement_used_ft: Option<i32>,
    pub legendary_actions_used: Option<i32>,
    pub legendary_resistances_used: Option<i32>,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: Option<i32>,
    pub delayed_turn: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CombatantMove {
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Deserialize)]
pub struct UseAction {
    pub action: String, // action | bonus_action | reaction | legendary_action | legendary_resistance
}

async fn list_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Combatant>>> {
    let e = fetch(&s, encounter_id).await?;
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
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast
             from combatants where encounter_id = $1 order by turn_order, -initiative, -dex_tiebreaker")
            .bind(encounter_id).fetch_all(&s.db).await?
    } else {
        // Players see HP/AC only for their own character's combatant; everyone
        // else's stats are zeroed so the sheet can't leak through the roster.
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
                    c.readied_action, c.cover_bonus, c.delayed_turn, c.action_spell_level, c.bonus_action_spell_level, c.last_hit_attack_total, c.last_hit_damage, c.last_hit_attacker, c.spell_being_cast
             from combatants c
             left join characters ch on ch.id = c.character_id
             where c.encounter_id = $1 and c.is_visible = true
             order by c.turn_order, -c.initiative, -c.dex_tiebreaker")
            .bind(encounter_id).bind(uid).fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn add_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<CombatantCreate>,
) -> AppResult<(StatusCode, Json<Combatant>)> {
    body.validate()?;
    let e = fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if body.ref_type != "character" && body.ref_type != "npc" {
        return Err(AppError::BadRequest("ref_type must be character|npc".into()));
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

    // Auto-populate NPC stats from structured stat block if not overridden.
    let mut npc_stats: Option<combat_engine::NpcStats> = None;
    if body.ref_type == "npc" && body.npc_id.is_some() {
        let raw: Option<Value> = sqlx::query_scalar(
            "select stats from npcs where id = $1 and campaign_id = $2")
            .bind(body.npc_id.ok_or(AppError::BadRequest("npc_id required for npc combatant".into()))?).bind(e.campaign_id).fetch_optional(&s.db).await?;
        npc_stats = raw.as_ref().and_then(combat_engine::NpcStats::from_value);
    }

    let default_hp_max = npc_stats.as_ref()
        .and_then(|n| n.hp.max).unwrap_or(0);
    let default_hp_current = npc_stats.as_ref()
        .and_then(|n| n.hp.current).unwrap_or(default_hp_max);
    let default_ac = npc_stats.as_ref()
        .and_then(|n| n.ac).unwrap_or(10);
    let default_dex = npc_stats.as_ref()
        .map(|n| n.abilities.dex).unwrap_or(10);
    let _default_pb = npc_stats.as_ref()
        .and_then(|n| if n.pb > 0 { Some(n.pb) } else { None });
    let default_legendary_actions = npc_stats.as_ref()
        .and_then(|n| n.legendary_actions.first())
        .map(|_| 3).unwrap_or(0); // standard 3 if any legendary actions listed
    let default_legendary_resistances = npc_stats.as_ref()
        .and_then(|n| n.traits.iter().find(|t| t.name.to_lowercase().contains("legendary resistance")))
        .map(|_| 3).unwrap_or(0); // standard 3 if trait present

    // Default initiative_rolled: characters start unrolled (must roll init),
    // NPCs start rolled (master provides the initiative value directly).
    let default_rolled = body.ref_type != "character";
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
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(encounter_id)
    .bind(&body.ref_type)
    .bind(body.character_id)
    .bind(body.npc_id)
    .bind(&body.display_name)
    .bind(body.initiative)
    .bind(body.dex_tiebreaker)
    .bind(body.hp_current)
    .bind(body.hp_max)
    .bind(body.ac)
    .bind(body.is_visible)
    .bind(body.initiative_rolled)
    .bind(default_rolled)
    .bind(default_dex as i16)          // $14: dex tiebreaker from stat block
    .bind(default_hp_current)          // $15
    .bind(default_hp_max)              // $16
    .bind(default_ac)                  // $17
    .bind(default_legendary_actions)   // $18
    .bind(default_legendary_resistances) // $19
    .fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"combatant_added","encounter_id":encounter_id,"id":c.id}).to_string());
    emit_campaign(&s.db, e.campaign_id, Some(uid),
        "combat.joined",
        &format!("{} joined combat", c.display_name),
        Some(&format!("Init {} · HP {}/{} · AC {}", c.initiative, c.hp_current, c.hp_max, c.ac)),
        Some("encounter"), Some(encounter_id)).await;
    Ok((StatusCode::CREATED, Json(c)))
}

#[derive(Debug, Deserialize)]
struct BulkAddBody {
    pub combatants: Vec<CombatantCreate>,
}

#[derive(Debug, Serialize)]
struct BulkAddResult {
    pub added: usize,
    pub combatants: Vec<Combatant>,
}

async fn bulk_add_combatants(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<BulkAddBody>,
) -> AppResult<Json<BulkAddResult>> {
    let e = fetch(&s, encounter_id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut added = Vec::new();
    for spec in &body.combatants {
        if spec.ref_type != "character" && spec.ref_type != "npc" { continue; }

        let mut npc_stats: Option<combat_engine::NpcStats> = None;
        if spec.ref_type == "npc" && spec.npc_id.is_some() {
            if let Ok(Some(raw)) = sqlx::query_scalar::<_, Value>(
                "select stats from npcs where id = $1 and campaign_id = $2"
            ).bind(spec.npc_id).bind(e.campaign_id).fetch_optional(&s.db).await {
                npc_stats = combat_engine::NpcStats::from_value(&raw);
            }
        }

        let default_hp_max = npc_stats.as_ref().and_then(|n| n.hp.max).unwrap_or(0);
        let default_hp_current = npc_stats.as_ref().and_then(|n| n.hp.current).unwrap_or(default_hp_max);
        let default_ac = npc_stats.as_ref().and_then(|n| n.ac).unwrap_or(10);
        let default_dex = npc_stats.as_ref().map(|n| n.abilities.dex).unwrap_or(10);
        let default_legendary = npc_stats.as_ref()
            .and_then(|n| n.legendary_actions.first()).map(|_| 3).unwrap_or(0);
        let default_resist = npc_stats.as_ref()
            .and_then(|n| n.traits.iter().find(|t| t.name.to_lowercase().contains("legendary resistance")))
            .map(|_| 3).unwrap_or(0);
        let default_rolled = spec.ref_type != "character";

        if let Ok(c) = sqlx::query_as::<_, Combatant>(
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
                         readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
        )
        .bind(encounter_id)
        .bind(&spec.ref_type)
        .bind(spec.character_id)
        .bind(spec.npc_id)
        .bind(&spec.display_name)
        .bind(spec.initiative)
        .bind(spec.dex_tiebreaker)
        .bind(spec.hp_current)
        .bind(spec.hp_max)
        .bind(spec.ac)
        .bind(spec.is_visible)
        .bind(spec.initiative_rolled)
        .bind(default_rolled)
        .bind(default_dex as i16)
        .bind(default_hp_current)
        .bind(default_hp_max)
        .bind(default_ac)
        .bind(default_legendary)
        .bind(default_resist)
        .fetch_one(&s.db).await {
            ws::publish(e.campaign_id, json!({"type":"combatant_added","encounter_id":encounter_id,"id":c.id}).to_string());
            added.push(c);
        }
    }

    Ok(Json(BulkAddResult { added: added.len(), combatants: added }))
}

async fn update_combatant(
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
         where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let campaign_id = row.1;
    let ref_type = row.3;
    let owner = row.4;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    // Non-masters may only edit their own character-combatant, and only
    // cosmetic fields (token_image/color). Everything else is master-only.
    if role != Role::Master {
        if ref_type != "character" || owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
        let cosmetic_only = body.display_name.is_none()
            && body.initiative.is_none() && body.dex_tiebreaker.is_none()
            && body.hp_current.is_none() && body.hp_max.is_none()
            && body.temp_hp.is_none() && body.ac.is_none()
            && body.conditions.is_none() && body.notes.is_none()
            && body.is_visible.is_none()
            && body.token_x.is_none() && body.token_y.is_none()
            && body.token_on_map.is_none()
            && body.token_color.is_none() && body.token_image.is_none()
            && body.action_used.is_none() && body.bonus_action_used.is_none()
            && body.reaction_used.is_none() && body.movement_used_ft.is_none()
            && body.legendary_actions_used.is_none() && body.legendary_resistances_used.is_none();
        if !cosmetic_only {
            return Err(AppError::Forbidden);
        }
    }
    // Character HP/temp/ac is owned by the player via the character sheet;
    // even master cannot overwrite those fields on a character-linked combatant.
    if ref_type == "character" && (
        body.hp_current.is_some() || body.hp_max.is_some() || body.temp_hp.is_some() || body.ac.is_some()
    ) {
        return Err(AppError::BadRequest("character HP/AC is owned by the player sheet".into()));
    }
    let prev_hp = row.2;
    let clear_token_image = body.clear_token_image.unwrap_or(false);
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
             delayed_turn = coalesce($26, delayed_turn)
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .bind(body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.temp_hp).bind(body.ac)
    .bind(body.conditions).bind(body.notes).bind(body.is_visible)
    .bind(body.token_x).bind(body.token_y).bind(body.token_color).bind(body.token_on_map)
    .bind(body.token_image).bind(clear_token_image)
    .bind(body.action_used).bind(body.bonus_action_used).bind(body.reaction_used)
    .bind(body.movement_used_ft).bind(body.legendary_actions_used).bind(body.legendary_resistances_used)
    .bind(body.readied_action).bind(body.cover_bonus).bind(body.delayed_turn)
    .fetch_one(&s.db).await?;
    ws::publish(campaign_id, json!({"type":"combatant_updated","id":id}).to_string());

    // Sync combatant HP/AC back into linked character sheet. Master HP
    // changes also toggle `alive` so a combat revive/kill is reflected in
    // the sheet; dying resets death_saves so the player starts fresh.
    if c.ref_type == "character" {
        if let Some(chid) = c.character_id {
            if body.hp_current.is_some() || body.hp_max.is_some() || body.temp_hp.is_some() || body.ac.is_some() {
                // alive = (hp_current > 0). When reviving from 0, reset death_saves.
                let new_hp = c.hp_current;
                let alive = new_hp > 0;
                // Direct merge without jsonb_strip_nulls, so explicit nulls
                // elsewhere in the sheet (e.g. concentration: null) survive.
                let _ = sqlx::query(
                    r#"update characters set sheet =
                         coalesce(sheet, '{}'::jsonb)
                         || jsonb_build_object(
                              'hp', coalesce(sheet->'hp', '{}'::jsonb)
                                    || jsonb_build_object(
                                         'current', $2::int,
                                         'max',     $3::int,
                                         'temp',    $4::int
                                       ),
                              'ac', $5::int,
                              'alive', $6::bool,
                              'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                               then jsonb_build_object('successes', 0, 'failures', 0)
                                               else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                             end
                            )
                       where id = $1"#,
                )
                .bind(chid)
                .bind(body.hp_current.unwrap_or(c.hp_current))
                .bind(body.hp_max.unwrap_or(c.hp_max))
                .bind(body.temp_hp.unwrap_or(c.temp_hp))
                .bind(body.ac.unwrap_or(c.ac))
                .bind(alive)
                .execute(&s.db).await;
                ws::publish(campaign_id, json!({"type":"character_updated","id":chid}).to_string());
            }
        }
    }

    if prev_hp > 0 && c.hp_current <= 0 {
        emit_campaign(&s.db, campaign_id, None,
            "combat.down",
            &format!("{} dropped to 0 HP", c.display_name),
            None, Some("encounter"), Some(c.encounter_id)).await;
    } else if prev_hp <= 0 && c.hp_current > 0 {
        emit_campaign(&s.db, campaign_id, None,
            "combat.revived",
            &format!("{} is back up ({} HP)", c.display_name, c.hp_current),
            None, Some("encounter"), Some(c.encounter_id)).await;
    }
    Ok(Json(c))
}

async fn delete_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let row: (Uuid, Uuid, Uuid) = sqlx::query_as(
        "select c.id, e.campaign_id, c.encounter_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (_id, campaign_id, encounter_id) = row;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    sqlx::query("delete from combatants where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(campaign_id, json!({"type":"combatant_removed","id":id,"encounter_id":encounter_id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// Move a token on the battle map. Master may move any; players may move only
// their own character's combatant. Coordinates are percentages 0..100.
async fn move_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CombatantMove>,
) -> AppResult<Json<Combatant>> {
    if !body.x.is_finite() || !body.y.is_finite() {
        return Err(AppError::BadRequest("invalid coords".into()));
    }
    let x = body.x.clamp(0.0, 100.0);
    let y = body.y.clamp(0.0, 100.0);

    // Authorize against a fresh read; actual write is a conditional UPDATE
    // below, so the once-per-round rule is enforced atomically regardless of
    // races.
    let row: (Uuid, Option<Uuid>, String, String, Option<f64>, Option<f64>, i32) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, c.ref_type::text, e.status::text,
                  c.token_x, c.token_y, c.movement_used_ft
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, owner, ref_type, enc_status, old_x, old_y, _movement_used) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && (ref_type != "character" || owner != Some(uid)) {
        return Err(AppError::Forbidden);
    }

    let is_player_in_active = role != Role::Master && enc_status == "active";

    // Movement speed enforcement
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);

    let dist_pct = if let (Some(ox), Some(oy)) = (old_x, old_y) {
        let dx = x - ox as f32;
        let dy = y - oy as f32;
        (dx*dx + dy*dy).sqrt()
    } else { 0.0 };
    let dist_ft = (dist_pct * 100.0 / 5.0).round() as i32 * 5;

    let overlays: Vec<(String, Option<f64>, Option<f64>, Option<i32>)> = sqlx::query_as(
        "select zone_type, origin_x, origin_y, radius_ft from encounter_overlays
         where active = true and encounter_id = $1 and zone_type = 'difficult_terrain'")
        .bind(snap.encounter_id).fetch_all(&s.db).await?;
    let in_difficult = overlays.iter().any(|(zt, ox, oy, rad)| {
        if let (Some(cx), Some(cy)) = (ox, oy) {
            let dx = x - *cx as f32;
            let dy = y - *cy as f32;
            let in_zone = if let Some(r) = rad {
                (dx*dx + dy*dy).sqrt() < (*r as f32)
            } else {
                (dx*dx + dy*dy).sqrt() < 5.0
            };
            in_zone && zt == "difficult_terrain"
        } else { false }
    });
    let move_cost = if in_difficult { dist_ft * 2 } else { dist_ft };

    // Atomic update: if player in active combat, gate on token_moved_round < round
    // AND movement speed limit. Forced movement effects bypass the round gate.
    let c: Option<Combatant> = if is_player_in_active {
        sqlx::query_as::<_, Combatant>(
            r#"update combatants c
               set token_x = $2, token_y = $3, token_on_map = true,
                   token_moved_round = e.round,
                   movement_used_ft = c.movement_used_ft + $4
               from encounters e
               where c.id = $1 and c.encounter_id = e.id
                 and (
                   c.token_moved_round is null
                   or c.token_moved_round < e.round
                   or exists (
                     select 1 from combatant_effects ce
                     where ce.combatant_id = c.id
                       and ce.active = true
                       and ce.modifiers @> '{"movement": {}}'::jsonb
                       and not (ce.modifiers @> '{"movement": {"type": "dash_bonus"}}'::jsonb)
                   )
                 )
                 and (c.movement_used_ft + $4 <= $5 or $5 <= 0)
               returning c.id, c.encounter_id, c.ref_type::text as ref_type, c.character_id, c.npc_id, c.display_name,
                         c.initiative, c.dex_tiebreaker, c.hp_current, c.hp_max, c.temp_hp, c.ac, c.conditions, c.notes, c.is_visible, c.turn_order, c.initiative_rolled,
                         c.token_x, c.token_y, c.token_color, c.token_on_map, c.token_image, null::text as portrait_url, c.token_moved_round,
                         c.action_used, c.bonus_action_used, c.reaction_used, c.movement_used_ft,
                         c.legendary_actions_max, c.legendary_actions_used, c.legendary_resistances_max, c.legendary_resistances_used,
                         c.readied_action, c.cover_bonus, c.delayed_turn, c.action_spell_level, c.bonus_action_spell_level, c.last_hit_attack_total, c.last_hit_damage, c.last_hit_attacker, c.spell_being_cast"#)
            .bind(id).bind(x).bind(y).bind(move_cost).bind(speed).fetch_optional(&s.db).await?
    } else {
        sqlx::query_as::<_, Combatant>(
            r#"update combatants set token_x = $2, token_y = $3, token_on_map = true,
                   movement_used_ft = movement_used_ft + $4
               where id = $1
               returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                         initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                         token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#)
            .bind(id).bind(x).bind(y).bind(move_cost).fetch_optional(&s.db).await?
    };
    let c = c.ok_or_else(|| AppError::BadRequest(
        if is_player_in_active && speed > 0 && move_cost > 0 {
            "already moved this round or not enough movement".into()
        } else {
            "already moved this round".into()
        }
    ))?;
    ws::publish(campaign_id, json!({
        "type":"combatant_moved","id":id,"x":x,"y":y,"token_moved_round":c.token_moved_round,"movement_used_ft":c.movement_used_ft
    }).to_string());

    // Auto-trigger ready actions watching for "target_enters_range"
    let enc_id = c.encounter_id;
    auto_trigger_ready_actions_for_event(&s.db, campaign_id, enc_id,
        "target_enters_range", id, id).await;

    Ok(Json(c))
}

async fn start(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut tx = s.db.begin().await?;

    // Reset stale per-round move trackers from any prior session of this encounter.
    sqlx::query(
        "update combatants set token_moved_round = null where encounter_id = $1")
        .bind(id).execute(&mut *tx).await?;

    // auto-add any party characters not already in this encounter. They
    // start with initiative_rolled = false so they sit out until the owner
    // rolls initiative.
    sqlx::query(
        r#"insert into combatants
             (encounter_id, ref_type, character_id, display_name, initiative,
              hp_current, hp_max, ac, initiative_rolled)
           select $1, 'character'::combatant_ref, ch.id, ch.name,
                  0,
                  greatest(1, coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 10)),
                  greatest(1, coalesce((ch.sheet->'hp'->>'max')::int, 10)),
                  coalesce((ch.sheet->>'ac')::int, 10),
                  false
           from characters ch
           where ch.campaign_id = $2
             and coalesce((ch.sheet->>'alive')::boolean, true) = true
             and not exists (
               select 1 from combatants c
               where c.encounter_id = $1 and c.character_id = ch.id
             )"#,
    )
    .bind(id).bind(e.campaign_id).execute(&mut *tx).await?;

    // Re-check for unrolled character combatants INSIDE the transaction.
    // A character created between the request arriving and now will have been
    // auto-added above; they must also roll before the encounter can start.
    let pending_names: Vec<String> = sqlx::query_scalar(
        "select c.display_name from combatants c
         where c.encounter_id = $1
           and c.ref_type = 'character'
           and c.initiative_rolled = false
         order by c.display_name")
        .bind(id).fetch_all(&mut *tx).await?;
    if !pending_names.is_empty() {
        tx.rollback().await?;
        let who = pending_names.join(", ");
        return Err(AppError::BadRequest(
            format!("Waiting for initiative from: {who}")));
    }

    // order only combatants that have rolled — unrolled go to the end, skipped
    // during turn cycling.
    sqlx::query(
        r#"with ordered as (
             select id,
                    row_number() over (
                        order by initiative_rolled desc,
                                 initiative desc,
                                 dex_tiebreaker desc
                    ) - 1 as ord
             from combatants where encounter_id = $1
           )
           update combatants c set turn_order = o.ord
           from ordered o where c.id = o.id"#,
    )
    .bind(id).execute(&mut *tx).await?;

    // first turn = first rolled combatant, or -1 if none rolled yet
    let first_idx: Option<i64> = sqlx::query_scalar(
        "select min(turn_order)::bigint from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    let start_idx = first_idx.unwrap_or(0) as i32;

    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'active', round = 1, turn_index = $2 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(start_idx).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_started","id":id,"round":1,"turn_index":start_idx}).to_string());
    emit_campaign(&s.db, e.campaign_id, None,
        "combat.started", &format!("Combat started: {}", e.name),
        None, Some("encounter"), Some(id)).await;

    // notify every party character owner whose combatant still needs init
    let pending: Vec<(Uuid, String)> = sqlx::query_as(
        r#"select ch.owner_id, c.display_name
           from combatants c join characters ch on ch.id = c.character_id
           where c.encounter_id = $1 and c.initiative_rolled = false"#,
    )
    .bind(id).fetch_all(&s.db).await.unwrap_or_default();
    for (owner, name) in pending {
        emit(&s.db, NewNotif {
            user_id: owner, campaign_id: Some(e.campaign_id),
            kind: "combat.roll_initiative",
            title: "Roll initiative!",
            body: Some(&format!("Combat started — roll initiative for {}", name)),
            ref_kind: Some("encounter"), ref_id: Some(e.id),
        }).await;
    }

    if first_idx.is_some() { notify_turn(&s, &e, 0).await; }
    Ok(Json(e))
}

#[derive(Debug, Deserialize, Validate)]
pub struct SetInitiativeBody {
    pub character_id: Uuid,
    pub initiative: i32,
}

async fn set_initiative(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<SetInitiativeBody>,
) -> AppResult<Json<Combatant>> {
    let e = fetch(&s, encounter_id).await?;
    rbac::require_member(&s.db, uid, e.campaign_id).await?;
    // verify character belongs to user
    let ch: Option<(Uuid, serde_json::Value)> = sqlx::query_as(
        "select owner_id, sheet from characters where id = $1 and campaign_id = $2")
        .bind(body.character_id).bind(e.campaign_id).fetch_optional(&s.db).await?;
    let (owner, sheet) = ch.ok_or(AppError::NotFound)?;
    if owner != uid { return Err(AppError::Forbidden); }
    if sheet.get("alive").and_then(|v| v.as_bool()) == Some(false) {
        return Err(AppError::BadRequest("character is dead".into()));
    }

    let mut tx = s.db.begin().await?;
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants
           set initiative = $3, initiative_rolled = true
           where encounter_id = $1 and character_id = $2
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(encounter_id).bind(body.character_id).bind(body.initiative)
    .fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;

    // re-order all combatants; rolled first, then unrolled (stable-ish)
    sqlx::query(
        r#"with ordered as (
             select id,
                    row_number() over (
                        order by initiative_rolled desc, initiative desc, dex_tiebreaker desc
                    ) - 1 as ord
             from combatants where encounter_id = $1
           )
           update combatants c set turn_order = o.ord
           from ordered o where c.id = o.id"#,
    )
    .bind(encounter_id).execute(&mut *tx).await?;
    tx.commit().await?;

    ws::publish(e.campaign_id, json!({
        "type":"combatant_updated","id":c.id,"initiative":c.initiative,"initiative_rolled":true
    }).to_string());
    emit_campaign(&s.db, e.campaign_id, Some(uid),
        "combat.rolled",
        &format!("{} rolled initiative: {}", c.display_name, c.initiative),
        None, Some("encounter"), Some(encounter_id)).await;
    Ok(Json(c))
}

async fn notify_turn(s: &AppState, e: &Encounter, prev_round: i32) {
    let row: Option<(String, Option<Uuid>, Uuid)> = sqlx::query_as(
        r#"select c.display_name, ch.owner_id, c.id
           from combatants c
           left join characters ch on ch.id = c.character_id
           where c.encounter_id = $1
           order by c.turn_order asc
           offset $2 limit 1"#,
    )
    .bind(e.id).bind(e.turn_index as i64).fetch_optional(&s.db).await.ok().flatten();
    if let Some((name, owner, _cid)) = row {
        if e.round > prev_round {
            emit_campaign(&s.db, e.campaign_id, None,
                "combat.round",
                &format!("Round {} — {}", e.round, name),
                None, Some("encounter"), Some(e.id)).await;
        }
        if let Some(o) = owner {
            emit(&s.db, NewNotif {
                user_id: o, campaign_id: Some(e.campaign_id),
                kind: "combat.your_turn",
                title: "It's your turn!",
                body: Some(&format!("{} — round {}", name, e.round)),
                ref_kind: Some("encounter"), ref_id: Some(e.id),
            }).await;
        }
    }
}

async fn next_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    if rolled == 0 {
        tx.rollback().await?;
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }
    let next_idx = e.turn_index + 1;
    let (new_idx, new_round) = if (next_idx as i64) >= rolled {
        (0, e.round + 1)
    } else {
        (next_idx, e.round)
    };
    let prev_round = e.round;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    // New round started — reset all player token_moved_round so they can move again.
    // Also reset reactions for ALL combatants, legendary actions for legendary creatures,
    // and lair action.
    if new_round > prev_round {
        let _ = sqlx::query(
            "update combatants set token_moved_round = null, reaction_used = false
             where encounter_id = $1")
            .bind(id).execute(&mut *tx).await;
        let _ = sqlx::query(
            "update encounters set lair_action_used = false where id = $1")
            .bind(id).execute(&mut *tx).await;
    }
    // Reset action/bonus/movement for the combatant whose turn is starting.
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(id).fetch_all(&mut *tx).await?;
    if let Some((_, cid)) = combatants.iter().find(|(t, _)| *t == new_idx) {
        let _ = sqlx::query(
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, spell_being_cast = null, legendary_actions_used = 0 where id = $1")
            .bind(cid).execute(&mut *tx).await;
    }
    // Tick down effects based on triggers
    tick_effects(&mut tx, id, prev_round, e.turn_index, new_round, new_idx, e.campaign_id).await?;

    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

/// Tick down combatant effects based on turn/round advancement.
/// `old_turn` = turn_index before change, `new_turn` = after.
async fn tick_effects(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    encounter_id: Uuid,
    old_round: i32,
    old_turn: i32,
    new_round: i32,
    new_turn: i32,
    campaign_id: Uuid,
) -> anyhow::Result<()> {
    // Build a mapping of turn_index -> combatant_id for this encounter
    let combatants: Vec<(i32, Uuid)> = sqlx::query_as(
        "select turn_order, id from combatants where encounter_id = $1 and initiative_rolled = true order by turn_order")
        .bind(encounter_id)
        .fetch_all(&mut **tx).await?;

    if combatants.is_empty() { return Ok(()); }

    let _max_turn = (combatants.len() as i32) - 1;

    // Helper: find combatant ID at a given turn index
    fn cid_at(turn: i32, list: &[(i32, Uuid)]) -> Option<Uuid> {
        list.iter().find(|(t, _)| *t == turn).map(|(_, id)| *id)
    }

    // 1. round_end: tick down when round increments
    if new_round > old_round {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'round_end' and remaining is not null
               and combatant_id in (select id from combatants where encounter_id = $1)")
            .bind(encounter_id)
            .execute(&mut **tx).await?;
    }

    // 2. target_turn_end: tick down for combatant whose turn just ended
    let ended_turn = old_turn;
    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_end' and remaining is not null
               and combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 3. target_turn_start: tick down for combatant whose turn is starting
    let started_turn = new_turn;
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'target_turn_start' and remaining is not null
               and combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 4. caster_turn_end: tick down for effects where caster's turn just ended
    if let Some(cid) = cid_at(ended_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_end' and remaining is not null
               and caster_combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // 5. caster_turn_start: tick down for effects where caster's turn is starting
    if let Some(cid) = cid_at(started_turn, &combatants) {
        sqlx::query(
            "update combatant_effects set remaining = remaining - 1
             where active = true and tick_trigger = 'caster_turn_start' and remaining is not null
               and caster_combatant_id = $1")
            .bind(cid)
            .execute(&mut **tx).await?;
    }

    // Deactivate any effects whose remaining dropped to 0 or below
    let expired_effects: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "select id, combatant_id from combatant_effects
         where active = true and remaining is not null and remaining <= 0
           and combatant_id in (select id from combatants where encounter_id = $1)")
        .bind(encounter_id)
        .fetch_all(&mut **tx).await?;

    if !expired_effects.is_empty() {
        sqlx::query(
            "update combatant_effects set active = false
             where active = true and remaining is not null and remaining <= 0
               and combatant_id in (select id from combatants where encounter_id = $1)")
            .bind(encounter_id)
            .execute(&mut **tx).await?;
        for (_, combatant_id) in &expired_effects {
            ws::publish(campaign_id, json!({
                "type": "effects_changed",
                "combatant_id": combatant_id
            }).to_string());
        }
    }

    // Deactivate overlays whose expiry round/turn has passed
    let expired_overlays: Vec<Uuid> = sqlx::query_scalar(
        "select id from encounter_overlays
         where active = true and encounter_id = $1
           and (expires_at_round is not null and expires_at_round < $2
                or (expires_at_round = $2 and expires_at_turn is not null and expires_at_turn < $3))")
        .bind(encounter_id).bind(new_round).bind(new_turn)
        .fetch_all(&mut **tx).await?;

    if !expired_overlays.is_empty() {
        sqlx::query(
            "update encounter_overlays set active = false
             where active = true and encounter_id = $1
               and (expires_at_round is not null and expires_at_round < $2
                    or (expires_at_round = $2 and expires_at_turn is not null and expires_at_turn < $3))")
            .bind(encounter_id).bind(new_round).bind(new_turn)
            .execute(&mut **tx).await?;
        ws::publish(campaign_id, json!({
            "type": "overlays_expired",
            "ids": expired_overlays
        }).to_string());
    }

    // Per-turn effects for the combatant whose turn is starting.
    if let Some(cid) = cid_at(new_turn, &combatants) {
        let (conditions, hp_current, hp_max): (Vec<String>, i32, i32) = sqlx::query_as(
            "select conditions, hp_current, hp_max from combatants where id = $1")
            .bind(cid).fetch_optional(&mut **tx).await?.unwrap_or_default();
        // Surprised: block full turn, then remove condition
        let is_surprised = has_condition(&conditions, "surprised");
        if is_surprised {
            sqlx::query(
                "update combatants set action_used = true, bonus_action_used = true, movement_used_ft = 9999 where id = $1")
                .bind(cid).execute(&mut **tx).await?;
            let new_conds = remove_condition(conditions.clone(), "surprised");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conds).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_surprised",
                "combatant_id": cid,
            }).to_string());
        }

        // Hazard zones: apply per-turn damage to combatants inside hazard overlays
        let combatant_pos: Option<(f64, f64)> = sqlx::query_as(
            "select token_x, token_y from combatants where id = $1")
            .bind(cid).fetch_optional(&mut **tx).await?;
        if let Some((cx, cy)) = combatant_pos {
            let hazards: Vec<(String, f64, f64, Option<i32>, Option<String>, Option<String>, Option<String>, Option<i32>, bool)> = sqlx::query_as(
                r#"select shape, origin_x, origin_y, radius_ft,
                          hazard_damage_expression, hazard_damage_type,
                          hazard_save_ability, hazard_save_dc, hazard_half_on_save
                   from encounter_overlays
                   where encounter_id = $1 and active = true
                     and zone_type = 'hazard'
                     and hazard_damage_expression is not null"#)
                .bind(encounter_id).fetch_all(&mut **tx).await?;

            for (shape, ox, oy, rad, dmg_expr, dmg_type, save_ability, save_dc, half_on_save) in hazards {
                let r = rad.unwrap_or(20) as f64;
                let in_zone = match shape.as_str() {
                    "circle" => { let dx = cx - ox; let dy = cy - oy; (dx*dx + dy*dy).sqrt() <= r }
                    "cube" | "square" => { (cx - ox).abs() <= r && (cy - oy).abs() <= r }
                    _ => { let dx = cx - ox; let dy = cy - oy; (dx*dx + dy*dy).sqrt() <= r }
                };
                if !in_zone { continue; }

                if let (Some(ref expr), Some(ref dtype)) = (dmg_expr, dmg_type) {
                    let mut rng = rand::rngs::StdRng::from_os_rng();
                    let roll = crate::dice::roll(expr, &mut rng);
                    if let Ok(roll) = roll {
                        let snap_hp: (i32, i32, i32) = sqlx::query_as(
                            "select hp_current, hp_max, temp_hp from combatants where id = $1")
                            .bind(cid).fetch_one(&mut **tx).await?;
                        let dmg = roll.total.max(0);
                        let _ = (save_ability, save_dc, half_on_save); // full save support in overlay_damage endpoint

                        let (new_hp, new_temp) = combat_engine::apply_hp_damage(snap_hp.0, snap_hp.2, dmg);
                        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                            .bind(new_hp).bind(new_temp).bind(cid).execute(&mut **tx).await?;
                        ws::publish(campaign_id, json!({
                            "type": "combatant_hazard_damage",
                            "combatant_id": cid,
                            "damage": dmg,
                            "damage_type": dtype,
                            "hp_after": new_hp,
                        }).to_string());
                    }
                }
            }
        }

        // Regeneration: sum hp_regen_per_turn from active effects modifiers
        let regen: i32 = sqlx::query_scalar(
            r#"select coalesce(sum((modifiers->>'hp_regen_per_turn')::int), 0)::int
               from combatant_effects
               where combatant_id = $1 and active = true
                 and modifiers ? 'hp_regen_per_turn'"#)
            .bind(cid).fetch_optional(&mut **tx).await?.unwrap_or(0);
        if regen > 0 && hp_current > 0 && hp_current < hp_max {
            let new_hp = (hp_current + regen).min(hp_max);
            sqlx::query("update combatants set hp_current = $1 where id = $2")
                .bind(new_hp).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_regenerated",
                "combatant_id": cid,
                "hp_restored": regen,
                "hp_after": new_hp,
            }).to_string());
        }

        // Tick down timed conditions
        let current_conditions = if is_surprised {
            remove_condition(conditions, "surprised")
        } else {
            conditions
        };
        let mut changed = false;
        let new_conditions: Vec<String> = current_conditions.into_iter().filter_map(|c| {
            if let Some(idx) = c.rfind(':') {
                let (name, num_str) = c.split_at(idx);
                if let Ok(n) = num_str[1..].parse::<i32>() {
                    if n <= 1 { changed = true; return None; }
                    changed = true;
                    return Some(format!("{}:{}", name, n - 1));
                }
            }
            Some(c)
        }).collect();
        if changed {
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&new_conditions).bind(cid).execute(&mut **tx).await?;
            ws::publish(campaign_id, json!({
                "type": "combatant_conditions_ticked",
                "combatant_id": cid,
                "conditions": new_conditions,
            }).to_string());
        }
    }

    Ok(())
}

async fn prev_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&mut *tx).await?;
    if rolled == 0 {
        tx.rollback().await?;
        return Err(AppError::BadRequest("waiting for initiative rolls".into()));
    }
    let (new_idx, new_round) = if e.turn_index == 0 {
        if e.round <= 1 { (0, 1) } else { ((rolled - 1) as i32, e.round - 1) }
    } else {
        (e.turn_index - 1, e.round)
    };
    let prev_round = e.round;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2, round = $3 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&mut *tx).await?;
    // Round rewound: clear any stale token_moved_round that now points at a
    // round >= the new round, so players can move again in the restored round.
    if new_round < prev_round {
        let _ = sqlx::query(
            "update combatants set token_moved_round = null
             where encounter_id = $1 and ref_type = 'character' and token_moved_round >= $2")
            .bind(id).bind(new_round).execute(&mut *tx).await;
    }
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

#[derive(Debug, Deserialize)]
pub struct GotoTurnBody { pub turn_index: i32 }

async fn goto_turn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GotoTurnBody>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&s.db).await?;
    if rolled == 0 || body.turn_index < 0 || (body.turn_index as i64) >= rolled {
        return Err(AppError::BadRequest("turn_index out of range".into()));
    }
    let prev_round = e.round;
    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).bind(body.turn_index).fetch_one(&mut *tx).await?;
    tick_effects(&mut tx, id, prev_round, e.turn_index, e.round, body.turn_index, e.campaign_id).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":e.round,"turn_index":body.turn_index}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
}

async fn end_encounter(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;

    let mut tx = s.db.begin().await?;
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'ended' where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_ended","id":id}).to_string());
    emit_campaign(&s.db, e.campaign_id, None,
        "combat.ended", &format!("Combat ended: {}", e.name),
        None, Some("encounter"), Some(id)).await;
    Ok(Json(e))
}

// =====================================================================
// Combat Resolution Endpoints
// =====================================================================

#[derive(Debug, Serialize)]
struct GrappleResult {
    pub success: bool,
    pub attacker_roll: i32,
    pub attacker_total: i32,
    pub defender_roll: i32,
    pub defender_total: i32,
    pub grapple_applied: bool,
}

#[derive(Debug, Deserialize)]
struct GrappleBody {
    pub target_id: Uuid,
}

async fn grapple(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GrappleBody>,
) -> AppResult<Json<GrappleResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let defender_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != defender_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let defender_stats = combat_engine::compute_stats(&defender_snap);

    // Grapple is contested STR (Athletics) vs STR (Athletics) or DEX (Acrobatics)
    let att_ath = attacker_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&attacker_snap, "str"));
    let def_ath = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "str"));
    let def_acr = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "acrobatics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "dex"));
    let def_best = def_ath.max(def_acr); // Defender chooses Athletics or Acrobatics

    let mut rng = rand::rngs::StdRng::from_os_rng();
    // Frightened/Charmed: disadvantage on ability checks too
    let att_expr = if attacker_stats.frightened || attacker_stats.charmed {
        format!("2d20kl1+{}", att_ath)
    } else {
        format!("1d20+{}", att_ath)
    };
    let def_expr = format!("1d20+{}", def_best);

    let att_roll = crate::dice::roll(&att_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let def_roll = crate::dice::roll(&def_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = att_roll.total >= def_roll.total;
    let mut grapple_applied = false;

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if success {
        // Apply grappled condition to defender, grappling condition to attacker
        let mut def_conditions: Vec<String> = defender_snap.conditions.clone();
        if !has_condition(&def_conditions, "grappled") {
            def_conditions.push("grappled".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&def_conditions).bind(body.target_id).execute(&s.db).await?;

        let mut att_conditions: Vec<String> = attacker_snap.conditions.clone();
        if !has_condition(&att_conditions, "grappling") {
            att_conditions.push("grappling".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&att_conditions).bind(id).execute(&s.db).await?;
        grapple_applied = true;
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_grappled",
        "attacker_id": id,
        "target_id": body.target_id,
        "success": success,
    }).to_string());

    Ok(Json(GrappleResult {
        success,
        attacker_roll: att_roll.terms[0].rolls[0],
        attacker_total: att_roll.total,
        defender_roll: def_roll.terms[0].rolls[0],
        defender_total: def_roll.total,
        grapple_applied,
    }))
}

// =====================================================================
// Shove
// =====================================================================

#[derive(Debug, Serialize)]
struct ShoveResult {
    pub success: bool,
    pub attacker_total: i32,
    pub defender_total: i32,
    pub knocked_prone: bool,
    pub pushed_away: bool,
}

#[derive(Debug, Deserialize)]
struct ShoveBody {
    pub target_id: Uuid,
    pub knock_prone: bool, // true = knock prone, false = push away 5ft
}

async fn shove(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ShoveBody>,
) -> AppResult<Json<ShoveResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let defender_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != defender_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let defender_stats = combat_engine::compute_stats(&defender_snap);

    let att_ath = attacker_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&attacker_snap, "str"));
    let def_ath = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "athletics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "str"));
    let def_acr = defender_stats.skill_mods.iter()
        .find(|(s, _)| s == "acrobatics").map(|(_, m)| *m)
        .unwrap_or_else(|| combat_engine::ability_mod(&defender_snap, "dex"));
    let def_best = def_ath.max(def_acr);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = if attacker_stats.frightened || attacker_stats.charmed {
        format!("2d20kl1+{}", att_ath)
    } else {
        format!("1d20+{}", att_ath)
    };
    let def_expr = format!("1d20+{}", def_best);

    let att_roll = crate::dice::roll(&att_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let def_roll = crate::dice::roll(&def_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = att_roll.total >= def_roll.total;
    let mut knocked_prone = false;
    let mut pushed_away = false;

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if success {
        if body.knock_prone {
            let mut conditions = defender_snap.conditions.clone();
            if !has_condition(&conditions, "prone") {
                conditions.push("prone".to_string());
            }
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&conditions).bind(body.target_id).execute(&s.db).await?;
            knocked_prone = true;
        } else {
            // Push 5ft away (1 grid cell) — simplified: just move token 1 cell in a direction
            if let (Some(tx), Some(ty)) = (defender_snap.token_x, defender_snap.token_y) {
                // Push in direction away from attacker
                let dx = tx - attacker_snap.token_x.unwrap_or(tx);
                let dy = ty - attacker_snap.token_y.unwrap_or(ty);
                let len = (dx*dx + dy*dy).sqrt().max(0.01);
                let push_pct = 5.0; // approximate 5ft as 5% of map
                let new_x = (tx + (dx/len) * push_pct).clamp(0.0, 100.0);
                let new_y = (ty + (dy/len) * push_pct).clamp(0.0, 100.0);
                sqlx::query("update combatants set token_x = $1, token_y = $2 where id = $3")
                    .bind(new_x).bind(new_y).bind(body.target_id).execute(&s.db).await?;
            }
            pushed_away = true;
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_shoved",
        "attacker_id": id,
        "target_id": body.target_id,
        "success": success,
        "knocked_prone": knocked_prone,
        "pushed_away": pushed_away,
    }).to_string());

    Ok(Json(ShoveResult {
        success,
        attacker_total: att_roll.total,
        defender_total: def_roll.total,
        knocked_prone,
        pushed_away,
    }))
}

// =====================================================================
// Stand Up (remove prone)
// =====================================================================

async fn stand_up(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, Vec<String>, i32) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, c.conditions, c.movement_used_ft
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner, conditions, movement_used) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    if !has_condition(&conditions, "prone") {
        return Err(AppError::BadRequest("not prone".into()));
    }

    // Compute speed to know half-speed cost
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let speed = stats.speed.max(0);
    let stand_cost = (speed as f32 / 2.0).ceil() as i32;

    if movement_used + stand_cost > speed && speed > 0 {
        return Err(AppError::BadRequest(format!(
            "not enough movement to stand up (used {}ft + {}ft > {}ft)",
            movement_used, stand_cost, speed
        )));
    }

    let new_conditions: Vec<String> = remove_condition(conditions, "prone");

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             conditions = $1,
             movement_used_ft = movement_used_ft + $2
           where id = $3
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(&new_conditions)
    .bind(stand_cost)
    .bind(id)
    .fetch_one(&s.db)
    .await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_stood_up",
        "combatant_id": id,
        "movement_cost": stand_cost,
    }).to_string());

    Ok(Json(c))
}

// =====================================================================
// Grapple Escape
// =====================================================================

#[derive(Debug, Deserialize)]
struct GrappleEscapeBody {
    pub grappler_id: Uuid,
}

#[derive(Debug, Serialize)]
struct GrappleEscapeResult {
    pub success: bool,
    pub escapee_roll: i32,
    pub escapee_total: i32,
    pub grappler_roll: i32,
    pub grappler_total: i32,
    pub escaped: bool,
}

async fn grapple_escape(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<GrappleEscapeBody>,
) -> AppResult<Json<GrappleEscapeResult>> {
    let escapee_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let grappler_snap = combat_engine::load_snapshot(&s.db, body.grappler_id).await?;

    if escapee_snap.encounter_id != grappler_snap.encounter_id {
        return Err(AppError::BadRequest("not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(escapee_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Action economy check
    let action_used: bool = sqlx::query_scalar("select action_used from combatants where id = $1")
        .bind(id).fetch_one(&s.db).await?;
    if action_used {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // Escapee must actually be grappled
    if !has_condition(&escapee_snap.conditions, "grappled") {
        return Err(AppError::BadRequest("not grappled".into()));
    }

    let mut rng = rand::rngs::StdRng::from_os_rng();

    // Escapee uses Athletics or Acrobatics; we use the higher skill mod
    let escapee_stats = combat_engine::compute_stats(&escapee_snap);
    let athletics = escapee_stats.skill_mods.iter().find(|(s, _)| s == "athletics").map(|(_, m)| *m).unwrap_or(0);
    let acrobatics = escapee_stats.skill_mods.iter().find(|(s, _)| s == "acrobatics").map(|(_, m)| *m).unwrap_or(0);
    let escapee_mod = athletics.max(acrobatics);

    let grappler_stats = combat_engine::compute_stats(&grappler_snap);
    let grappler_athletics = grappler_stats.skill_mods.iter().find(|(s, _)| s == "athletics").map(|(_, m)| *m).unwrap_or(0);

    let esc_expr = format!("1d20+{}", escapee_mod);
    let grap_expr = format!("1d20+{}", grappler_athletics);

    let esc_roll = crate::dice::roll(&esc_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let grap_roll = crate::dice::roll(&grap_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = esc_roll.total >= grap_roll.total;
    let mut escaped = false;

    if success {
        // Remove grappled from escapee, remove grappling from grappler
        let esc_conditions = remove_condition(escapee_snap.conditions.clone(), "grappled");
        sqlx::query("update combatants set conditions = $1, action_used = true where id = $2")
            .bind(&esc_conditions).bind(id).execute(&s.db).await?;

        let grap_conditions = remove_condition(grappler_snap.conditions.clone(), "grappling");
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&grap_conditions).bind(body.grappler_id).execute(&s.db).await?;

        escaped = true;
    } else {
        sqlx::query("update combatants set action_used = true where id = $1")
            .bind(id).execute(&s.db).await?;
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_grapple_escape",
        "escapee_id": id,
        "grappler_id": body.grappler_id,
        "success": success,
        "escaped": escaped,
    }).to_string());

    Ok(Json(GrappleEscapeResult {
        success,
        escapee_roll: esc_roll.terms[0].rolls[0],
        escapee_total: esc_roll.total,
        grappler_roll: grap_roll.terms[0].rolls[0],
        grappler_total: grap_roll.total,
        escaped,
    }))
}

// =====================================================================
// Lair Action
// =====================================================================

async fn lair_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if e.lair_action_used {
        return Err(AppError::BadRequest("lair action already used this round".into()));
    }
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set lair_action_used = true where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, grid_type, lair_action_used, updated_at")
        .bind(id).fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({
        "type": "lair_action",
        "encounter_id": id,
        "round": e.round,
    }).to_string());
    Ok(Json(e))
}

// =====================================================================
// Legendary Action
// =====================================================================

#[derive(Debug, Serialize)]
struct LegendaryActionResult {
    pub legendary_actions_used: i32,
    pub legendary_actions_max: i32,
}

async fn legendary_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LegendaryActionResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select e.campaign_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let row: (i32, i32) = sqlx::query_as(
        "select legendary_actions_used, legendary_actions_max from combatants where id = $1")
        .bind(id).fetch_one(&s.db).await?;
    let (used, max) = row;
    if used >= max {
        return Err(AppError::BadRequest("no legendary actions remaining".into()));
    }

    sqlx::query("update combatants set legendary_actions_used = legendary_actions_used + 1 where id = $1")
        .bind(id).execute(&s.db).await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_legendary_action",
        "combatant_id": id,
        "legendary_actions_used": used + 1,
        "legendary_actions_max": max,
    }).to_string());

    Ok(Json(LegendaryActionResult {
        legendary_actions_used: used + 1,
        legendary_actions_max: max,
    }))
}

// =====================================================================
// NPC Multiattack Parsing
// =====================================================================

#[derive(Debug, Clone, Serialize)]
struct ParsedMultiAttack {
    /// List of sub-attacks parsed from the NPC multiattack action description
    pub attacks: Vec<ParsedSubAttack>,
}

#[derive(Debug, Clone, Serialize, Default)]
struct ParsedSubAttack {
    pub name: String,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    #[serde(default)]
    pub damage_type: String,
    pub label: Option<String>,
}

/// Parse a multiattack description like "2 claws + 1 bite" or
/// "makes two attacks: one with its bite and one with its claws"
/// and look up the corresponding attack actions in the NPC's actions list.
fn parse_npc_multiattack(
    description: &str,
    actions: &[serde_json::Value],
) -> Vec<ParsedSubAttack> {
    let desc = description.to_lowercase();
    let mut attack_names: Vec<(u32, String)> = Vec::new(); // (count, name)

    // Pattern 1: "2 claws + 1 bite" or "two claws + one bite"
    if desc.contains('+') || desc.chars().filter(|&c| c.is_ascii_digit()).count() > 0 {
        // Split by '+' and extract "N name"
        for part in desc.split('+') {
            let part = part.trim();
            // Extract leading number word/digit
            let (cnt, nm): (u32, String) = if let Some(d) = part.chars().next().and_then(|c| c.to_digit(10)) {
                (d, part.chars().skip(1).collect::<String>().trim().to_string())
            } else {
                // Check for word numbers: "two claws", "one bite"
                let words: Vec<&str> = part.split_whitespace().collect();
                if words.len() >= 2 {
                    let c = match words[0] {
                        "one" | "a" | "an" => 1,
                        "two" => 2,
                        "three" => 3,
                        "four" => 4,
                        "five" => 5,
                        _ => 1,
                    };
                    (c, words[1..].join(" "))
                } else {
                    (1, part.to_string())
                }
            };
            if !nm.is_empty() {
                attack_names.push((cnt, nm));
            }
        }
    }

    // Pattern 2: "makes [N] attacks: one with its [weapon], one with its [weapon]"
    if attack_names.is_empty() {
        if let Some(attacks_part) = desc.split(':').nth(1) {
            for segment in attacks_part.split(',') {
                let seg = segment.trim();
                for prefix in &["one with its ", "one with his ", "one with her ", "one "] {
                    if let Some(rest) = seg.strip_prefix(prefix) {
                        let name = rest.trim_end_matches(&['.', ' '][..]).to_string();
                        if !name.is_empty() {
                            attack_names.push((1, name));
                        }
                        break;
                    }
                }
            }
        }
    }

    // Pattern 3: just "makes [N] melee/ranged attacks" — repeat the first melee attack N times
    if attack_names.is_empty() {
        let p3_count = desc.split_whitespace().find_map(|w| {
            w.chars().next().and_then(|c| c.to_digit(10))
        }).unwrap_or(1);
        // Get the first weapon-slot attack
        if let Some(first_atk) = actions.iter().find(|a| {
            let aname = a.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            aname != "multiattack"
        }) {
            let aname = first_atk.get("name").and_then(|v| v.as_str()).unwrap_or("attack").to_string();
            attack_names.push((p3_count, aname));
        }
    }

    // Resolve attack_names to actual attack data from actions
    let mut results: Vec<ParsedSubAttack> = Vec::new();
    let actions_lower: Vec<(String, &serde_json::Value)> = actions.iter()
        .filter_map(|a| {
            let name = a.get("name").and_then(|v| v.as_str())?;
            Some((name.to_lowercase(), a))
        })
        .collect();

    for (count, name_hint) in attack_names {
        // Find the closest matching action
        let hint = name_hint.trim().to_lowercase();
        // Try exact match first
        let found = actions_lower.iter().find(|(n, _)| *n == hint)
            .or_else(|| actions_lower.iter().find(|(n, _)| n.contains(&hint) || hint.contains(n)))
            .or_else(|| actions_lower.iter().find(|(n, _)| n != &"multiattack"));

        if let Some((_, action)) = found {
            let atk_bonus = action.get("attack_bonus").and_then(|v| v.as_i64()).unwrap_or(0);
            let dam = action.get("damage").and_then(|v| v.as_str()).unwrap_or("1d4");
            let dtype = action.get("damage_type").and_then(|v| v.as_str()).unwrap_or("bludgeoning");
            let aname = action.get("name").and_then(|v| v.as_str()).unwrap_or("Attack");
            for _ in 0..count {
                results.push(ParsedSubAttack {
                    name: aname.to_string(),
                    attack_expression: Some(format!("1d20+{}", atk_bonus)),
                    damage_expression: Some(dam.to_string()),
                    damage_type: dtype.to_string(),
                    label: Some(aname.to_string()),
                });
            }
        }
    }

    results
}

/// Try to parse NPC multiattack from combatant ID, returning ParsedMultiAttack or an error string.
async fn try_parse_npc_multiattack(db: &sqlx::PgPool, combatant_id: Uuid) -> Result<ParsedMultiAttack, String> {
    let npc_id: Option<Uuid> = sqlx::query_scalar(
        "select npc_id from combatants where id = $1")
        .bind(combatant_id).fetch_optional(db).await
        .map_err(|e| e.to_string())?
        .flatten()
        .ok_or_else(|| "not an NPC combatant".to_string())?;

    let npc_stats: Option<serde_json::Value> = sqlx::query_scalar(
        "select stats from npcs where id = $1")
        .bind(npc_id).fetch_optional(db).await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "NPC not found".to_string())?;

    let stats = npc_stats.ok_or_else(|| "NPC has no stats".to_string())?;
    let actions: Vec<serde_json::Value> = stats.get("actions")
        .and_then(|a| a.as_array())
        .cloned()
        .unwrap_or_default();

    let multiattack_action = actions.iter().find(|a| {
        a.get("name").and_then(|v| v.as_str())
            .map(|n| n.to_lowercase() == "multiattack")
            .unwrap_or(false)
    }).ok_or_else(|| "NPC has no Multiattack action".to_string())?;

    let description = multiattack_action.get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    if description.is_empty() {
        return Err("Multiattack action has no description".to_string());
    }

    let attacks = parse_npc_multiattack(description, &actions);
    if attacks.is_empty() {
        return Err(format!("could not parse multiattack description: {}", description));
    }

    Ok(ParsedMultiAttack { attacks })
}

async fn parse_multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ParsedMultiAttack>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        r#"select e.campaign_id from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let parsed = try_parse_npc_multiattack(&s.db, id).await
        .map_err(|e| AppError::BadRequest(e))?;
    Ok(Json(parsed))
}

// =====================================================================
// Multiattack
// =====================================================================

#[derive(Debug, Deserialize)]
struct MultiAttackTarget {
    pub target_id: Uuid,
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    pub damage_type: String,
    pub damage_die: Option<String>,
    pub ability: Option<String>,
    pub weapon_id: Option<String>,
    pub label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MultiAttackBody {
    pub targets: Vec<MultiAttackTarget>,
}

#[derive(Debug, Serialize)]
struct MultiAttackResult {
    pub results: Vec<combat_engine::AttackResult>,
    pub targets_hit: usize,
    pub total_damage: i32,
}

async fn multiattack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MultiAttackBody>,
) -> AppResult<Json<MultiAttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Auto-populate attacks from NPC parsed multiattack if targets have no expressions
    let needs_auto = body.targets.iter().all(|t| t.attack_expression.is_none() && t.weapon_id.is_none());
    let targets: Vec<MultiAttackTarget> = if !needs_auto {
        body.targets.iter().map(|t| MultiAttackTarget {
            target_id: t.target_id,
            attack_expression: t.attack_expression.clone(),
            damage_expression: t.damage_expression.clone(),
            damage_type: t.damage_type.clone(),
            damage_die: t.damage_die.clone(),
            ability: t.ability.clone(),
            weapon_id: t.weapon_id.clone(),
            label: t.label.clone(),
        }).collect()
    } else if let Ok(ParsedMultiAttack { attacks }) = try_parse_npc_multiattack(&s.db, id).await {
        if attacks.is_empty() {
            return Err(AppError::BadRequest("no targets and could not parse NPC multiattack".into()));
        }
        body.targets.iter().enumerate().map(|(i, t)| {
            let atk = attacks.get(i).cloned().unwrap_or_default();
            MultiAttackTarget {
                target_id: t.target_id,
                attack_expression: t.attack_expression.clone().or(atk.attack_expression),
                damage_expression: t.damage_expression.clone().or(atk.damage_expression),
                damage_type: if t.damage_type == "slashing" && !atk.damage_type.is_empty() { atk.damage_type } else { t.damage_type.clone() },
                damage_die: t.damage_die.clone(),
                ability: t.ability.clone(),
                weapon_id: t.weapon_id.clone(),
                label: t.label.clone().or(atk.label),
            }
        }).collect()
    } else {
        return Err(AppError::BadRequest("no targets specified".into()));
    };

    if targets.is_empty() {
        return Err(AppError::BadRequest("no targets specified".into()));
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let mut results = Vec::new();
    let mut total_damage = 0i32;
    let mut targets_hit = 0usize;

    for t in &targets {
        let target_snap = combat_engine::load_snapshot(&s.db, t.target_id).await?;
        if target_snap.encounter_id != attacker_snap.encounter_id {
            continue;
        }
        let target_stats = combat_engine::compute_stats(&target_snap);

        let req = combat_engine::AttackReq {
            target_id: t.target_id,
            attack_expression: t.attack_expression.clone(),
            damage_expression: t.damage_expression.clone(),
            damage_type: t.damage_type.clone(),
            damage_die: t.damage_die.clone(),
            ability: t.ability.clone(),
            proficient: Some(true),
            advantage: false,
            disadvantage: false,
            cover: None,
            is_spell_attack: false,
            is_magical: false,
            label: t.label.clone(),
            weapon_id: t.weapon_id.clone(),
            extra_damage_expression: None,
            extra_damage_type: None,
            power_attack: false,
            reckless: false,
            bless_dice: None,
            bardic_inspiration_dice: None,
        };

        match combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats) {
            Ok(res) => {
                if res.hit {
                    targets_hit += 1;
                    total_damage += res.damage_applied;
                }
                results.push(res);
            }
            Err(_) => continue,
        }
    }

    // Apply all damage in one transaction and atomically consume action
    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    for (i, t) in body.targets.iter().enumerate() {
        if let Some(res) = results.get(i) {
            if res.hit {
                sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
                    .bind(res.target_hp_after)
                    .bind(res.target_temp_hp_after)
                    .bind(t.target_id)
                    .execute(&mut *tx).await?;
                if res.concentration_broken {
                    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                        .bind(t.target_id).execute(&mut *tx).await?;
                }
                let _ = sync_combatant_hp_to_sheet(&s.db, t.target_id, res.target_hp_after, res.target_temp_hp_after).await;
                let _ = sqlx::query(
                    "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
                    .bind(attacker_snap.encounter_id)
                    .bind(round)
                    .bind(id)
                    .bind(t.target_id)
                    .bind(format!("Multiattack: {} damage", res.damage_applied))
                    .bind(-res.damage_applied)
                    .bind(t.label.as_deref())
                    .execute(&mut *tx).await;
            }
        }
    }
    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_multiattack",
        "attacker_id": id,
        "targets_hit": targets_hit,
        "total_damage": total_damage,
    }).to_string());

    Ok(Json(MultiAttackResult { results, targets_hit, total_damage }))
}





// =====================================================================
// Trigger Ready Action
// =====================================================================

async fn trigger_ready(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Option<String>, bool, bool, String) = sqlx::query_as(
        r#"select e.campaign_id, c.readied_action, c.action_used, c.reaction_used, e.status::text
           from combatants c
           join encounters e on e.id = c.encounter_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, readied, _action_used, reaction_used, status) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if readied.is_none() {
        return Err(AppError::BadRequest("no readied action to trigger".into()));
    }
    if reaction_used {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    // Consume reaction, clear readied_action, and reset action_used so the stored action can be performed.
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set
             reaction_used = true,
             readied_action = null,
             action_used = false
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#)
        .bind(id).fetch_one(&s.db).await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_readied_triggered",
        "combatant_id": id,
        "readied_action": readied,
    }).to_string());

    Ok(Json(c))
}

// =====================================================================
// Class Feature Activation
// =====================================================================

#[derive(Debug, Deserialize)]
struct ClassFeatureBody {
    pub feature: String,
    #[serde(alias = "_target_id")]
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
struct ClassFeatureResult {
    pub feature: String,
    pub success: bool,
    pub message: String,
    pub hp_after: Option<i32>,
    pub effect_applied: bool,
}

async fn class_feature(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ClassFeatureBody>,
) -> AppResult<Json<ClassFeatureResult>> {
    let row: (Uuid, Option<Uuid>, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, ch.owner_id, e.status::text, c.character_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let (campaign_id, owner, status, character_id) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let feature = body.feature.to_lowercase();
    let message: String;
    let mut hp_after = None;
    let effect_applied: bool;

    match feature.as_str() {
        "action_surge" => {
            sqlx::query("update combatants set action_used = false where id = $1")
                .bind(id).execute(&s.db).await?;
            message = "Action Surge! You can take an additional action.".into();
            effect_applied = true;
        }
        "second_wind" => {
            if let Some(chid) = character_id {
                let fighter_level: i32 = sqlx::query_scalar(
                    "select coalesce((sheet->>'level_total')::int, 1) from characters where id = $1")
                    .bind(chid).fetch_one(&s.db).await?;
                let mut rng = rand::rngs::StdRng::from_os_rng();
                let roll = crate::dice::roll(&format!("1d10+{}", fighter_level), &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;
                let heal = roll.total;
                let snap = combat_engine::load_snapshot(&s.db, id).await?;
                let new_hp = (snap.hp_current + heal).min(snap.hp_max);
                sqlx::query("update combatants set hp_current = $1 where id = $2")
                    .bind(new_hp).bind(id).execute(&s.db).await?;
                let _ = sync_combatant_hp_to_sheet(&s.db, id, new_hp, snap.temp_hp).await;
                hp_after = Some(new_hp);
                message = format!("Second Wind heals {} HP", heal);
                effect_applied = true;
            } else {
                return Err(AppError::BadRequest("Second Wind requires a linked character".into()));
            }
        }
        "rage" => {
            // Compute rage damage bonus from barbarian level (PHB p.48)
            let barbarian_level: i32 = if let Some(chid) = character_id {
                sqlx::query_scalar(
                    r#"select coalesce((
                         select (elem->>'level')::int
                         from characters, jsonb_array_elements(sheet->'classes') as elem
                         where id = $1 and lower(elem->>'name') = 'barbarian'
                         limit 1
                       ), 1)"#)
                    .bind(chid).fetch_optional(&s.db).await?.unwrap_or(1)
            } else { 1 };
            let rage_dmg_bonus = if barbarian_level >= 9 { 3 } else if barbarian_level >= 16 { 4 } else { 2 };

            // Remove any existing rage effect first (idempotent)
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Rage' and active = true")
                .bind(id).execute(&s.db).await?;

            // Apply rage as a combatant effect (lasts until end of combat / manual removal)
            let rage_mods = serde_json::json!({
                "damage_bonus": rage_dmg_bonus,
                "damage_resistance": ["bludgeoning", "piercing", "slashing"],
                "attack_advantage": true
            });
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Rage', 'buff', 'swords', 'manual', null, null, 'round_end',
                           false, true, $2, 'ability')"#)
                .bind(id).bind(rage_mods).execute(&s.db).await?;

            // Also set rage condition for awareness
            let mut conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
                .bind(id).fetch_one(&s.db).await?;
            if !has_condition(&conditions, "rage") {
                conditions.push("rage".to_string());
            }
            sqlx::query("update combatants set conditions = $1, bonus_action_used = true where id = $2")
                .bind(&conditions).bind(id).execute(&s.db).await?;
            message = format!("Rage! +{} damage, BPS resistance, STR advantage.", rage_dmg_bonus);
            effect_applied = true;
        }
        "lay_on_hands" => {
            // body.target_id is the target; amount comes from a sheet resource "Lay on Hands"
            let target_id = body.target_id.ok_or(AppError::BadRequest("target_id required for Lay on Hands".into()))?;
            let chid = character_id.ok_or(AppError::BadRequest("Lay on Hands requires a linked character".into()))?;

            // Read the Lay on Hands pool remaining from sheet.resources
            let pool: Option<serde_json::Value> = sqlx::query_scalar(
                r#"select elem from characters, jsonb_array_elements(sheet->'resources') as elem
                   where id = $1 and lower(elem->>'name') like '%lay on hands%'
                   limit 1"#)
                .bind(chid).fetch_optional(&s.db).await?;
            let (pool_current, pool_id): (i32, String) = if let Some(p) = pool {
                let cur = p.get("current").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
                let rid = p.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                (cur, rid)
            } else {
                return Err(AppError::BadRequest("No Lay on Hands pool found on character sheet".into()));
            };
            if pool_current <= 0 {
                return Err(AppError::BadRequest("Lay on Hands pool is empty".into()));
            }

            // Amount to heal = min(pool, target missing HP), at least 1
            let target_snap = combat_engine::load_snapshot(&s.db, target_id).await?;
            let missing = (target_snap.hp_max - target_snap.hp_current).max(0);
            let heal_amt = pool_current.min(missing).max(1);
            let new_hp = (target_snap.hp_current + heal_amt).min(target_snap.hp_max);

            // Deduct from pool
            sqlx::query(
                r#"update characters set sheet = jsonb_set(
                     sheet,
                     ('{resources,' || idx - 1 || ',current}')::text[],
                     to_jsonb($2::int)
                   )
                   from (select position - 1 as idx
                         from characters, jsonb_array_elements(sheet->'resources') with ordinality as t(elem, position)
                         where id = $1 and lower(t.elem->>'name') like '%lay on hands%'
                         limit 1) sub
                   where id = $1"#)
                .bind(chid).bind(pool_current - heal_amt).execute(&s.db).await?;

            // Apply HP to target
            sqlx::query("update combatants set hp_current = $1 where id = $2")
                .bind(new_hp).bind(target_id).execute(&s.db).await?;
            let _ = sync_combatant_hp_to_sheet(&s.db, target_id, new_hp, target_snap.temp_hp).await;

            hp_after = Some(new_hp);
            let _ = pool_id; // used implicitly
            message = format!("Lay on Hands heals {} HP (pool: {} remaining)", heal_amt, pool_current - heal_amt);
            effect_applied = true;
        }
        "uncanny_dodge" => {
            let reaction_used: bool = sqlx::query_scalar("select reaction_used from combatants where id = $1")
                .bind(id).fetch_one(&s.db).await?;
            if reaction_used {
                return Err(AppError::BadRequest("reaction already used".into()));
            }
            sqlx::query("update combatants set reaction_used = true where id = $1")
                .bind(id).execute(&s.db).await?;
            message = "Uncanny Dodge! Damage from attacker halved.".into();
            effect_applied = true;
        }
        _ => {
            return Err(AppError::BadRequest(format!("unknown class feature: {}", body.feature)));
        }
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_class_feature",
        "combatant_id": id,
        "feature": feature,
        "message": &message,
        "hp_after": hp_after,
    }).to_string());

    Ok(Json(ClassFeatureResult {
        feature: body.feature,
        success: effect_applied,
        message,
        hp_after,
        effect_applied,
    }))
}

// =====================================================================
// Cunning Action / Action Use Body
// =====================================================================

async fn patch_effects(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<PatchEffectsBody>,
) -> AppResult<Json<PatchEffectsResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let mut affected = 0usize;

    if let Some(ref name) = body.remove_by_name {
        for cid in &body.combatant_ids {
            let r = sqlx::query(
                "update combatant_effects set active = false where name = $1 and combatant_id = $2 and active = true")
                .bind(name).bind(cid).execute(&s.db).await?;
            affected += r.rows_affected() as usize;
        }
    }

    if let Some(active) = body.set_active {
        for cid in &body.combatant_ids {
            if let Some(ref name) = body.remove_by_name {
                let r = sqlx::query(
                    "update combatant_effects set active = $1 where combatant_id = $2 and name = $3")
                    .bind(active).bind(cid).bind(name).execute(&s.db).await?;
                affected += r.rows_affected() as usize;
            } else {
                let r = sqlx::query(
                    "update combatant_effects set active = $1 where combatant_id = $2 and active != $1")
                    .bind(active).bind(cid).execute(&s.db).await?;
                affected += r.rows_affected() as usize;
            }
        }
    }

    if let Some(ref eff) = body.add_effect {
        for cid in &body.combatant_ids {
            let name = eff.get("name").and_then(|v| v.as_str()).unwrap_or("Effect");
            let modifiers = eff.get("modifiers").cloned().unwrap_or(json!({}));
            let kind = eff.get("kind").and_then(|v| v.as_str()).unwrap_or("buff");
            let icon = eff.get("icon").and_then(|v| v.as_str()).unwrap_or("sparkles");
            let _ = sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, $2, $3, $4, 'manual', null, null, 'round_end',
                           false, true, $5, 'manual')"#,
            )
            .bind(cid).bind(name).bind(kind).bind(icon).bind(&modifiers)
            .execute(&s.db).await?;
            affected += 1;
        }
    }

    if affected > 0 {
        for cid in &body.combatant_ids {
            ws::publish(campaign_id, json!({
                "type": "effects_changed",
                "combatant_id": cid
            }).to_string());
        }
    }

    Ok(Json(PatchEffectsResult { affected }))
}



