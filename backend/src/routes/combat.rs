use crate::{
    AppState,
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
    routing::{get, post},
};
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
        .route("/encounters/{id}/next-turn", post(next_turn))
        .route("/encounters/{id}/prev-turn", post(prev_turn))
        .route("/encounters/{id}/goto-turn", post(goto_turn))
        .route("/encounters/{id}/start", post(start))
        .route("/encounters/{id}/end", post(end_encounter))
        .route("/encounters/{id}/set-initiative", post(set_initiative))
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
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<Json<Vec<Encounter>>> {
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let rows: Vec<Encounter> = if role == Role::Master {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at
             from encounters where campaign_id = $1 order by updated_at desc")
            .bind(campaign_id).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Encounter>(
            "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at
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
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
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
                  coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 0),
                  coalesce((ch.sheet->'hp'->>'max')::int, 0),
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
        "select id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at
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
             show_grid      = coalesce($7, show_grid)
           where id = $1
           returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at"#)
        .bind(id)
        .bind(body.name)
        .bind(body.notes)
        .bind(body.map_image)
        .bind(clear_map)
        .bind(body.map_grid_size)
        .bind(body.show_grid)
        .fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_updated","id":id}).to_string());
    Ok(Json(e))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let e = fetch(&s, id).await?;
    rbac::require_master(&s.db, uid, e.campaign_id).await?;
    sqlx::query("delete from encounters where id = $1").bind(id).execute(&s.db).await?;
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
}

#[derive(Debug, Deserialize)]
pub struct CombatantMove {
    pub x: f32,
    pub y: f32,
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
                    coalesce(token_image, (select portrait_url from characters where id = character_id), (select image_key from npcs where id = npc_id)) as portrait_url
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
                    coalesce(c.token_image, ch.portrait_url, (select image_key from npcs where id = c.npc_id)) as portrait_url
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
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"insert into combatants
           (encounter_id, ref_type, character_id, npc_id, display_name, initiative, dex_tiebreaker, hp_current, hp_max, ac, is_visible, initiative_rolled)
           values ($1, $2::combatant_ref, $3, $4, $5, coalesce($6, 0), coalesce($7, 10),
                   coalesce($8, 0), coalesce($9, 0), coalesce($10, 10), coalesce($11, true), coalesce($12, true))
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url"#,
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
            && body.token_on_map.is_none();
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
             temp_hp        = coalesce($7, temp_hp),
             ac             = coalesce($8, ac),
             conditions     = coalesce($9, conditions),
             notes          = coalesce($10, notes),
             is_visible     = coalesce($11, is_visible),
             token_x        = coalesce($12, token_x),
             token_y        = coalesce($13, token_y),
             token_color    = coalesce($14, token_color),
             token_on_map   = coalesce($15, token_on_map),
             token_image    = case when $17 then null else coalesce($16, token_image) end
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url"#,
    )
    .bind(id)
    .bind(body.display_name).bind(body.initiative).bind(body.dex_tiebreaker)
    .bind(body.hp_current).bind(body.hp_max).bind(body.temp_hp).bind(body.ac)
    .bind(body.conditions).bind(body.notes).bind(body.is_visible)
    .bind(body.token_x).bind(body.token_y).bind(body.token_color).bind(body.token_on_map)
    .bind(body.token_image).bind(clear_token_image)
    .fetch_one(&s.db).await?;
    ws::publish(campaign_id, json!({"type":"combatant_updated","id":id}).to_string());

    // Sync combatant HP/AC back into linked character sheet so the sheet
    // shows the same values during combat. Only when fields actually changed.
    if c.ref_type == "character" {
        if let Some(chid) = c.character_id {
            if body.hp_current.is_some() || body.hp_max.is_some() || body.temp_hp.is_some() || body.ac.is_some() {
                let _ = sqlx::query(
                    r#"update characters set sheet = jsonb_strip_nulls(
                         coalesce(sheet, '{}'::jsonb)
                         || jsonb_build_object(
                              'hp', coalesce(sheet->'hp', '{}'::jsonb)
                                    || jsonb_build_object(
                                         'current', $2::int,
                                         'max',     $3::int,
                                         'temp',    $4::int
                                       ),
                              'ac', $5::int
                            )
                       )
                       where id = $1"#,
                )
                .bind(chid)
                .bind(body.hp_current.unwrap_or(c.hp_current))
                .bind(body.hp_max.unwrap_or(c.hp_max))
                .bind(body.temp_hp.unwrap_or(c.temp_hp))
                .bind(body.ac.unwrap_or(c.ac))
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
    let row: (Uuid, Uuid) = sqlx::query_as(
        "select c.id, e.campaign_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, row.1).await?;
    sqlx::query("delete from combatants where id = $1").bind(id).execute(&s.db).await?;
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
    let row: (Uuid, Uuid, Option<Uuid>, String) = sqlx::query_as(
        r#"select c.id, e.campaign_id, ch.owner_id, c.ref_type::text
           from combatants c
           join encounters e on e.id = c.encounter_id
           left join characters ch on ch.id = c.character_id
           where c.id = $1"#)
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let campaign_id = row.1;
    let owner = row.2;
    let ref_type = row.3;
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    if role != Role::Master {
        // Only owners of the linked character may move their token.
        if ref_type != "character" || owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    let c: Combatant = sqlx::query_as::<_, Combatant>(
        r#"update combatants set token_x = $2, token_y = $3, token_on_map = true
           where id = $1
           returning id, encounter_id, ref_type::text as ref_type, character_id, npc_id, display_name,
                     initiative, dex_tiebreaker, hp_current, hp_max, temp_hp, ac, conditions, notes, is_visible, turn_order, initiative_rolled,
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url"#)
        .bind(id).bind(x).bind(y).fetch_one(&s.db).await?;
    ws::publish(campaign_id, json!({
        "type":"combatant_moved","id":id,"x":x,"y":y
    }).to_string());
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

    // auto-add any party characters not already in this encounter. They
    // start with initiative_rolled = false so they sit out until the owner
    // rolls initiative.
    sqlx::query(
        r#"insert into combatants
             (encounter_id, ref_type, character_id, display_name, initiative,
              hp_current, hp_max, ac, initiative_rolled)
           select $1, 'character'::combatant_ref, ch.id, ch.name,
                  0,
                  coalesce((ch.sheet->'hp'->>'current')::int, (ch.sheet->'hp'->>'max')::int, 0),
                  coalesce((ch.sheet->'hp'->>'max')::int, 0),
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
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
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
                     token_x, token_y, token_color, token_on_map, token_image, null::text as portrait_url"#,
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
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&s.db).await?;
    if rolled == 0 {
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
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"next_turn","id":id,"round":new_round,"turn_index":new_idx}).to_string());
    notify_turn(&s, &e, prev_round).await;
    Ok(Json(e))
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
    let rolled: i64 = sqlx::query_scalar(
        "select count(*) from combatants where encounter_id = $1 and initiative_rolled = true")
        .bind(id).fetch_one(&s.db).await?;
    if rolled == 0 {
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
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
        .bind(id).bind(new_idx).bind(new_round).fetch_one(&s.db).await?;
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
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set turn_index = $2 where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
        .bind(id).bind(body.turn_index).fetch_one(&s.db).await?;
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
    let e: Encounter = sqlx::query_as::<_, Encounter>(
        "update encounters set status = 'ended' where id = $1
         returning id, campaign_id, name, status::text as status, round, turn_index, notes, map_image, map_grid_size, show_grid, updated_at")
        .bind(id).fetch_one(&s.db).await?;
    ws::publish(e.campaign_id, json!({"type":"encounter_ended","id":id}).to_string());
    emit_campaign(&s.db, e.campaign_id, None,
        "combat.ended", &format!("Combat ended: {}", e.name),
        None, Some("encounter"), Some(id)).await;
    Ok(Json(e))
}
