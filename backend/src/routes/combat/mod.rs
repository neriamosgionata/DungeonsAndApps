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
    extract::{Path, State},
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
use self::actions::*;
use self::combatants::{
    add_combatant, bulk_add_combatants, delete_combatant, list_combatants,
    move_combatant, update_combatant, use_action, Combatant,
};
use self::events::{
    list_events, delete_event, patch_effects,
};
use self::tactical::{
    add_condition, calculate_cover, check_flanking, create_overlay, delete_overlay,
    encounter_difficulty, is_between, is_flanking, list_overlays, overlay_damage,
    segments_intersect, surprise_auto, surprise_round,
};
use self::encounters::{EncounterCreate, EncounterUpdate};
use self::special::{
    class_feature, grapple, grapple_escape, lair_action, legendary_action, multiattack,
    parse_multiattack, shove, stand_up, trigger_ready,
};

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

// EncounterCreate and EncounterUpdate are in `encounters.rs`.

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
// Cunning Action / Action Use Body
// =====================================================================




