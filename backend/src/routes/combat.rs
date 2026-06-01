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
    routing::{get, post},
};
use serde_json::Value;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/encounters", get(list).post(create))
        .route("/encounters/{id}", get(read).patch(update).delete(delete))
        .route("/encounters/{id}/combatants", get(list_combatants).post(add_combatant))
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
        .route("/combatants/{id}/trigger-ready", post(trigger_ready))
        .route("/combatants/{id}/class-feature", post(class_feature))
        .route("/combatants/{id}/two-weapon-fight", post(two_weapon_fight))
        .route("/combatants/{id}/dash", post(dash))
        .route("/combatants/{id}/hide", post(hide))
        .route("/combatants/{id}/search", post(search_action))
        .route("/combatants/{id}/use-object", post(use_object))
        .route("/combatants/{id}/conditions", post(add_condition))
        .route("/encounters/{id}/overlay-damage", post(overlay_damage))
        .route("/encounters/{id}/surprise", post(surprise_round))
        .route("/encounters/{id}/difficulty", get(encounter_difficulty))
        .route("/encounters/{id}/flanking", get(check_flanking))
        .route("/encounters/{id}/cover", get(calculate_cover))
        .route("/encounters/{id}/events", get(list_events))
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

#[derive(Debug, Serialize, FromRow)]
pub struct Overlay {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub kind: String,
    pub shape: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: String,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub active: bool,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct OverlayCreate {
    pub kind: String, // aoe | zone
    pub shape: String, // circle | cone | line | cube | polygon
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: Option<bool>,
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

// =====================================================================
// Overlay endpoints
// =====================================================================

async fn list_overlays(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Overlay>>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let rows: Vec<Overlay> = sqlx::query_as::<_, Overlay>(
        r#"select id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                  radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                  active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                  hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save
           from encounter_overlays
           where encounter_id = $1 and active = true
           order by created_at desc"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn create_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<OverlayCreate>,
) -> AppResult<(StatusCode, Json<Overlay>)> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let o: Overlay = sqlx::query_as::<_, Overlay>(
        r#"insert into encounter_overlays
           (encounter_id, kind, shape, origin_x, origin_y, end_x, end_y, radius_ft, length_ft, width_ft, angle_deg, points,
            color, label, zone_type, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id,
            hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, hazard_half_on_save)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
           returning id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                     radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                     active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                     hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save"#,
    )
    .bind(encounter_id)
    .bind(&body.kind)
    .bind(&body.shape)
    .bind(body.origin_x)
    .bind(body.origin_y)
    .bind(body.end_x)
    .bind(body.end_y)
    .bind(body.radius_ft)
    .bind(body.length_ft)
    .bind(body.width_ft)
    .bind(body.angle_deg)
    .bind(body.points)
    .bind(body.color.as_deref().unwrap_or("rgba(255,0,0,0.25)"))
    .bind(body.label.as_deref())
    .bind(body.zone_type.as_deref())
    .bind(body.expires_at_round)
    .bind(body.expires_at_turn)
    .bind(body.source_spell_slug.as_deref())
    .bind(body.created_by_combatant_id)
    .bind(body.hazard_damage_expression.as_deref())
    .bind(body.hazard_damage_type.as_deref())
    .bind(body.hazard_save_ability.as_deref())
    .bind(body.hazard_save_dc)
    .bind(body.hazard_half_on_save.unwrap_or(false))
    .fetch_one(&s.db)
    .await?;

    ws::publish(campaign_id, json!({"type":"overlay_added","encounter_id":encounter_id,"id":o.id}).to_string());
    Ok((StatusCode::CREATED, Json(o)))
}

async fn delete_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((encounter_id, overlay_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    sqlx::query("update encounter_overlays set active = false where id = $1 and encounter_id = $2")
        .bind(overlay_id).bind(encounter_id).execute(&s.db).await?;

    ws::publish(campaign_id, json!({"type":"overlay_removed","encounter_id":encounter_id,"id":overlay_id}).to_string());
    Ok(StatusCode::NO_CONTENT)
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

/// Parse a spell's range_text into feet for distance validation.
/// Returns None for unlimited / self / touch (no distance check).
fn parse_spell_range_ft(range_text: &str) -> Option<i32> {
    let s = range_text.trim().to_lowercase();
    if s == "self" || s == "touch" || s.contains("unlimited") || s.contains("special") {
        return None;
    }
    // "60 feet", "120 feet", "1 mile", etc.
    if s.contains("mile") {
        let n: i32 = s.split_whitespace().next()?.parse().ok()?;
        return Some(n * 5280);
    }
    let first = s.split_whitespace().next()?;
    first.parse::<i32>().ok()
}

/// Returns true if this combatant is immune to the given condition (lowercase name).
/// Checks NPC condition_immunities, character sheet condition_immunities, and creature-type rules.
async fn check_condition_immunity(db: &sqlx::PgPool, combatant_id: Uuid, condition: &str) -> Result<bool, crate::error::AppError> {
    // Fetch NPC stats JSONB and character sheet JSONB
    let row: Option<(Option<serde_json::Value>, Option<serde_json::Value>)> = sqlx::query_as(
        r#"select n.stats, ch.sheet
           from combatants c
           left join npcs n on n.id = c.npc_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(combatant_id).fetch_optional(db).await?;

    let (npc_stats_raw, char_sheet) = row.unwrap_or((None, None));

    // NPC explicit condition_immunities list
    if let Some(ref raw) = npc_stats_raw {
        if let Some(npc) = combat_engine::NpcStats::from_value(raw) {
            if npc.condition_immunities.iter().any(|c| c.to_lowercase() == condition) {
                return Ok(true);
            }
            // Creature-type based immunities
            let creature_type = raw.get("creature_type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            if is_immune_by_type(&creature_type, condition) {
                return Ok(true);
            }
        }
    }

    // Character sheet condition_immunities (feats, class features)
    if let Some(ref sheet) = char_sheet {
        if let Some(arr) = sheet.get("condition_immunities").and_then(|v| v.as_array()) {
            if arr.iter().any(|c| c.as_str().map(|s| s.to_lowercase() == condition).unwrap_or(false)) {
                return Ok(true);
            }
        }
        let creature_type = sheet.get("creature_type").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
        if !creature_type.is_empty() && is_immune_by_type(&creature_type, condition) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_immune_by_type(creature_type: &str, condition: &str) -> bool {
    match condition {
        "poisoned" | "exhaustion" | "frightened" | "charmed" =>
            matches!(creature_type, "undead" | "construct" | "plant"),
        "paralyzed" | "petrified" =>
            creature_type == "construct",
        "blinded" | "deafened" =>
            creature_type == "plant",
        _ => false,
    }
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
            "update combatants set token_moved_round = null, reaction_used = false,
             legendary_actions_used = 0
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
            "update combatants set action_used = false, bonus_action_used = false, movement_used_ft = 0, action_spell_level = 0, bonus_action_spell_level = 0, last_hit_attack_total = null, last_hit_damage = null, last_hit_attacker = null, spell_being_cast = null where id = $1")
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

#[derive(Debug, Deserialize)]
struct AttackBody {
    target_id: Uuid,
    attack_expression: Option<String>,
    damage_expression: Option<String>,
    damage_type: String,
    damage_die: Option<String>,
    ability: Option<String>,
    proficient: Option<bool>,
    advantage: bool,
    disadvantage: bool,
    cover: Option<String>,
    is_spell_attack: bool,
    is_magical: bool,
    label: Option<String>,
    weapon_id: Option<String>,
    extra_damage_expression: Option<String>,
    extra_damage_type: Option<String>,
    power_attack: Option<bool>,
    skip_ammo: Option<bool>,
    reckless: Option<bool>,
}

/// Infer ammo name from weapon name (e.g. "Longbow" → "Arrow")
fn infer_ammo_type(weapon_name: &str) -> Option<&'static str> {
    let w = weapon_name.to_lowercase();
    if w.contains("bow") && !w.contains("crossbow") {
        Some("Arrow")
    } else if w.contains("crossbow") {
        Some("Bolt")
    } else if w.contains("musket") || w.contains("pistol") || w.contains("firearm") || w.contains("gun") || w.contains("rifle") {
        Some("Bullet")
    } else if w.contains("sling") {
        Some("Sling Bullet")
    } else if w.contains("blowgun") {
        Some("Needle")
    } else {
        None
    }
}

/// Decrement ammunition in character sheet equipment. Returns (ammo_name, remaining_qty) or None.
async fn decrement_ammo(
    db: &mut sqlx::PgConnection,
    character_id: Uuid,
    weapon_name: &str,
) -> Result<Option<(String, i32)>, crate::error::AppError> {
    let ammo_type = match infer_ammo_type(weapon_name) {
        Some(a) => a,
        None => return Ok(None),
    };

    // Find matching ammo in equipment
    let sheet_json: Option<serde_json::Value> = sqlx::query_scalar(
        "select sheet from characters where id = $1")
        .bind(character_id).fetch_optional(&mut *db).await?;

    let mut sheet = sheet_json.unwrap_or_else(|| serde_json::json!({}));
    let equipment = match sheet.get_mut("equipment").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => {
            return Err(crate::error::AppError::BadRequest(
                format!("No {} ammunition remaining for {}", ammo_type, weapon_name)
            ));
        }
    };

    // Find ammo item by fuzzy name match
    let mut found = false;
    let mut remaining = 0;
    for item in equipment.iter_mut() {
        if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
            if name.to_lowercase().contains(&ammo_type.to_lowercase()) {
                let qty = item.get("qty").and_then(|v| v.as_i64()).unwrap_or(0);
                if qty > 0 {
                    let new_qty = qty - 1;
                    item["qty"] = serde_json::json!(new_qty);
                    remaining = new_qty as i32;
                    found = true;
                    break;
                }
            }
        }
    }

    if !found {
        return Err(crate::error::AppError::BadRequest(
            format!("No {} ammunition remaining for {}", ammo_type, weapon_name)
        ));
    }

    sqlx::query("update characters set sheet = $1 where id = $2")
        .bind(&sheet)
        .bind(character_id)
        .execute(db).await?;

    Ok(Some((ammo_type.to_string(), remaining)))
}

async fn attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("attacker and target not in same encounter".into()));
    }

    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    // Authorize: master can use anyone; players only their own character
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let target_stats = combat_engine::compute_stats(&target_snap);

    let mut adv = body.advantage;
    let mut dis = body.disadvantage;

    // Reckless Attack: attacker gains advantage, but counter-effect gives enemies advantage
    let is_reckless = body.reckless.unwrap_or(false);
    if is_reckless {
        adv = true;
    }

    // Look up weapon for property checks
    let weapon = body.weapon_id.as_deref().and_then(|wid| combat_engine::find_weapon(&attacker_snap, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();

    // Ranged attack within 5ft of another combatant = disadvantage
    if weapon_props.ranged || weapon_props.thrown {
        let others: Vec<(Option<f32>, Option<f32>)> = sqlx::query_as(
            "select token_x, token_y from combatants where encounter_id = $1 and id != $2 and initiative_rolled = true")
            .bind(attacker_snap.encounter_id).bind(id).fetch_all(&s.db).await?;
        if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
            let within_5ft = others.iter().any(|(ox, oy)| {
                if let (Some(x), Some(y)) = (ox, oy) {
                    let dx = x - ax;
                    let dy = y - ay;
                    (dx*dx + dy*dy).sqrt() < 1.5 // ~5ft in map percent
                } else { false }
            });
            if within_5ft {
                dis = true;
            }
        }
    }

    // Visibility check: attacker in magical darkness / low visibility without darkvision
    let overlays: Vec<(String, Option<f32>, Option<f32>, Option<i32>, Option<i32>)> = sqlx::query_as(
        "select zone_type, origin_x, origin_y, radius_ft, length_ft from encounter_overlays
         where active = true and encounter_id = $1 and zone_type in ('magical_darkness', 'low_visibility', 'no_visibility')")
        .bind(attacker_snap.encounter_id).fetch_all(&s.db).await?;
    if let (Some(ax), Some(ay)) = (attacker_snap.token_x, attacker_snap.token_y) {
        let in_darkness = overlays.iter().any(|(zt, ox, oy, rad, _len)| {
            if let (Some(x), Some(y)) = (ox, oy) {
                let dx = ax - x;
                let dy = ay - y;
                let in_zone = if let Some(r) = rad {
                    // Approximate: 5ft = 1 grid cell. Map is ~100% = encounter map size.
                    // Use a rough conversion: 30ft radius ≈ 30% of map
                    (dx*dx + dy*dy).sqrt() < (*r as f32)
                } else {
                    // Default small zone
                    (dx*dx + dy*dy).sqrt() < 5.0
                };
                in_zone && (zt == "magical_darkness" || zt == "no_visibility" || (zt == "low_visibility" && attacker_stats.darkvision_range == 0))
            } else { false }
        });
        if in_darkness {
            dis = true;
        }
    }

    let req = combat_engine::AttackReq {
        target_id: body.target_id,
        attack_expression: body.attack_expression,
        damage_expression: body.damage_expression,
        damage_type: body.damage_type,
        damage_die: body.damage_die,
        ability: body.ability,
        proficient: body.proficient,
        advantage: adv,
        disadvantage: dis,
        cover: body.cover,
        is_spell_attack: body.is_spell_attack,
        is_magical: body.is_magical,
        label: body.label,
        weapon_id: body.weapon_id,
        extra_damage_expression: body.extra_damage_expression,
        extra_damage_type: body.extra_damage_type,
        power_attack: body.power_attack.unwrap_or(false),
        reckless: is_reckless,
    };

    let result = combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    // Decrement ammunition if weapon uses ammo (check before committing attack)
    let mut tx = s.db.begin().await?;

    // Atomic action consumption first
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    // Decrement ammunition if weapon uses ammo (inside transaction), unless skip_ammo is set
    let ammo_info: Option<(String, i32)> = if body.skip_ammo.unwrap_or(false) {
        None
    } else if let Some((w, _)) = &weapon {
        let wname = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("");
        if props.to_lowercase().contains("ammunition") || props.to_lowercase().contains("ammo") {
            if let Some(chid) = attacker_snap.character_id {
                decrement_ammo(&mut *tx, chid, wname).await?
            } else { None }
        } else { None }
    } else { None };

    // Apply damage to DB if hit
    if result.hit {
        // Record last hit on target for Shield reaction window
        sqlx::query(
            "update combatants set last_hit_attack_total = $1, last_hit_damage = $2, last_hit_attacker = $3 where id = $4")
            .bind(result.attack_total)
            .bind(result.damage_applied + result.extra_damage_applied)
            .bind(id)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        // Update target HP
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        // Break concentration if needed
        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx).await?;
        }

        // Massive damage: instant death
        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                let _ = sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#)
                    .bind(chid).execute(&mut *tx).await;
            }
        }

    }

    // Apply Reckless Attack counter-effect (enemies have advantage against attacker)
    if is_reckless {
        sqlx::query(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, active, modifiers, source_type)
               values ($1, 'Reckless Attack', 'debuff', 'swords', 'rounds', 1, 1, 'caster_turn_start',
                       false, true, '{"attack_advantage_against": true}', 'ability')"#)
            .bind(id)
            .execute(&mut *tx).await?;
    }

    // Reveal hidden attacker regardless of hit/miss (PHB: attacking ends hidden status)
    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'")
        .bind(id).execute(&mut *tx).await?;

    // Log combat event
    let total_dmg = result.damage_applied + result.extra_damage_applied;
    let event_action = if result.hit {
        let death_note = if result.instant_death { " — INSTANT DEATH" } else { "" };
        format!("{} attacked {}: {} damage{}", attacker_snap.display_name, target_snap.display_name, total_dmg, death_note)
    } else {
        format!("{} attacked {}: missed ({} vs AC {})", attacker_snap.display_name, target_snap.display_name, result.attack_total, result.target_ac)
    };
    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(body.target_id)
        .bind(&event_action)
        .bind(if result.hit { -total_dmg } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await;

    tx.commit().await?;

    if result.hit {
        let _ = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await;
        // Notify target they can react with Shield (reaction window)
        let total_dmg = result.damage_applied + result.extra_damage_applied;
        ws::publish(campaign_id, json!({
            "type": "reaction_window",
            "window_type": "hit_before_damage",
            "target_id": body.target_id,
            "attacker_id": id,
            "attack_total": result.attack_total,
            "target_ac": result.target_ac,
            "damage_pending": total_dmg,
        }).to_string());

        // Auto-trigger ready actions for combatants watching for "target_attacks" trigger
        auto_trigger_ready_actions_for_event(&s.db, campaign_id, attacker_snap.encounter_id,
            "target_attacks", id, body.target_id).await;
    }

    // Broadcast
    ws::publish(campaign_id, json!({
        "type": "combatant_attacked",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "extra_damage": if result.hit && result.extra_damage_applied > 0 { Some(result.extra_damage_applied) } else { None },
        "extra_damage_type": result.extra_damage_type.as_deref(),
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_broken": if result.hit { Some(result.concentration_broken) } else { None },
        "instant_death": if result.hit { Some(result.instant_death) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
        "ammo_consumed": ammo_info.as_ref().map(|(n, q)| serde_json::json!({"type": n, "remaining": q})),
    }).to_string());

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
struct DamageBody {
    amount: i32,
    damage_type: String,
    source_combatant_id: Option<Uuid>,
    label: Option<String>,
    is_magical: bool,
}

async fn deal_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DamageBody>,
) -> AppResult<Json<combat_engine::DamageResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        // Players can only damage their own character, or if source is their character
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        let source_owner: Option<Uuid> = if let Some(sid) = body.source_combatant_id {
            sqlx::query_scalar(
                "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
                .bind(sid).fetch_optional(&s.db).await?
        } else { None };
        if owner != Some(uid) && source_owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;

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
        .execute(&mut *tx).await?;

    if result.concentration_broken {
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(id)
            .execute(&mut *tx).await?;
    }

    // Massive damage: instant death — write to character sheet
    if result.instant_death {
        if let Some(chid) = target_snap.character_id {
            let _ = sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', false,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                   where id = $1"#)
                .bind(chid).execute(&mut *tx).await;
        }
    }

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid).fetch_optional(&s.db).await?.unwrap_or_else(|| "Unknown".into())
    } else { "DM".into() };

    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!("{} dealt {} {} damage to {}{}", source_name, result.damage_applied, req.damage_type, target_snap.display_name, if result.instant_death { " — INSTANT DEATH" } else { "" }))
        .bind(-result.damage_applied)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await;

    tx.commit().await?;

    let _ = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await;

    ws::publish(campaign_id, json!({
        "type": "combatant_damaged",
        "target_id": id,
        "damage": result.damage_applied,
        "hp_after": result.hp_after,
        "temp_hp_after": result.temp_hp_after,
        "concentration_broken": result.concentration_broken,
        "instant_death": result.instant_death,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Heal
// =====================================================================

#[derive(Debug, Deserialize)]
struct HealBody {
    amount: i32,
    source_combatant_id: Option<Uuid>,
    label: Option<String>,
}

async fn heal(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<HealBody>,
) -> AppResult<Json<combat_engine::HealResult>> {
    let target_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

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
        .execute(&mut *tx).await?;

    // PHB p.197: healing a dying creature (0 HP) resets death saves
    if reviving_from_zero {
        if let Some(chid) = target_snap.character_id {
            let _ = sqlx::query(
                r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                   || jsonb_build_object('alive', true,
                        'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                   where id = $1"#)
                .bind(chid).execute(&mut *tx).await;
        }
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(target_snap.encounter_id).fetch_one(&s.db).await?;

    let source_name = if let Some(sid) = body.source_combatant_id {
        sqlx::query_scalar::<_, String>("select display_name from combatants where id = $1")
            .bind(sid).fetch_optional(&s.db).await?.unwrap_or_else(|| "Unknown".into())
    } else { "DM".into() };

    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(target_snap.encounter_id)
        .bind(round)
        .bind(body.source_combatant_id)
        .bind(id)
        .bind(format!("{} healed {} for {} HP", source_name, target_snap.display_name, result.amount))
        .bind(result.amount)
        .bind(req.label.as_deref())
        .execute(&mut *tx).await;

    tx.commit().await?;

    let _ = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, result.temp_hp_after).await;

    ws::publish(campaign_id, json!({
        "type": "combatant_healed",
        "target_id": id,
        "amount": result.amount,
        "hp_after": result.hp_after,
        "stabilized": result.stabilized,
        "revived": reviving_from_zero,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Death Save
// =====================================================================

#[derive(Debug, Deserialize)]
struct DeathSaveBody {
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
}

async fn death_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<DeathSaveBody>,
) -> AppResult<Json<combat_engine::DeathSaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Only allow death saves at 0 HP
    if snap.hp_current > 0 {
        return Err(AppError::BadRequest("character is not dying".into()));
    }

    let req = combat_engine::DeathSaveReq {
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result = combat_engine::resolve_death_save(&snap, &req)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;

    // Update combatant HP
    sqlx::query("update combatants set hp_current = $1 where id = $2")
        .bind(result.hp_after)
        .bind(id)
        .execute(&mut *tx).await?;

    // Update character sheet death_saves + alive
    if let Some(chid) = snap.character_id {
        let _ = sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'death_saves', jsonb_build_object('successes', $2::int, 'failures', $3::int),
                      'alive', $4::bool
                    )
               where id = $1"#,
        )
        .bind(chid)
        .bind(result.successes_after)
        .bind(result.failures_after)
        .bind(result.alive)
        .execute(&mut *tx).await;
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;

    let action_str = if result.nat20 {
        "Death Save: NAT 20 — regains 1 HP".to_string()
    } else if result.nat1 {
        format!("Death Save: NAT 1 — {} failures", result.failures_after)
    } else if result.passed {
        format!("Death Save: success ({}/3)", result.successes_after)
    } else {
        format!("Death Save: failure ({}/3)", result.failures_after)
    };

    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(id)
        .bind(&action_str)
        .bind(if result.hp_after > 0 { result.hp_after } else { 0 })
        .bind(req.label.as_deref())
        .execute(&mut *tx).await;

    tx.commit().await?;

    let _ = sync_combatant_hp_to_sheet(&s.db, id, result.hp_after, snap.temp_hp).await;

    ws::publish(campaign_id, json!({
        "type": "combatant_death_save",
        "combatant_id": id,
        "natural_roll": result.natural_roll,
        "passed": result.passed,
        "successes": result.successes_after,
        "failures": result.failures_after,
        "stabilized": result.stabilized,
        "died": result.died,
        "hp_after": result.hp_after,
        "alive": result.alive,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Skill Check
// =====================================================================

#[derive(Debug, Deserialize)]
struct SkillCheckBody {
    skill: String,
    dc: Option<i32>,
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
}

async fn skill_check(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkillCheckBody>,
) -> AppResult<Json<combat_engine::SkillCheckResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let stats = combat_engine::compute_stats(&snap);
    let req = combat_engine::SkillCheckReq {
        skill: body.skill,
        dc: body.dc,
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
    };
    let result = combat_engine::resolve_skill_check(&snap, &req, &stats)
        .map_err(|e| AppError::BadRequest(e))?;

    ws::publish(campaign_id, json!({
        "type": "combatant_skill_check",
        "combatant_id": id,
        "skill": result.skill,
        "total": result.total,
        "dc": result.dc,
        "passed": result.passed,
    }).to_string());

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
struct SaveBody {
    ability: String,
    dc: i32,
    advantage: bool,
    disadvantage: bool,
    label: Option<String>,
    is_magical: Option<bool>,
}

async fn roll_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<combat_engine::SaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    let stats = combat_engine::compute_stats(&snap);
    let req = combat_engine::SaveReq {
        ability: body.ability,
        dc: body.dc,
        advantage: body.advantage,
        disadvantage: body.disadvantage,
        label: body.label,
        is_magical: body.is_magical,
    };
    let result = combat_engine::resolve_save(&snap, &req, &stats)
        .map_err(|e| AppError::BadRequest(e))?;

    ws::publish(campaign_id, json!({
        "type": "combatant_save",
        "combatant_id": id,
        "passed": result.passed,
        "save_total": result.save_total,
        "dc": result.dc,
        "natural_roll": result.natural_roll,
    }).to_string());

    Ok(Json(result))
}

async fn computed_stats(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<combat_engine::ComputedStats>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let stats = combat_engine::compute_stats(&snap);
    Ok(Json(stats))
}

async fn sync_combatant_hp_to_sheet(db: &sqlx::PgPool, combatant_id: Uuid, hp: i32, temp: i32) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac from combatants where id = $1 and ref_type = 'character'")
        .bind(combatant_id).fetch_optional(db).await?;
    if let Some((chid, hp_max, ac)) = row {
        let alive = hp > 0;
        let _ = sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max).bind(temp).bind(ac).bind(alive)
        .execute(db).await;
    }
    Ok(())
}

async fn sync_combatant_hp_to_sheet_tx(conn: &mut sqlx::PgConnection, combatant_id: Uuid, hp: i32, temp: i32) -> AppResult<()> {
    let row: Option<(Uuid, i32, i32)> = sqlx::query_as(
        "select character_id, hp_max, ac from combatants where id = $1 and ref_type = 'character'")
        .bind(combatant_id).fetch_optional(&mut *conn).await?;
    if let Some((chid, hp_max, ac)) = row {
        let alive = hp > 0;
        let _ = sqlx::query(
            r#"update characters set sheet =
                 coalesce(sheet, '{}'::jsonb)
                 || jsonb_build_object(
                      'hp', coalesce(sheet->'hp', '{}'::jsonb)
                            || jsonb_build_object('current', $2::int, 'max', $3::int, 'temp', $4::int),
                      'ac', $5::int,
                      'alive', $6::bool,
                      'death_saves', case when $6::bool and coalesce((sheet->>'alive')::bool, true) = false
                                       then jsonb_build_object('successes', 0, 'failures', 0)
                                       else coalesce(sheet->'death_saves', jsonb_build_object('successes', 0, 'failures', 0))
                                     end
                    )
               where id = $1"#,
        )
        .bind(chid).bind(hp).bind(hp_max).bind(temp).bind(ac).bind(alive)
        .execute(&mut *conn).await;
    }
    Ok(())
}


#[derive(Debug, Deserialize)]
struct ReactBody {
    pub reaction_type: String, // shield | counterspell | opportunity_attack | custom
    pub label: Option<String>,
}

async fn react(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReactBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, bool, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, c.reaction_used, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, _reaction_used, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    let mut tx = s.db.begin().await?;

    // Atomic reaction consumption
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set reaction_used = true where id = $1 and reaction_used = false
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .fetch_optional(&mut *tx).await?
    .ok_or(AppError::BadRequest("reaction already used this round".into()))?;

    // Apply reaction-specific effects
    let mut shield_blocked_hit = false;
    match body.reaction_type.as_str() {
        "shield" => {
            // PHB: Shield reaction only valid when hit (has last_hit_attack_total this round).
            // Check trigger context.
            let last_hit: Option<(Option<i32>, Option<i32>, Option<Uuid>)> = sqlx::query_as(
                "select last_hit_attack_total, last_hit_damage, last_hit_attacker from combatants where id = $1")
                .bind(id).fetch_optional(&mut *tx).await?;

            let (atk_total, pending_dmg, _attacker) = last_hit.unwrap_or((None, None, None));

            if atk_total.is_none() {
                return Err(AppError::BadRequest(
                    "Shield can only be used when you have been hit (no pending hit this round)".into()
                ));
            }

            // Shield adds +5 AC. Check if the hit now misses.
            let snap = combat_engine::load_snapshot(&s.db, id).await?;
            let stats = combat_engine::compute_stats(&snap);
            let ac_with_shield = stats.ac + 5;
            let attack_total = atk_total.unwrap();

            // Apply the +5 AC effect for the rest of the round
            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, active, modifiers, source_type)
                   values ($1, 'Shield (Reaction)', 'buff', 'rounds', 1, 1, 'caster_turn_start',
                           false, true, '{"ac_bonus": 5}', 'spell')"#,
            )
            .bind(id).execute(&mut *tx).await?;

            if attack_total < ac_with_shield {
                // Hit is retroactively negated — reverse the damage
                let dmg_to_restore = pending_dmg.unwrap_or(0);
                let current_hp: (i32, i32) = sqlx::query_as(
                    "select hp_current, temp_hp from combatants where id = $1")
                    .bind(id).fetch_one(&mut *tx).await?;
                let new_hp = (current_hp.0 + dmg_to_restore).min(snap.hp_max);
                sqlx::query("update combatants set hp_current = $1, last_hit_attack_total = null, last_hit_damage = null where id = $2")
                    .bind(new_hp).bind(id).execute(&mut *tx).await?;
                shield_blocked_hit = true;
            } else {
                // Hit still lands even with +5 AC; just clear the pending hit
                sqlx::query("update combatants set last_hit_attack_total = null, last_hit_damage = null where id = $1")
                    .bind(id).execute(&mut *tx).await?;
            }
        }
        "counterspell" => {
            // Counterspell only valid when a spell is actively being cast.
            // Check spell_being_cast on the combatants in the encounter.
            let active_cast: Option<(Uuid, String)> = sqlx::query_as(
                r#"select id, spell_being_cast from combatants
                   where encounter_id = $1 and spell_being_cast is not null
                   limit 1"#)
                .bind(encounter_id).fetch_optional(&mut *tx).await?;

            if active_cast.is_none() {
                return Err(AppError::BadRequest(
                    "Counterspell can only be used when a spell is being cast".into()
                ));
            }
            // Clear the spell_being_cast flag — spell is countered
            if let Some((caster_id, _slug)) = active_cast {
                sqlx::query("update combatants set spell_being_cast = null where id = $1")
                    .bind(caster_id).execute(&mut *tx).await?;
            }
        }
        _ => {}
    }

    tx.commit().await?;

    let label = body.label.unwrap_or_else(|| body.reaction_type.clone());
    ws::publish(campaign_id, json!({
        "type": "combatant_reacted",
        "combatant_id": id,
        "reaction_type": body.reaction_type,
        "label": label,
        "shield_blocked_hit": shield_blocked_hit,
    }).to_string());

    emit_campaign(&s.db, campaign_id, None,
        "combat.reaction",
        &format!("{} used reaction: {}", c.display_name, label),
        None, Some("encounter"), Some(encounter_id)).await;

    Ok(Json(c))
}


#[derive(Debug, Deserialize)]
struct CastSpellBody {
    pub spell_slug: String,
    pub target_ids: Vec<Uuid>,
    pub upcast_level: Option<i32>,
    pub damage_expression: Option<String>,
    pub save_dc: Option<i32>,
    pub spell_attack_bonus: Option<i32>,
    pub half_on_save: bool,
    pub save_ability: Option<String>,
    pub cast_as_ritual: Option<bool>,
    pub use_spell_attack: Option<bool>,
}

#[derive(Debug, Serialize)]
struct CastSpellTargetResult {
    pub target_id: Uuid,
    pub target_name: String,
    pub hit: Option<bool>,
    pub critical: bool,
    pub attack_total: Option<i32>,
    pub save_passed: Option<bool>,
    pub save_total: Option<i32>,
    pub damage_applied: i32,
    pub hp_after: i32,
    pub temp_hp_after: i32,
    pub effects_applied: Vec<String>,
    pub concentration_broken: bool,
}

#[derive(Debug, Serialize)]
struct CastSpellResult {
    pub spell_name: String,
    pub spell_level: i32,
    pub caster_id: Uuid,
    pub slot_level_consumed: i32,
    pub targets: Vec<CastSpellTargetResult>,
    pub overlay_created: Option<Uuid>,
    pub concentration_required: bool,
}

async fn cast_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(caster_id): Path<Uuid>,
    Json(body): Json<CastSpellBody>,
) -> AppResult<Json<CastSpellResult>> {
    let caster_snap = combat_engine::load_snapshot(&s.db, caster_id).await?;
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;

    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(caster_id).fetch_optional(&s.db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }

    // Fetch spell
    let spell: (String, i32, bool, bool, serde_json::Value, serde_json::Value, Option<String>, Option<String>) = sqlx::query_as(
        "select name, level, concentration, ritual, effects, casting_time, range_text, components from spells where slug = $1")
        .bind(&body.spell_slug)
        .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (spell_name, spell_level, concentration_required, is_ritual_spell, effects_json, _casting_time, range_text, components_text) = spell;
    let cast_as_ritual = body.cast_as_ritual.unwrap_or(false);
    if cast_as_ritual && !is_ritual_spell {
        return Err(AppError::BadRequest("spell cannot be cast as a ritual".into()));
    }
    let slot_level = body.upcast_level.unwrap_or(spell_level);

    let casting_time_str = _casting_time.as_str().unwrap_or("1 action");
    let is_bonus_action = casting_time_str.to_lowercase().contains("bonus");

    // Spell component checks (PHB p.203)
    let comps = components_text.as_deref().unwrap_or("").to_uppercase();
    if comps.contains('V') {
        // Verbal component: blocked by Silence spell (silenced effect modifier)
        let is_silenced = caster_snap.active_effects.iter().any(|e| {
            e.modifiers.get("silenced").and_then(|v| v.as_bool()).unwrap_or(false)
        });
        if is_silenced {
            return Err(AppError::BadRequest("cannot cast: silenced (no verbal component)".into()));
        }
    }
    if comps.contains('S') {
        // Somatic component: blocked by a no_somatic effect (e.g. arms restrained without War Caster)
        let has_war_caster = caster_snap.sheet_raw.get("feats")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("war_caster")))
            .unwrap_or(false);
        if !has_war_caster {
            let no_somatic = caster_snap.active_effects.iter().any(|e| {
                e.modifiers.get("no_somatic").and_then(|v| v.as_bool()).unwrap_or(false)
            });
            if no_somatic {
                return Err(AppError::BadRequest("cannot cast: somatic component blocked".into()));
            }
        }
    }

    // Spell preparation enforcement (PHB p.201): prepared-list classes must have prepared = true.
    // Cantrips (level 0), NPCs, and Masters bypass this check.
    // Known-spell classes (Sorcerer, Bard, Warlock, Ranger, Rogue, Fighter) skip preparation.
    if spell_level > 0 && role != Role::Master {
        if let Some(chid) = caster_snap.character_id {
            let primary_class = caster_snap.classes.as_array()
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("name"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();

            let requires_preparation = matches!(primary_class.as_str(),
                "wizard" | "cleric" | "druid" | "paladin" | "artificer"
            );

            if requires_preparation {
                let prepared: Option<bool> = sqlx::query_scalar(
                    r#"select cs.prepared
                       from character_spells cs
                       join spells s on s.id = cs.spell_id
                       where cs.character_id = $1 and s.slug = $2"#)
                    .bind(chid).bind(&body.spell_slug)
                    .fetch_optional(&s.db).await?;

                match prepared {
                    None => return Err(AppError::BadRequest(
                        format!("'{}' is not in your spell list", spell_name)
                    )),
                    Some(false) => return Err(AppError::BadRequest(
                        format!("'{}' is not prepared", spell_name)
                    )),
                    Some(true) => {}
                }
            }
        }
    }

    // Cantrip damage scaling: multiply base die count by tier (PHB p.205)
    // 1d8 @ level 1-4 → 2d8 @ 5-10 → 3d8 @ 11-16 → 4d8 @ 17+
    let effective_damage_expression = if spell_level == 0 {
        body.damage_expression.as_deref().map(|expr| {
            let caster_level = caster_snap.level_total.max(1);
            let multiplier = match caster_level {
                1..=4 => 1,
                5..=10 => 2,
                11..=16 => 3,
                _ => 4,
            };
            if multiplier <= 1 { return expr.to_string(); }
            // Scale leading NdX pattern: "1d8+3" → "2d8+3" at level 5+
            let re_pat = expr;
            if let Some(d_pos) = re_pat.find('d').or_else(|| re_pat.find('D')) {
                let num_str = &re_pat[..d_pos];
                let base_n: i32 = num_str.parse().unwrap_or(1);
                let scaled_n = base_n * multiplier;
                format!("{}{}", scaled_n, &re_pat[d_pos..])
            } else {
                expr.to_string()
            }
        })
    } else {
        body.damage_expression.clone()
    };

    let caster_stats = combat_engine::compute_stats(&caster_snap);
    let save_dc = body.save_dc.unwrap_or(caster_stats.spell_save_dc);
    let _spell_atk = body.spell_attack_bonus.unwrap_or(caster_stats.spell_attack_bonus);

    // Parse spell effect templates
    let template_arr: Vec<serde_json::Value> = serde_json::from_value(effects_json)
        .unwrap_or_default();

    // Check for AoE in templates
    let aoe_template = template_arr.iter().find(|t| t.get("aoe").is_some());
    let mut overlay_id: Option<Uuid> = None;

    let mut results = Vec::new();
    let mut rng = rand::rngs::StdRng::from_os_rng();

    // Pre-compute all target results before opening the transaction (pure calculation, no writes).
    let (round, turn_index, map_grid_size): (i32, i32, i32) = sqlx::query_as(
        "select round, turn_index, map_grid_size from encounters where id = $1")
        .bind(caster_snap.encounter_id).fetch_one(&s.db).await?;

    // Parse spell range to feet (None = unlimited / no validation)
    let range_ft = range_text.as_deref().and_then(parse_spell_range_ft);

    for target_id in &body.target_ids {
        let target_snap = match combat_engine::load_snapshot(&s.db, *target_id).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        if target_snap.encounter_id != caster_snap.encounter_id {
            continue;
        }

        // Spell range validation: only when both tokens are placed and range is finite
        if let Some(max_ft) = range_ft {
            if let (Some(cx), Some(cy), Some(tx), Some(ty)) = (
                caster_snap.token_x, caster_snap.token_y,
                target_snap.token_x, target_snap.token_y,
            ) {
                // map coords are 0-100 (percent). Convert using map_grid_size (pixels per 5ft square).
                // Each 5ft = (5.0 / grid_cell_ft) percent of map; we approximate grid occupies 100%
                // over the encounter map. grid_size is px/grid_cell; 1 grid = 5ft.
                // Distance in map % → feet: dist_pct * (100 / map_grid_size) * 5
                // Simplified: 1% map ≈ 5ft when grid_size = 50 (default)
                let pct_per_5ft = 5.0_f32 / (map_grid_size as f32);
                let dx = (cx - tx) / pct_per_5ft;
                let dy = (cy - ty) / pct_per_5ft;
                let dist_ft = (dx * dx + dy * dy).sqrt() * 5.0;
                if dist_ft > max_ft as f32 + 2.5 { // +2.5ft tolerance for grid snapping
                    return Err(AppError::BadRequest(format!(
                        "target out of range ({:.0}ft, max {}ft)", dist_ft, max_ft
                    )));
                }
            }
        }

        let target_stats = combat_engine::compute_stats(&target_snap);

        let save_ability_str = body.save_ability.as_deref().unwrap_or("dex").to_lowercase();
        let use_attack_roll = body.use_spell_attack.unwrap_or(false);
        let spell_atk_bonus = body.spell_attack_bonus.unwrap_or(caster_stats.spell_attack_bonus);

        // Resolve hit: spell attack roll OR saving throw
        let (hit, crit, attack_total, save_passed, save_total) = if use_attack_roll {
            let adv = caster_stats.attack_advantage;
            let dis = caster_stats.attack_disadvantage;
            let atk_expr = if adv && !dis {
                format!("2d20kh1+{}", spell_atk_bonus)
            } else if dis && !adv {
                format!("2d20kl1+{}", spell_atk_bonus)
            } else {
                format!("1d20+{}", spell_atk_bonus)
            };
            let atk_roll = crate::dice::roll(&atk_expr, &mut rng)
                .map_err(|e| AppError::BadRequest(e.to_string()))?;
            let nat = atk_roll.terms.first().and_then(|t| t.rolls.first().copied()).unwrap_or(0);
            let crit_range = caster_snap.sheet_raw.get("crit_range")
                .and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(20);
            let critical = nat >= crit_range;
            let auto_miss = nat == 1;
            let hit = if critical { true } else if auto_miss { false } else { atk_roll.total >= target_stats.ac };
            (Some(hit), critical, Some(atk_roll.total), None, None)
        } else if effective_damage_expression.is_some() {
            let save_req = combat_engine::SaveReq {
                ability: save_ability_str.clone(),
                dc: save_dc,
                advantage: false,
                disadvantage: false,
                label: None,
                is_magical: Some(true),
            };
            let save_res = combat_engine::resolve_save(&target_snap, &save_req, &target_stats)
                .map_err(|e| AppError::BadRequest(e))?;
            (None, false, None, Some(save_res.passed), Some(save_res.save_total))
        } else {
            (None, false, None, None, None)
        };

        // If attack roll and missed, skip damage
        let attack_missed = use_attack_roll && hit == Some(false);

        let mut damage_applied = 0i32;
        if !attack_missed {
            if let Some(ref dmg_expr) = effective_damage_expression {
                let mut dmg_roll = crate::dice::roll(dmg_expr, &mut rng)
                    .map_err(|e| AppError::BadRequest(e.to_string()))?;

                // Crit on spell attack: double the dice
                if crit {
                    let crit_expr = combat_engine::crit_double_dice(dmg_expr);
                    dmg_roll = crate::dice::roll(&crit_expr, &mut rng)
                        .map_err(|e| AppError::BadRequest(e.to_string()))?;
                }

                let raw_dmg = dmg_roll.total;

                let dtype = template_arr.iter()
                    .find(|t| t.get("modifiers").and_then(|m| m.get("fire_damage")).is_some()).map(|_| "fire")
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("cold_damage")).is_some()).map(|_| "cold"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("lightning_damage")).is_some()).map(|_| "lightning"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("thunder_damage")).is_some()).map(|_| "thunder"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("acid_damage")).is_some()).map(|_| "acid"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("poison_damage")).is_some()).map(|_| "poison"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("necrotic_damage")).is_some()).map(|_| "necrotic"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("radiant_damage")).is_some()).map(|_| "radiant"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("psychic_damage")).is_some()).map(|_| "psychic"))
                    .or_else(|| template_arr.iter().find(|t| t.get("modifiers").and_then(|m| m.get("force_damage")).is_some()).map(|_| "force"))
                    .unwrap_or("force");

                let (eff_dmg, _, _, _) = combat_engine::apply_damage_type(raw_dmg, dtype, &target_stats, true);

                if body.half_on_save && save_passed == Some(true) {
                    if target_stats.evasion && save_ability_str == "dex" {
                        damage_applied = 0;
                    } else {
                        damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
                    }
                } else if save_passed == Some(false) || save_passed.is_none() {
                    damage_applied = eff_dmg;
                }
            }
        }

        let (new_hp, new_temp) = combat_engine::apply_hp_damage(target_snap.hp_current, target_snap.temp_hp, damage_applied);

        let mut conc_broken = false;
        if target_snap.active_effects.iter().any(|e| e.concentration) && damage_applied > 0 {
            let (broken, _) = combat_engine::concentration_check(&target_snap, damage_applied, &mut rng);
            conc_broken = broken;
        }

        results.push(CastSpellTargetResult {
            target_id: *target_id,
            target_name: target_snap.display_name.clone(),
            hit,
            critical: crit,
            attack_total,
            save_passed,
            save_total,
            damage_applied,
            hp_after: new_hp,
            temp_hp_after: new_temp,
            effects_applied: template_arr.iter()
                .filter(|t| t.get("aoe").is_none())
                .filter_map(|t| t.get("name").and_then(|v| v.as_str()).map(|s| s.to_string()))
                .collect(),
            concentration_broken: conc_broken,
        });
    }

    // Single transaction: action atomicity first, then slot check, then all writes.
    let mut tx = s.db.begin().await?;

    // PHB p.203 "Bonus Action Spells": if you cast a spell with your BA, your action can only be used
    // to cast a cantrip (spell_level == 0). And vice versa: if action was already used for a non-cantrip
    // spell, you can only cast a cantrip with your BA.
    let (prev_action_spell_level, prev_bonus_spell_level): (i16, i16) = sqlx::query_as(
        "select action_spell_level, bonus_action_spell_level from combatants where id = $1")
        .bind(caster_id).fetch_one(&mut *tx).await?;
    if is_bonus_action {
        // Casting with BA: if action was already used for a leveled spell, only cantrips allowed
        if prev_action_spell_level > 0 && spell_level > 0 {
            return Err(AppError::BadRequest(
                "you already used your action to cast a spell; bonus-action spell must be a cantrip (PHB p.203)".into()
            ));
        }
    } else {
        // Casting with action: if BA was already used for a leveled spell, only cantrips allowed
        if prev_bonus_spell_level > 0 && spell_level > 0 {
            return Err(AppError::BadRequest(
                "you already used your bonus action to cast a spell; action spell must be a cantrip (PHB p.203)".into()
            ));
        }
    }

    // Mark spell_being_cast so Counterspell can detect this window.
    // Cleared after all writes complete or on error.
    sqlx::query("update combatants set spell_being_cast = $1 where id = $2")
        .bind(&body.spell_slug).bind(caster_id).execute(&mut *tx).await?;

    // Publish reaction window for Counterspell
    ws::publish(campaign_id, json!({
        "type": "reaction_window",
        "window_type": "spell_being_cast",
        "caster_id": caster_id,
        "spell_slug": body.spell_slug,
        "spell_level": spell_level,
        "slot_level": slot_level,
    }).to_string());

    // Atomic action/bonus-action consumption — must be first so double-cast is impossible.
    let action_consumed: Option<Uuid> = if is_bonus_action {
        sqlx::query_scalar(
            "update combatants set bonus_action_used = true, bonus_action_spell_level = $2 where id = $1 and bonus_action_used = false returning id")
            .bind(caster_id).bind(spell_level as i16).fetch_optional(&mut *tx).await?
    } else {
        sqlx::query_scalar(
            "update combatants set action_used = true, action_spell_level = $2 where id = $1 and action_used = false returning id")
            .bind(caster_id).bind(spell_level as i16).fetch_optional(&mut *tx).await?
    };
    if action_consumed.is_none() {
        let msg = if is_bonus_action { "bonus action already used" } else { "action already used" };
        return Err(AppError::BadRequest(msg.into()));
    }

    // Consume spell slot (skip for ritual casting and cantrips).
    if !cast_as_ritual && slot_level > 0 {
        if let Some(chid) = caster_snap.character_id {
            let slot_key = format!("{}", slot_level);
            let slot_current: Option<i32> = sqlx::query_scalar(
                "select (sheet->'slots'->$1->>'current')::int from characters where id = $2")
                .bind(&slot_key).bind(chid).fetch_optional(&mut *tx).await?;
            if let Some(current) = slot_current {
                if current <= 0 {
                    return Err(AppError::BadRequest("no spell slots of that level remaining".into()));
                }
                sqlx::query(
                    "update characters set sheet = jsonb_set(sheet, array['slots', $1, 'current'], to_jsonb($2::int)) where id = $3")
                    .bind(&slot_key).bind(current - 1).bind(chid).execute(&mut *tx).await?;
            }
        }
    }

    // Break existing caster concentration if this spell requires it.
    if concentration_required {
        sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
            .bind(caster_id).execute(&mut *tx).await?;
    }

    // Apply computed results to each target.
    for result in &results {
        let target_id = result.target_id;

        for t in &template_arr {
            if t.get("aoe").is_some() { continue; }

            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("Effect").to_string();
            let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("neutral").to_string();
            let icon = t.get("icon").and_then(|v| v.as_str()).unwrap_or("circle-dot").to_string();
            let duration_unit = t.get("duration_unit").and_then(|v| v.as_str()).unwrap_or("rounds").to_string();
            let duration_value = t.get("duration_value").and_then(|v| v.as_i64()).map(|v| v as i32);
            let tick_trigger = t.get("tick_trigger").and_then(|v| v.as_str()).unwrap_or("round_end").to_string();
            let conc = t.get("concentration").and_then(|v| v.as_bool()).unwrap_or(false);
            let modifiers = t.get("modifiers").cloned().unwrap_or_else(|| json!({}));

            sqlx::query(
                r#"insert into combatant_effects
                   (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                    concentration, caster_combatant_id, source_type, source_name, source_spell_slug, modifiers,
                    applied_at_round, applied_at_turn_index)
                   values ($1, $2, $3::effect_kind, $4, $5::duration_unit, $6, $7, $8::tick_trigger,
                           $9, $10, 'spell', $11, $12, $13, $14, $15)"#,
            )
            .bind(target_id)
            .bind(&name)
            .bind(&kind)
            .bind(&icon)
            .bind(&duration_unit)
            .bind(duration_value)
            .bind(duration_value)
            .bind(&tick_trigger)
            .bind(conc)
            .bind(caster_id)
            .bind(&spell_name)
            .bind(&body.spell_slug)
            .bind(modifiers)
            .bind(round)
            .bind(turn_index)
            .execute(&mut *tx).await?;
        }

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.hp_after).bind(result.temp_hp_after).bind(target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(target_id).execute(&mut *tx).await?;
        }

        let _ = sync_combatant_hp_to_sheet_tx(&mut *tx, target_id, result.hp_after, result.temp_hp_after).await;
    }

    // Create AoE overlay if spell has one.
    if let Some(template) = aoe_template {
        if let Some(aoe) = template.get("aoe") {
            let shape = aoe.get("shape").and_then(|v| v.as_str()).unwrap_or("circle");
            let radius_ft = aoe.get("radius_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let length_ft = aoe.get("length_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let width_ft = aoe.get("width_ft").and_then(|v| v.as_i64()).map(|v| v as i32);
            let color = aoe.get("color").and_then(|v| v.as_str()).unwrap_or("rgba(255,0,0,0.25)");
            let aoe_duration = template.get("duration_value").and_then(|v| v.as_i64()).map(|v| v as i32).unwrap_or(1);

            let oid: Uuid = sqlx::query_scalar(
                r#"insert into encounter_overlays
                   (encounter_id, kind, shape, origin_x, origin_y, radius_ft, length_ft, width_ft, color, label,
                    expires_at_round, source_spell_slug, created_by_combatant_id)
                   values ($1, 'aoe', $2, 50, 50, $3, $4, $5, $6, $7, $8, $9, $10)
                   returning id"#,
            )
            .bind(caster_snap.encounter_id)
            .bind(shape)
            .bind(radius_ft)
            .bind(length_ft)
            .bind(width_ft)
            .bind(color)
            .bind(&spell_name)
            .bind(round + aoe_duration)
            .bind(&body.spell_slug)
            .bind(caster_id)
            .fetch_one(&mut *tx).await?;
            overlay_id = Some(oid);
        }
    }

    tx.commit().await?;

    // Clear spell_being_cast now that the spell has resolved
    let _ = sqlx::query("update combatants set spell_being_cast = null where id = $1")
        .bind(caster_id).execute(&s.db).await;

    // Auto-trigger ready actions watching for "target_casts"
    auto_trigger_ready_actions_for_event(&s.db, campaign_id, caster_snap.encounter_id,
        "target_casts", caster_id, caster_id).await;

    ws::publish(campaign_id, json!({
        "type": "combatant_spell_cast",
        "caster_id": caster_id,
        "spell_slug": body.spell_slug,
        "spell_name": spell_name,
        "targets": results.iter().map(|r| json!({
            "target_id": r.target_id,
            "damage": r.damage_applied,
            "hp_after": r.hp_after,
            "save_passed": r.save_passed,
            "concentration_broken": r.concentration_broken,
        })).collect::<Vec<_>>(),
    }).to_string());

    Ok(Json(CastSpellResult {
        spell_name,
        spell_level,
        caster_id,
        slot_level_consumed: slot_level,
        targets: results,
        overlay_created: overlay_id,
        concentration_required,
    }))
}


// =====================================================================
// Dodge / Disengage / Help Actions
// =====================================================================

#[derive(Debug, Deserialize)]
struct SpecialActionBody {
    pub _target_id: Option<Uuid>, // for Help action
}

async fn dodge(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(_encounter_id).fetch_one(&s.db).await?;

    // Remove existing dodge effect first
    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Dodge'")
        .bind(id).execute(&s.db).await?;

    // Apply dodge: attackers have disadvantage
    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dodge', 'buff', 'shield', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"attack_disadvantage_against": true, "dex_save_advantage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&s.db).await?;

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_dodged","id":id}).to_string());
    Ok(Json(c))
}

async fn disengage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(_encounter_id).fetch_one(&s.db).await?;

    sqlx::query("update combatant_effects set active = false where combatant_id = $1 and name = 'Disengage'")
        .bind(id).execute(&s.db).await?;

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Disengage', 'buff', 'wind', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"disengage": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&s.db).await?;

    // Atomic action/BA consumption
    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_disengaged","id":id}).to_string());
    Ok(Json(c))
}

async fn help_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SpecialActionBody>,
) -> AppResult<Json<Combatant>> {
    let target_id = body._target_id.ok_or(AppError::BadRequest("target_id required".into()))?;
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let (_round, _turn_index): (i32, i32) = sqlx::query_as(
        "select round, turn_index from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    // Apply "Helped by X" effect on the target: next attack against target gets advantage
    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Helped', 'buff', 'hand', 'rounds', 1, 1, 'target_turn_start',
                   false, true, '{"attack_advantage_against": true}', 'ability')"#,
    )
    .bind(target_id)
    .execute(&s.db).await?;

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_helped","helper_id":id,"target_id":target_id}).to_string());
    Ok(Json(c))
}

// =====================================================================
// Opportunity Attack
// =====================================================================

#[derive(Debug, Deserialize)]
struct OppAttackBody {
    pub target_id: Uuid,
}

async fn opportunity_attack(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<OppAttackBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
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

    // Verify attacker has reaction available
    if attacker_snap.active_effects.iter().any(|e| e.modifiers.get("reaction_used").is_some()) {
        // Actually, we need to check the combatant's reaction_used field
    }

    // Check attacker is not incapacitated
    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    if attacker_stats.incapacitated {
        return Err(AppError::BadRequest("attacker is incapacitated".into()));
    }

    // Check target doesn't have disengage
    let target_stats = combat_engine::compute_stats(&target_snap);
    // Note: disengage check would need to look for active disengage effect
    let has_disengage = target_snap.active_effects.iter().any(|e| {
        e.modifiers.as_object().map(|m| m.get("disengage").and_then(|v| v.as_bool()) == Some(true)).unwrap_or(false)
    });
    if has_disengage {
        return Err(AppError::BadRequest("target has disengaged".into()));
    }

    let req = combat_engine::AttackReq {
        target_id: body.target_id,
        attack_expression: None,
        damage_expression: None,
        damage_type: "slashing".to_string(),
        damage_die: None,
        ability: Some("str".to_string()),
        proficient: Some(true),
        advantage: false,
        disadvantage: false,
        cover: None,
        is_spell_attack: false,
        is_magical: false,
        label: Some("Opportunity Attack".to_string()),
        weapon_id: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        reckless: false,
    };

    let result = combat_engine::resolve_attack(&attacker_snap, &target_snap, &req, &attacker_stats, &target_stats)
        .map_err(|e| AppError::BadRequest(e))?;

    let mut tx = s.db.begin().await?;

    // Atomic reaction consumption
    let reaction_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set reaction_used = true where id = $1 and reaction_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if reaction_consumed.is_none() {
        return Err(AppError::BadRequest("reaction already used".into()));
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id).execute(&mut *tx).await?;
        }

        if result.instant_death {
            if let Some(chid) = target_snap.character_id {
                let _ = sqlx::query(
                    r#"update characters set sheet = coalesce(sheet,'{}'::jsonb)
                       || jsonb_build_object('alive', false,
                            'death_saves', jsonb_build_object('successes', 0, 'failures', 3))
                       where id = $1"#)
                    .bind(chid).execute(&mut *tx).await;
            }
        }
    }

    // Reveal hidden attacker on any attack (hit or miss)
    sqlx::query(
        "update combatant_effects set active = false
         where combatant_id = $1 and active = true
           and modifiers->>'hidden' = 'true'")
        .bind(id).execute(&mut *tx).await?;

    tx.commit().await?;

    if result.hit {
        let _ = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await;
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_opportunity_attack",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "damage": result.damage_applied,
        "instant_death": result.instant_death,
    }).to_string());

    Ok(Json(result))
}

async fn refresh_combatant(db: &sqlx::PgPool, id: Uuid) -> AppResult<Combatant> {
    sqlx::query_as::<_, Combatant>(
        r#"select id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                token_x, token_y, token_color, token_on_map, token_image,
                coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url,
                token_moved_round,
                action_used, bonus_action_used, reaction_used, movement_used_ft,
                legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast
         from combatants where id = $1"#,
    )
    .bind(id)
    .fetch_one(db)
    .await
    .map_err(|_| AppError::NotFound)
}

// =====================================================================
// Encounter Difficulty Calculator
// =====================================================================

#[derive(Debug, Serialize)]
struct DifficultyResult {
    pub total_xp: i32,
    pub adjusted_xp: i32,
    pub difficulty: String, // easy | medium | hard | deadly
    pub thresholds: DifficultyThresholds,
    pub party_levels: Vec<i32>,
    pub monster_xp: Vec<(String, i32, i32)>, // name, cr_xp, count
}

#[derive(Debug, Serialize)]
struct DifficultyThresholds {
    pub easy: i32,
    pub medium: i32,
    pub hard: i32,
    pub deadly: i32,
}

async fn encounter_difficulty(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<DifficultyResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    // Fetch party character levels from the dedicated column (not sheet JSONB)
    let party_levels: Vec<i32> = sqlx::query_scalar(
        r#"select ch.level_total
           from characters ch
           where ch.campaign_id = $1
             and coalesce((ch.sheet->>'alive')::boolean, true) = true"#,
    )
    .bind(campaign_id)
    .fetch_all(&s.db).await?;

    // Calculate thresholds per DMG p.82
    let mut easy = 0i32;
    let mut medium = 0i32;
    let mut hard = 0i32;
    let mut deadly = 0i32;

    for level in &party_levels {
        let (e, m, h, d) = xp_thresholds(*level);
        easy += e;
        medium += m;
        hard += h;
        deadly += d;
    }

    // Fetch combatants
    let combatants: Vec<(String, Option<serde_json::Value>)> = sqlx::query_as(
        r#"select c.display_name, n.stats as npc_stats
           from combatants c
           left join npcs n on n.id = c.npc_id
           where c.encounter_id = $1"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db).await?;

    let mut total_xp = 0i32;
    let mut monster_entries = Vec::new();

    for (name, stats) in &combatants {
        let xp = if let Some(s) = stats {
            // Try structured fields first, then legacy free-form fields
            s.get("xp").and_then(|v| v.as_i64()).map(|v| v as i32)
                .or_else(|| s.get("cr_xp").and_then(|v| v.as_i64()).map(|v| v as i32))
                .or_else(|| s.get("cr").and_then(|v| v.as_str()).and_then(cr_to_xp))
                .unwrap_or(0)
        } else {
            0
        };
        total_xp += xp;
        if xp > 0 {
            monster_entries.push((name.clone(), xp, 1));
        }
    }

    // Multiplier based on monster count vs party size
    let multiplier = if party_levels.is_empty() {
        1.0
    } else {
        let n_monsters = combatants.len().max(1);
        let n_party = party_levels.len();
        encounter_multiplier(n_monsters, n_party)
    };

    let adjusted_xp = (total_xp as f32 * multiplier).ceil() as i32;

    let difficulty = if adjusted_xp >= deadly {
        "deadly"
    } else if adjusted_xp >= hard {
        "hard"
    } else if adjusted_xp >= medium {
        "medium"
    } else {
        "easy"
    };

    Ok(Json(DifficultyResult {
        total_xp,
        adjusted_xp,
        difficulty: difficulty.to_string(),
        thresholds: DifficultyThresholds { easy, medium, hard, deadly },
        party_levels,
        monster_xp: monster_entries,
    }))
}

/// Auto-trigger ready actions in an encounter when a specific event fires.
/// `event_type`: "target_attacks" | "target_casts" | "target_enters_range"
/// `actor_id`: the combatant who performed the trigger action
/// `subject_id`: the combatant being watched (usually same as actor_id; for "target_attacks" it's the attacker)
async fn auto_trigger_ready_actions_for_event(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    encounter_id: Uuid,
    event_type: &str,
    actor_id: Uuid,
    subject_id: Uuid,
) {
    // Fetch all combatants with a readied action in this encounter
    let readied: Vec<(Uuid, serde_json::Value, bool)> = match sqlx::query_as(
        r#"select id, readied_action, reaction_used
           from combatants
           where encounter_id = $1 and readied_action is not null and reaction_used = false"#)
        .bind(encounter_id).fetch_all(db).await {
        Ok(rows) => rows,
        Err(_) => return,
    };

    for (cid, action_json, _) in readied {
        // Skip the actor themselves
        if cid == actor_id { continue; }

        let trigger_event = action_json.get("trigger_event")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let watch_target = action_json.get("watch_target_id")
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<Uuid>().ok());

        // Check if event type matches
        if trigger_event != event_type { continue; }

        // Check if they're watching a specific target (or watching anyone)
        if let Some(wid) = watch_target {
            if wid != subject_id { continue; }
        }

        // Trigger: consume reaction, clear readied_action, grant free action
        let ok = sqlx::query(
            "update combatants set reaction_used = true, readied_action = null, action_used = false
             where id = $1 and reaction_used = false")
            .bind(cid).execute(db).await.is_ok();

        if ok {
            ws::publish(campaign_id, json!({
                "type": "combatant_readied_triggered",
                "combatant_id": cid,
                "trigger_event": event_type,
                "triggered_by": actor_id,
                "readied_action": action_json,
            }).to_string());
        }
    }
}

fn xp_thresholds(level: i32) -> (i32, i32, i32, i32) {
    match level {
        1 => (25, 50, 75, 100),
        2 => (50, 100, 150, 200),
        3 => (75, 150, 225, 400),
        4 => (125, 250, 375, 500),
        5 => (250, 500, 750, 1100),
        6 => (300, 600, 900, 1400),
        7 => (350, 750, 1100, 1700),
        8 => (450, 900, 1400, 2100),
        9 => (550, 1100, 1600, 2400),
        10 => (600, 1200, 1900, 2800),
        11 => (800, 1600, 2400, 3600),
        12 => (1000, 2000, 3000, 4500),
        13 => (1100, 2200, 3400, 5100),
        14 => (1250, 2500, 3800, 5700),
        15 => (1400, 2800, 4300, 6400),
        16 => (1600, 3200, 4800, 7200),
        17 => (2000, 3900, 5900, 8800),
        18 => (2100, 4200, 6300, 9500),
        19 => (2400, 4900, 7300, 10900),
        20 => (2800, 5700, 8500, 12700),
        _ => (25, 50, 75, 100),
    }
}

fn cr_to_xp(cr: &str) -> Option<i32> {
    let c = cr.trim().to_lowercase();
    match c.as_str() {
        "0" | "1/8" => Some(10),
        "1/4" => Some(50),
        "1/2" => Some(100),
        "1" => Some(200),
        "2" => Some(450),
        "3" => Some(700),
        "4" => Some(1100),
        "5" => Some(1800),
        "6" => Some(2300),
        "7" => Some(2900),
        "8" => Some(3900),
        "9" => Some(5000),
        "10" => Some(5900),
        "11" => Some(7200),
        "12" => Some(8400),
        "13" => Some(10000),
        "14" => Some(11500),
        "15" => Some(13000),
        "16" => Some(15000),
        "17" => Some(18000),
        "18" => Some(20000),
        "19" => Some(22000),
        "20" => Some(25000),
        "21" => Some(33000),
        "22" => Some(41000),
        "23" => Some(50000),
        "24" => Some(62000),
        "25" => Some(75000),
        "26" => Some(90000),
        "27" => Some(105000),
        "28" => Some(120000),
        "29" => Some(135000),
        "30" => Some(155000),
        _ => None,
    }
}

fn encounter_multiplier(n_monsters: usize, n_party: usize) -> f32 {
    // DMG p. 82
    let base = match n_monsters {
        1 => 0.5,
        2 => 1.0,
        3..=6 => 1.5,
        7..=10 => 2.0,
        11..=14 => 2.5,
        _ => 3.0,
    };
    // Adjust for small parties (3 or fewer) or large parties (6 or more)
    if n_party <= 2 {
        base + 0.5
    } else if n_party >= 6 {
        (base - 0.5).max(0.5)
    } else {
        base
    }
}


// =====================================================================
// Ready Action
// =====================================================================

#[derive(Debug, Deserialize)]
struct ReadyBody {
    pub trigger: String, // e.g. "enemy moves within reach", "spell is cast"
    pub action: String,  // e.g. "attack", "cast spell", "dash"
    pub _target_id: Option<Uuid>,
    /// Automated trigger event: "target_attacks" | "target_casts" | "target_enters_range"
    pub trigger_event: Option<String>,
    /// Specific combatant to watch (None = watch anyone)
    pub watch_target_id: Option<Uuid>,
}

async fn ready_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ReadyBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    // Check action not already used
    let action_used: bool = sqlx::query_scalar("select action_used from combatants where id = $1")
        .bind(id).fetch_one(&s.db).await?;
    if action_used {
        return Err(AppError::BadRequest("action already used this turn".into()));
    }

    let readied = json!({
        "trigger": body.trigger,
        "action": body.action,
        "target_id": body._target_id,
        "trigger_event": body.trigger_event,
        "watch_target_id": body.watch_target_id,
    });

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set action_used = true, readied_action = $2
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .bind(readied)
    .fetch_one(&s.db).await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_readied",
        "id": id,
        "trigger": body.trigger,
        "action": body.action,
    }).to_string());

    Ok(Json(c))
}

// =====================================================================
// Delay Turn
// =====================================================================

#[derive(Debug, Deserialize)]
struct DelayBody {
    pub insert_after_turn_index: i32, // re-insert after this turn index
}

async fn delay_turn(
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
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, current_turn, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    let mut tx = s.db.begin().await?;

    // Set delayed_turn flag and mark action as used (delay consumes action)
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set delayed_turn = true, action_used = true, readied_action = null
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                    readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(id)
    .fetch_one(&mut *tx).await?;

    // Reorder: shift all combatants with turn_order > current_turn down by 1,
    // then place the delayed combatant after insert_after_turn_index
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
    .execute(&mut *tx).await?;

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": "combatant_delayed",
        "id": id,
        "insert_after": body.insert_after_turn_index,
    }).to_string());

    Ok(Json(c))
}

// =====================================================================
// Grapple
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

    let _attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let _defender_stats = combat_engine::compute_stats(&defender_snap);

    // Grapple is contested STR (Athletics) vs STR (Athletics) or DEX (Acrobatics)
    let str_mod_att = combat_engine::ability_mod(&attacker_snap, "str");
    let str_mod_def = combat_engine::ability_mod(&defender_snap, "str");
    let prof_att = combat_engine::proficiency_from_level(attacker_snap.level_total);
    let prof_def = combat_engine::proficiency_from_level(defender_snap.level_total);

    // Simple contested check: d20 + STR mod + proficiency (if proficient in Athletics)
    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = format!("1d20+{}", str_mod_att + prof_att);
    let def_expr = format!("1d20+{}", str_mod_def + prof_def);

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

    let str_mod_att = combat_engine::ability_mod(&attacker_snap, "str");
    let str_mod_def = combat_engine::ability_mod(&defender_snap, "str");
    let prof_att = combat_engine::proficiency_from_level(attacker_snap.level_total);
    let prof_def = combat_engine::proficiency_from_level(defender_snap.level_total);

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let att_expr = format!("1d20+{}", str_mod_att + prof_att);
    let def_expr = format!("1d20+{}", str_mod_def + prof_def);

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
    if body.targets.is_empty() {
        return Err(AppError::BadRequest("no targets specified".into()));
    }
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

    let attacker_stats = combat_engine::compute_stats(&attacker_snap);
    let mut results = Vec::new();
    let mut total_damage = 0i32;
    let mut targets_hit = 0usize;

    for t in &body.targets {
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
// AoE Overlay Auto-Damage
// =====================================================================

#[derive(Debug, Deserialize)]
struct OverlayDamageBody {
    pub overlay_id: Uuid,
    pub damage_expression: String,
    pub damage_type: String,
    pub save_ability: Option<String>,
    pub save_dc: Option<i32>,
    pub half_on_save: bool,
    pub is_magical: bool,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
struct OverlayDamageResult {
    pub overlay_id: Uuid,
    pub targets_affected: Vec<OverlayTargetResult>,
}

#[derive(Debug, Serialize)]
struct OverlayTargetResult {
    pub target_id: Uuid,
    pub target_name: String,
    pub in_area: bool,
    pub save_passed: Option<bool>,
    pub damage_applied: i32,
    pub hp_after: i32,
}

async fn overlay_damage(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<OverlayDamageBody>,
) -> AppResult<Json<OverlayDamageResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        return Err(AppError::Forbidden);
    }

    // Get overlay geometry
    let overlay: (String, Option<f64>, Option<f64>, Option<i32>, Option<i32>, Option<i32>) = sqlx::query_as(
        "select shape, origin_x, origin_y, radius_ft, length_ft, width_ft from encounter_overlays where id = $1 and encounter_id = $2 and active = true")
        .bind(body.overlay_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (shape, ox, oy, radius, _length, _width) = overlay;
    let origin = (ox.unwrap_or(50.0), oy.unwrap_or(50.0));

    // Get all combatants in encounter
    let combatants: Vec<(Uuid, String, Option<f64>, Option<f64>)> = sqlx::query_as(
        "select id, display_name, token_x, token_y from combatants where encounter_id = $1")
        .bind(encounter_id).fetch_all(&s.db).await?;

    let mut rng = rand::rngs::StdRng::from_os_rng();
    let mut targets_affected = Vec::new();

    for (cid, name, tx, ty) in &combatants {
        let in_area = if let (Some(x), Some(y)) = (tx, ty) {
            match shape.as_str() {
                "circle" => {
                    let r = radius.unwrap_or(20) as f64;
                    let dx = *x - origin.0;
                    let dy = *y - origin.1;
                    (dx*dx + dy*dy).sqrt() <= r
                }
                "cube" | "square" => {
                    let r = radius.unwrap_or(20) as f64;
                    let dx = (*x - origin.0).abs();
                    let dy = (*y - origin.1).abs();
                    dx <= r && dy <= r
                }
                _ => {
                    let dx = *x - origin.0;
                    let dy = *y - origin.1;
                    let r = radius.unwrap_or(20) as f64;
                    (dx*dx + dy*dy).sqrt() <= r
                }
            }
        } else { false };

        if !in_area { continue; }

        let snap = match combat_engine::load_snapshot(&s.db, *cid).await {
            Ok(s) => s,
            Err(_) => continue,
        };
        let stats = combat_engine::compute_stats(&snap);

        let mut save_passed = None;

        if let Some(ref ability) = body.save_ability {
            let dc = body.save_dc.unwrap_or(15);
            let save_req = combat_engine::SaveReq {
                ability: ability.clone(),
                dc,
                advantage: false,
                disadvantage: false,
                label: body.label.clone(),
                is_magical: Some(true),
            };
            if let Ok(res) = combat_engine::resolve_save(&snap, &save_req, &stats) {
                save_passed = Some(res.passed);
            }
        }

        let dmg_roll = crate::dice::roll(&body.damage_expression, &mut rng)
            .map_err(|e| AppError::BadRequest(e.to_string()))?;
        let raw_dmg = dmg_roll.total;

        let (eff_dmg, _, _, _) = combat_engine::apply_damage_type(raw_dmg, &body.damage_type, &stats, body.is_magical);

        let mut damage_applied = eff_dmg;
        if body.half_on_save && save_passed == Some(true) {
            damage_applied = (eff_dmg as f32 / 2.0).floor() as i32;
        } else if save_passed == Some(false) || save_passed.is_none() {
            damage_applied = eff_dmg;
        }

        let (new_hp, new_temp) = combat_engine::apply_hp_damage(snap.hp_current, snap.temp_hp, damage_applied);

        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(new_hp).bind(new_temp).bind(cid).execute(&s.db).await?;

        let _ = sync_combatant_hp_to_sheet(&s.db, *cid, new_hp, new_temp).await;

        targets_affected.push(OverlayTargetResult {
            target_id: *cid,
            target_name: name.clone(),
            in_area: true,
            save_passed,
            damage_applied,
            hp_after: new_hp,
        });
    }

    ws::publish(campaign_id, json!({
        "type": "overlay_damage",
        "overlay_id": body.overlay_id,
        "targets": targets_affected.iter().map(|t| json!({
            "target_id": t.target_id,
            "damage": t.damage_applied,
            "hp_after": t.hp_after,
            "save_passed": t.save_passed,
        })).collect::<Vec<_>>(),
    }).to_string());

    Ok(Json(OverlayDamageResult { overlay_id: body.overlay_id, targets_affected }))
}

// =====================================================================
// Surprise Round
// =====================================================================

#[derive(Debug, Deserialize)]
struct SurpriseBody {
    pub surprised_combatant_ids: Vec<Uuid>,
}

async fn surprise_round(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SurpriseBody>,
) -> AppResult<Json<Encounter>> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    if e.status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }
    if e.round != 1 {
        return Err(AppError::BadRequest("surprise can only be set on round 1".into()));
    }

    for cid in &body.surprised_combatant_ids {
        let conditions: Vec<String> = sqlx::query_scalar("select conditions from combatants where id = $1")
            .bind(cid).fetch_one(&s.db).await?;
        let mut new_conditions = conditions.clone();
        if !new_conditions.iter().any(|c| c.to_lowercase() == "surprised") {
            new_conditions.push("surprised".to_string());
        }
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&new_conditions).bind(cid).execute(&s.db).await?;
    }

    ws::publish(e.campaign_id, json!({
        "type": "surprise_round",
        "encounter_id": id,
        "surprised_ids": body.surprised_combatant_ids,
    }).to_string());

    Ok(Json(e))
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

#[derive(Debug, Deserialize)]
struct ActionBody {
    #[serde(default)]
    use_bonus_action: bool,
}

// =====================================================================
// Two-Weapon Fighting
// =====================================================================

#[derive(Debug, Deserialize)]
struct TwoWeaponFightBody {
    pub target_id: Uuid,
    pub offhand_weapon_id: String,
}

async fn two_weapon_fight(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<TwoWeaponFightBody>,
) -> AppResult<Json<combat_engine::AttackResult>> {
    let attacker_snap = combat_engine::load_snapshot(&s.db, id).await?;
    let target_snap = combat_engine::load_snapshot(&s.db, body.target_id).await?;

    if attacker_snap.encounter_id != target_snap.encounter_id {
        return Err(AppError::BadRequest("attacker and target not in same encounter".into()));
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
    let target_stats = combat_engine::compute_stats(&target_snap);

    // Check for Two-Weapon Fighting style in sheet features
    let twf_style = attacker_snap.sheet_raw.get("features")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().any(|f| {
            f.get("name").and_then(|v| v.as_str())
                .map(|n| n.to_lowercase().contains("two-weapon fighting"))
                .unwrap_or(false)
        }))
        .unwrap_or(false);

    let result = combat_engine::resolve_two_weapon_attack(
        &attacker_snap, &target_snap, &body.offhand_weapon_id, &attacker_stats, &target_stats, twf_style
    ).map_err(|e| AppError::BadRequest(e))?;

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(attacker_snap.encounter_id).fetch_one(&s.db).await?;

    let mut tx = s.db.begin().await?;

    // Consume BONUS action (not action)
    let bonus_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
        .bind(id).fetch_optional(&mut *tx).await?;
    if bonus_consumed.is_none() {
        return Err(AppError::BadRequest("bonus action already used".into()));
    }

    if result.hit {
        sqlx::query("update combatants set hp_current = $1, temp_hp = $2 where id = $3")
            .bind(result.target_hp_after)
            .bind(result.target_temp_hp_after)
            .bind(body.target_id)
            .execute(&mut *tx).await?;

        if result.concentration_broken {
            sqlx::query("update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true")
                .bind(body.target_id)
                .execute(&mut *tx).await?;
        }
    }

    let event_action = if result.hit {
        format!("{} TWF {}: {} damage", attacker_snap.display_name, target_snap.display_name, result.damage_applied)
    } else {
        format!("{} TWF {}: missed ({} vs AC {})", attacker_snap.display_name, target_snap.display_name, result.attack_total, result.target_ac)
    };
    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, delta_hp, note) values ($1, $2, $3, $4, $5, $6, $7)")
        .bind(attacker_snap.encounter_id)
        .bind(round)
        .bind(id)
        .bind(body.target_id)
        .bind(&event_action)
        .bind(if result.hit { -result.damage_applied } else { 0 })
        .execute(&mut *tx).await;

    tx.commit().await?;

    if result.hit {
        let _ = sync_combatant_hp_to_sheet(&s.db, body.target_id, result.target_hp_after, result.target_temp_hp_after).await;
    }

    ws::publish(campaign_id, json!({
        "type": "combatant_two_weapon_fought",
        "attacker_id": id,
        "target_id": body.target_id,
        "hit": result.hit,
        "critical": result.critical,
        "damage": if result.hit { Some(result.damage_applied) } else { None },
        "hp_after": if result.hit { Some(result.target_hp_after) } else { None },
        "temp_hp_after": if result.hit { Some(result.target_temp_hp_after) } else { None },
        "concentration_broken": if result.hit { Some(result.concentration_broken) } else { None },
        "attack_total": if !result.hit { Some(result.attack_total) } else { None },
        "target_ac": result.target_ac,
    }).to_string());

    Ok(Json(result))
}

// =====================================================================
// Flanking Detection
// =====================================================================

#[derive(Debug, Serialize)]
struct FlankResult {
    pub flanking_pairs: Vec<FlankPair>,
}

#[derive(Debug, Serialize)]
struct FlankPair {
    pub attacker_a_id: Uuid,
    pub attacker_a_name: String,
    pub attacker_b_id: Uuid,
    pub attacker_b_name: String,
    pub target_id: Uuid,
    pub target_name: String,
}

async fn check_flanking(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<FlankResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let tokens: Vec<(Uuid, String, f32, f32, String)> = sqlx::query_as(
        r#"select id, display_name, coalesce(token_x, 50) as x, coalesce(token_y, 50) as y,
           case when ref_type = 'character' then 'ally' else 'enemy' end as side
           from combatants
           where encounter_id = $1 and token_on_map = true and hp_current > 0"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db).await?;

    let mut pairs = Vec::new();
    let grid_size: i32 = sqlx::query_scalar("select map_grid_size from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    // Group by side
    let allies: Vec<_> = tokens.iter().filter(|t| t.4 == "ally").collect();
    let enemies: Vec<_> = tokens.iter().filter(|t| t.4 == "enemy").collect();

    // Check allies flanking enemies
    for target in &enemies {
        for i in 0..allies.len() {
            for j in (i+1)..allies.len() {
                let a = allies[i];
                let b = allies[j];
                if is_flanking(a.2, a.3, b.2, b.3, target.2, target.3, grid_size) {
                    pairs.push(FlankPair {
                        attacker_a_id: a.0,
                        attacker_a_name: a.1.clone(),
                        attacker_b_id: b.0,
                        attacker_b_name: b.1.clone(),
                        target_id: target.0,
                        target_name: target.1.clone(),
                    });
                }
            }
        }
    }

    // Check enemies flanking allies
    for target in &allies {
        for i in 0..enemies.len() {
            for j in (i+1)..enemies.len() {
                let a = enemies[i];
                let b = enemies[j];
                if is_flanking(a.2, a.3, b.2, b.3, target.2, target.3, grid_size) {
                    pairs.push(FlankPair {
                        attacker_a_id: a.0,
                        attacker_a_name: a.1.clone(),
                        attacker_b_id: b.0,
                        attacker_b_name: b.1.clone(),
                        target_id: target.0,
                        target_name: target.1.clone(),
                    });
                }
            }
        }
    }

    Ok(Json(FlankResult { flanking_pairs: pairs }))
}

fn is_flanking(ax: f32, ay: f32, bx: f32, by: f32, tx: f32, ty: f32, grid_size: i32) -> bool {
    // Map coords are percent (0–100). grid_size is pixels-per-cell.
    // Assume map renders at ~600px wide → 1% ≈ 6px → 1 cell ≈ grid_size/6 percent.
    // Use 2 cells as melee reach threshold.
    let px_per_pct = 6.0_f32;
    let cell_pct = (grid_size as f32) / px_per_pct;
    let max_dist = cell_pct * 2.0; // within ~2 cells = melee range

    let dx_a = ax - tx;
    let dy_a = ay - ty;
    let dx_b = bx - tx;
    let dy_b = by - ty;

    let dist_a = (dx_a * dx_a + dy_a * dy_a).sqrt();
    let dist_b = (dx_b * dx_b + dy_b * dy_b).sqrt();

    if dist_a > max_dist || dist_b > max_dist { return false; }
    if dist_a < 0.01 || dist_b < 0.01 { return false; } // overlapping tokens

    // Normalise vectors and check dot product (negative = opposite sides)
    let na = (dx_a / dist_a, dy_a / dist_a);
    let nb = (dx_b / dist_b, dy_b / dist_b);
    let dot = na.0 * nb.0 + na.1 * nb.1;

    dot <= -0.5 // vectors at least 120° apart → opposite sides
}

// =====================================================================
// Cover Calculation
// =====================================================================

#[derive(Debug, Serialize)]
struct CoverResult {
    pub attacker_id: Uuid,
    pub target_id: Uuid,
    pub cover_type: String, // none | half | three_quarters | full
    pub cover_bonus: i32,
    pub blockers: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct CoverQuery {
    pub attacker_id: Uuid,
    pub target_id: Uuid,
}

async fn calculate_cover(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Query(q): Query<CoverQuery>,
) -> AppResult<Json<CoverResult>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let attacker: (f32, f32) = sqlx::query_as(
        "select coalesce(token_x, 50), coalesce(token_y, 50) from combatants where id = $1 and encounter_id = $2")
        .bind(q.attacker_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let target: (f32, f32) = sqlx::query_as(
        "select coalesce(token_x, 50), coalesce(token_y, 50) from combatants where id = $1 and encounter_id = $2")
        .bind(q.target_id).bind(encounter_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let others: Vec<(String, f32, f32, String)> = sqlx::query_as(
        r#"select display_name, coalesce(token_x, 50) as x, coalesce(token_y, 50) as y, ref_type::text
           from combatants
           where encounter_id = $1 and id not in ($2, $3) and token_on_map = true and hp_current > 0"#,
    )
    .bind(encounter_id)
    .bind(q.attacker_id)
    .bind(q.target_id)
    .fetch_all(&s.db).await?;

    let mut blockers = Vec::new();
    let mut max_cover = 0i32; // 0=none, 1=half, 2=threeq, 3=full

    for (name, ox, oy, _ref_type) in &others {
        if is_between(*ox, *oy, attacker.0, attacker.1, target.0, target.1) {
            blockers.push(name.clone());
            max_cover = (max_cover + 1).min(3);
        }
    }

    let (cover_type, cover_bonus) = match max_cover {
        1 => ("half".to_string(), 2),
        2 => ("three_quarters".to_string(), 5),
        3 => ("full".to_string(), 999), // effectively can't be targeted
        _ => ("none".to_string(), 0),
    };

    Ok(Json(CoverResult {
        attacker_id: q.attacker_id,
        target_id: q.target_id,
        cover_type,
        cover_bonus,
        blockers,
    }))
}

// =====================================================================
// Dash / Hide / Search / Use an Object
// =====================================================================

async fn dash(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    // Atomic action/BA consumption
    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Apply Dash: grants extra movement equal to speed for this turn
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let stats = combat_engine::compute_stats(&snap);
    let extra = stats.speed.max(0);

    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Dash', 'buff', 'bolt', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, $2, 'ability')"#,
    )
    .bind(id)
    .bind(json!({"extra_movement": extra}))
    .execute(&s.db).await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_dashed","id":id,"extra_movement":extra}).to_string());
    Ok(Json(c))
}

async fn hide(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ActionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    // Atomic action/BA consumption
    if body.use_bonus_action {
        let ba_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set bonus_action_used = true where id = $1 and bonus_action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if ba_consumed.is_none() {
            return Err(AppError::BadRequest("bonus action already used".into()));
        }
    } else {
        let action_consumed: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true where id = $1 and action_used = false returning id")
            .bind(id).fetch_optional(&s.db).await?;
        if action_consumed.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    // Apply Hidden effect
    sqlx::query(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, active, modifiers, source_type)
           values ($1, 'Hidden', 'buff', 'eye-slash', 'rounds', 1, 1, 'caster_turn_start',
                   false, true, '{"hidden": true}', 'ability')"#,
    )
    .bind(id)
    .execute(&s.db).await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_hid","id":id}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
struct SearchBody {
    pub label: Option<String>,
}

async fn search_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SearchBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let label = body.label.unwrap_or_else(|| "Search".to_string());
    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, action, note) values ($1, $2, $3, $4, $5)")
        .bind(encounter_id).bind(round).bind(id).bind(&label).bind("search")
        .execute(&s.db).await;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_searched","id":id,"label":label}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
struct UseObjectBody {
    pub label: Option<String>,
    pub target_id: Option<Uuid>,
}

async fn use_object(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseObjectBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, encounter_id, status, owner) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }
    if status != "active" {
        return Err(AppError::BadRequest("encounter not active".into()));
    }

    // Atomic action consumption
    let action_consumed: Option<Uuid> = sqlx::query_scalar(
        "update combatants set action_used = true where id = $1 and action_used = false returning id")
        .bind(id).fetch_optional(&s.db).await?;
    if action_consumed.is_none() {
        return Err(AppError::BadRequest("action already used".into()));
    }

    let round: i32 = sqlx::query_scalar("select round from encounters where id = $1")
        .bind(encounter_id).fetch_one(&s.db).await?;

    let label = body.label.unwrap_or_else(|| "Use an Object".to_string());
    let _ = sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, note) values ($1, $2, $3, $4, $5, $6)")
        .bind(encounter_id).bind(round).bind(id).bind(body.target_id).bind(&label).bind("use_object")
        .execute(&s.db).await;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish(campaign_id, json!({"type":"combatant_used_object","id":id,"label":label}).to_string());
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
struct ConditionBody {
    pub condition: String,
    pub remove: Option<bool>,
    pub duration_rounds: Option<i32>,
}

async fn add_condition(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ConditionBody>,
) -> AppResult<Json<Combatant>> {
    let row: (Uuid, Uuid, String, Option<Uuid>, Vec<String>) = sqlx::query_as(
        r#"select e.campaign_id, e.id, e.status::text, ch.owner_id, c.conditions
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#,
    )
    .bind(id)
    .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let (campaign_id, _encounter_id, _status, owner, conditions) = row;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master && owner != Some(uid) {
        return Err(AppError::Forbidden);
    }

    let condition = body.condition.to_lowercase();
    let removing = body.remove == Some(true);

    // Condition immunity check (skip when removing)
    if !removing {
        let immune = check_condition_immunity(&s.db, id, &condition).await?;
        if immune {
            return Err(AppError::BadRequest(format!(
                "immune to {}", condition
            )));
        }
    }
    let new_conditions: Vec<String> = if removing {
        // Remove any entry whose name part (before ':') matches
        conditions.into_iter().filter(|c| {
            let name = c.split(':').next().unwrap_or(c).to_lowercase();
            name != condition
        }).collect()
    } else {
        let mut c = conditions;
        let already = c.iter().any(|existing| {
            existing.split(':').next().unwrap_or(existing).to_lowercase() == condition
        });
        if !already {
            let entry = if let Some(dur) = body.duration_rounds.filter(|&d| d > 0) {
                format!("{}:{}", condition, dur)
            } else {
                condition.clone()
            };
            c.push(entry);
        }
        c
    };

    let breaks_concentration = !removing && matches!(
        condition.as_str(),
        "incapacitated" | "paralyzed" | "stunned" | "unconscious"
    );
    // PHB: grapple ends if grappler becomes incapacitated
    let releases_grapple = !removing && matches!(
        condition.as_str(),
        "incapacitated" | "paralyzed" | "stunned" | "unconscious" | "dead"
    );

    let mut tx = s.db.begin().await?;

    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set conditions = $1 where id = $2
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url, token_moved_round,
                     action_used, bonus_action_used, reaction_used, movement_used_ft,
                     legendary_actions_max, legendary_actions_used, legendary_resistances_max, legendary_resistances_used,
                     readied_action, cover_bonus, delayed_turn, action_spell_level, bonus_action_spell_level, last_hit_attack_total, last_hit_damage, last_hit_attacker, spell_being_cast"#,
    )
    .bind(&new_conditions)
    .bind(id)
    .fetch_one(&mut *tx).await?;

    if breaks_concentration {
        sqlx::query(
            "update combatant_effects set active = false where combatant_id = $1 and concentration = true and active = true"
        )
        .bind(id)
        .execute(&mut *tx).await?;
    }

    if releases_grapple {
        // Remove grappling from this combatant
        let grappler_conds: Vec<String> = sqlx::query_scalar(
            "select conditions from combatants where id = $1")
            .bind(id).fetch_optional(&mut *tx).await?.unwrap_or_default();
        if has_condition(&grappler_conds, "grappling") {
            let freed = remove_condition(grappler_conds, "grappling");
            sqlx::query("update combatants set conditions = $1 where id = $2")
                .bind(&freed).bind(id).execute(&mut *tx).await?;
            // Release all combatants this grappler was holding (remove their grappled condition)
            let enc_combatants: Vec<(Uuid, Vec<String>)> = sqlx::query_as(
                "select id, conditions from combatants
                 where encounter_id = (select encounter_id from combatants where id = $1)
                   and id != $1")
                .bind(id).fetch_all(&mut *tx).await?;
            for (gid, gconds) in enc_combatants {
                if has_condition(&gconds, "grappled") {
                    let new_gconds = remove_condition(gconds, "grappled");
                    sqlx::query("update combatants set conditions = $1 where id = $2")
                        .bind(&new_gconds).bind(gid).execute(&mut *tx).await?;
                    ws::publish(campaign_id, json!({
                        "type": "combatant_condition_removed",
                        "combatant_id": gid,
                        "condition": "grappled",
                        "reason": "grappler incapacitated",
                    }).to_string());
                }
            }
        }
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({
        "type": if removing { "combatant_condition_removed" } else { "combatant_condition_added" },
        "combatant_id": id,
        "condition": body.condition,
    }).to_string());

    if breaks_concentration {
        ws::publish(campaign_id, json!({
            "type": "concentration_broken",
            "combatant_id": id,
            "reason": condition,
        }).to_string());
    }

    Ok(Json(c))
}

fn is_between(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> bool {
    // Check if point P is on the line segment AB, within a small tolerance
    let dx = bx - ax;
    let dy = by - ay;
    let len_sq = dx*dx + dy*dy;
    if len_sq < 0.0001 { return false; }

    // Project P onto AB
    let t = ((px - ax) * dx + (py - ay) * dy) / len_sq;
    if t < 0.1 || t > 0.9 { return false; } // not between A and B

    // Distance from P to line AB
    let dist = ((px - ax) * dy - (py - ay) * dx).abs() / len_sq.sqrt();
    dist < 3.0 // within ~3% of map (roughly a token radius)
}
