use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac,
    routes::notifications::{NewNotif, emit},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use tracing::warn;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns", get(list).post(create))
        .route("/campaigns/{id}", get(read).patch(update).delete(delete))
        .route("/campaigns/{id}/archive", post(archive))
        .route("/campaigns/{id}/restore", post(restore))
        .route("/campaigns/{id}/calendar", get(get_calendar).patch(update_calendar))
        .route("/campaigns/{id}/calendar/advance", post(advance_calendar))
        .route("/campaigns/{id}/export", get(export_campaign))
        .route("/campaigns/{id}/characters/bulk-level", post(bulk_level))
        .route("/campaigns/import", post(import_campaign))
        .route(
            "/campaigns/{id}/members",
            get(list_members).post(add_member),
        )
        .route(
            "/campaigns/{id}/members/{user_id}",
            axum::routing::patch(update_member).delete(remove_member),
        )
        .route("/campaigns/{id}/presence", get(presence))
}

async fn presence(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<Uuid>>> {
    crate::rbac::require_master(&s.db, uid, id).await?;
    Ok(Json(crate::ws::online_users(id)))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Campaign {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub master_id: Uuid,
    pub icon_url: Option<String>,
    pub leveling: String,
    pub settings: serde_json::Value,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339::option")]
    pub archived_at: Option<OffsetDateTime>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CampaignCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub leveling: Option<String>, // 'xp' | 'milestone'
    /// House rules / campaign settings (free-form jsonb, master-editable).
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CampaignUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub leveling: Option<String>, // 'xp' | 'milestone'
    /// House rules / campaign settings (free-form jsonb, master-editable).
    pub settings: Option<serde_json::Value>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
) -> AppResult<Json<Vec<Campaign>>> {
    let rows: Vec<Campaign> = sqlx::query_as::<_, Campaign>(
        r#"select c.id, c.name, c.description, c.master_id, c.icon_url,
                  c.leveling::text as leveling, c.settings, c.created_at, c.archived_at
           from campaigns c
           join memberships m on m.campaign_id = c.id
           where m.user_id = $1 and c.archived_at is null
           order by c.created_at desc"#,
    )
    .bind(uid)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<CampaignCreate>,
) -> AppResult<(StatusCode, Json<Campaign>)> {
    body.validate()?;

    // Any authenticated user may start a campaign — the membership below makes
    // them campaign master. App-wide admin role is separate (manages users).
    let mut tx = s.db.begin().await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "insert into campaigns (name, description, master_id, icon_url, leveling)
         values ($1, $2, $3, $4, coalesce($5::leveling_mode, 'xp'))
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(&body.name)
    .bind(&body.description)
    .bind(uid)
    .bind(&body.icon_url)
    .bind(&body.leveling)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1, $2, 'master')")
        .bind(c.id)
        .bind(uid)
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into parties (campaign_id) values ($1)")
        .bind(c.id)
        .execute(&mut *tx)
        .await?;

    sqlx::query("insert into campaign_calendar (campaign_id) values ($1) on conflict do nothing")
        .bind(c.id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(c)))
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_member(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "select id, name, description, master_id, icon_url,
                leveling::text as leveling, settings, created_at, archived_at
         from campaigns where id = $1",
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    Ok(Json(c))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CampaignUpdate>,
) -> AppResult<Json<Campaign>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        r#"update campaigns
           set name = coalesce($2, name),
               description = coalesce($3, description),
               icon_url = coalesce($4, icon_url),
               leveling = coalesce($5::leveling_mode, leveling),
               settings = coalesce($6, settings)
           where id = $1
           returning id, name, description, master_id, icon_url,
                     leveling::text as leveling, settings, created_at, archived_at"#,
    )
    .bind(id)
    .bind(body.name)
    .bind(body.description)
    .bind(body.icon_url)
    .bind(body.leveling)
    .bind(body.settings)
    .fetch_one(&s.db)
    .await?;
    crate::ws::publish(
        id,
        serde_json::json!({
            "type":"campaign_updated","id":id,"leveling":c.leveling
        })
        .to_string(),
    );
    Ok(Json(c))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Calendar {
    pub campaign_id: Uuid,
    pub year: i32,
    pub month: i32,
    pub day: i32,
    pub days_per_month: i32,
    pub months: serde_json::Value,
    pub weekdays: serde_json::Value,
    pub notes: String,
    pub weather: String,
    pub holidays: serde_json::Value,
    pub moon_phases: serde_json::Value,
}

async fn get_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Calendar>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        "select campaign_id, year, month, day, days_per_month, months, weekdays, notes, weather, holidays, moon_phases
         from campaign_calendar where campaign_id = $1",
    )
    .bind(cid)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(cal))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CalendarAdvance {
    #[validate(range(min = 1, max = 3650))]
    pub days: i32,
}

async fn advance_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CalendarAdvance>,
) -> AppResult<Json<Calendar>> {
    body.validate()?;
    rbac::require_member(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        r#"with cal as (
             select campaign_id, year, month, day, days_per_month from campaign_calendar where campaign_id = $1
           )
           update campaign_calendar cc set
             year = y, month = m, day = d, updated_at = now()
           from (select
                   (select year from cal) + (($2 + (select day from cal) - 1) / (select days_per_month from cal) + (select month from cal) - 1) / 12 as y,
                   ((select month from cal) - 1 + ($2 + (select day from cal) - 1) / (select days_per_month from cal)) % 12 + 1 as m,
                   (($2 + (select day from cal) - 1) % (select days_per_month from cal)) + 1 as d
                ) adv
           where cc.campaign_id = $1
           returning cc.campaign_id, cc.year, cc.month, cc.day, cc.days_per_month, cc.months, cc.weekdays, cc.notes, cc.weather, cc.holidays, cc.moon_phases"#,
    )
    .bind(cid)
    .bind(body.days)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(cid, serde_json::json!({"type":"calendar_updated"}).to_string());
    Ok(Json(cal))
}

#[derive(Debug, Deserialize, Validate)]
pub struct CalendarUpdate {
    #[validate(range(min = 1, max = 400))]
    pub days_per_month: Option<i32>,
    pub months: Option<serde_json::Value>,
    pub weekdays: Option<serde_json::Value>,
    #[validate(length(max = 2000))]
    pub notes: Option<String>,
    #[validate(length(max = 200))]
    pub weather: Option<String>,
    pub holidays: Option<serde_json::Value>,
    pub moon_phases: Option<serde_json::Value>,
}

async fn update_calendar(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<CalendarUpdate>,
) -> AppResult<Json<Calendar>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let cal: Calendar = sqlx::query_as::<_, Calendar>(
        r#"update campaign_calendar set
             days_per_month = coalesce($2, days_per_month),
             months = coalesce($3, months),
             weekdays = coalesce($4, weekdays),
             notes = coalesce($5, notes),
             weather = coalesce($6, weather),
             holidays = coalesce($7, holidays),
             moon_phases = coalesce($8, moon_phases),
             updated_at = now()
           where campaign_id = $1
           returning campaign_id, year, month, day, days_per_month, months, weekdays, notes, weather, holidays, moon_phases"#,
    )
    .bind(cid)
    .bind(body.days_per_month)
    .bind(body.months)
    .bind(body.weekdays)
    .bind(body.notes)
    .bind(body.weather)
    .bind(body.holidays)
    .bind(body.moon_phases)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(cid, serde_json::json!({"type":"calendar_updated"}).to_string());
    Ok(Json(cal))
}

// =====================================================================
// Campaign export / import (full backup)
// =====================================================================

/// Bulk level update: set level_total for many characters (master-only).
/// Single-class sheets keep their class level in sync.
#[derive(Debug, Deserialize, Validate)]
pub struct BulkLevel {
    #[validate(length(min = 1, max = 500))]
    pub character_ids: Vec<Uuid>,
    #[validate(range(min = 1, max = 20))]
    pub level: i16,
}

async fn bulk_level(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<BulkLevel>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let mut updated = 0i64;
    for chid in &body.character_ids {
        let res = sqlx::query(
            r#"update characters set
                 level_total = $2,
                 sheet = case
                   when jsonb_array_length(sheet->'classes') = 1
                   then jsonb_set(sheet, '{classes,0,level}', to_jsonb($2))
                   else sheet
                 end
               where id = $1 and campaign_id = $3"#,
        )
        .bind(chid)
        .bind(body.level)
        .bind(cid)
        .execute(&s.db)
        .await?;
        updated += res.rows_affected() as i64;
    }
    Ok(Json(serde_json::json!({ "updated": updated })))
}

async fn export_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    rbac::require_master(&s.db, uid, cid).await?;
    let campaign: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select jsonb_build_object('id', id, 'name', name, 'description', description,
                                   'icon_url', icon_url, 'leveling', leveling, 'settings', settings)
         from campaigns where id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let members: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('email', u.email, 'role', m.role)),
                         '[]'::jsonb)
         from memberships m join users u on u.id = m.user_id where m.campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let calendar: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce((select jsonb_build_object('year', year, 'month', month, 'day', day,
                                                   'days_per_month', days_per_month, 'months', months,
                                                   'weekdays', weekdays, 'notes', notes, 'weather', weather, 'holidays', holidays, 'moon_phases', moon_phases)
                          from campaign_calendar where campaign_id = $1), '{}'::jsonb)")
        .bind(cid).fetch_one(&s.db).await?.0;
    let factions: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'name', name, 'color', banner_color,
                                                     'description', description, 'attitude', attitude, 'visibility', visibility)),
                         '[]'::jsonb) from factions where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let npcs: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'name', name, 'role', role,
                                                     'faction_id', faction_id, 'description', description,
                                                     'stats', stats, 'image_key', image_key, 'visibility', visibility)),
                         '[]'::jsonb) from npcs where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let lore: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'title', title, 'category', category,
                                                     'body', body, 'visibility', visibility)),
                         '[]'::jsonb) from lore_entries where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let news: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'title', title, 'body', body, 'visibility', visibility)),
                         '[]'::jsonb) from news_entries where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let sessions: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'title', title, 'session_number', session_number,
                                                     'played_at', played_at, 'status', status, 'recap', recap,
                                                     'visibility', visibility, 'created_by', (select email from users where id = created_by),
                                                     'attendance', (select coalesce(jsonb_agg(email), '[]'::jsonb)
                                                                   from session_attendance a join users u on u.id = a.user_id
                                                                   where a.session_id = campaign_sessions.id))),
                         '[]'::jsonb) from campaign_sessions where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let characters: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'name', name, 'race', race,
                                                     'level_total', level_total, 'sheet', sheet,
                                                     'portrait_url', portrait_url,
                                                     'owner', (select email from users where id = owner_id))),
                         '[]'::jsonb) from characters where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let spells: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('slug', slug, 'name', name, 'level', level, 'school', school,
                                                     'casting_time', casting_time, 'range_text', range_text,
                                                     'components', components, 'duration', duration, 'classes', classes,
                                                     'ritual', ritual, 'concentration', concentration,
                                                     'description', description, 'higher_levels', higher_levels, 'effects', effects)),
                         '[]'::jsonb) from campaign_spells where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let maps: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', m.id, 'name', m.name, 'image_key', m.image_key,
                                                     'pins', (select coalesce(jsonb_agg(jsonb_build_object('x', p.x, 'y', p.y, 'kind', p.kind, 'note', p.note, 'is_party', p.is_party)), '[]'::jsonb)
                                                              from map_pins p where p.map_id = m.id))),
                         '[]'::jsonb) from maps m where m.campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let party: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce((select jsonb_build_object('name', name, 'cp', cp, 'sp', sp, 'ep', ep, 'gp', gp, 'pp', pp,
                                                   'shared_notes', shared_notes)
                          from parties where campaign_id = $1), '{}'::jsonb)")
        .bind(cid).fetch_one(&s.db).await?.0;
    let loot: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', l.id, 'name', l.name, 'qty', l.quantity, 'description', l.description, 'value_gp', l.value_gp,
                                                     'claimed_by', (select ch.name from characters ch where ch.id = l.claimed_by))),
                         '[]'::jsonb) from loot_items l join parties p on p.id = l.party_id where p.campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    let quests: serde_json::Value = sqlx::query_as::<_, (serde_json::Value,)>(
        "select coalesce(jsonb_agg(jsonb_build_object('id', id, 'title', title, 'description', description, 'status', status,
                                                     'npc_ids', coalesce((select jsonb_agg(npc_id) from quest_npcs where quest_id = quests.id), '[]'::jsonb))),
                         '[]'::jsonb) from quests where campaign_id = $1")
        .bind(cid).fetch_one(&s.db).await?.0;
    Ok(Json(serde_json::json!({
        "version": 1,
        "campaign": campaign,
        "members": members,
        "calendar": calendar,
        "factions": factions,
        "npcs": npcs,
        "lore": lore,
        "news": news,
        "sessions": sessions,
        "characters": characters,
        "campaign_spells": spells,
        "maps": maps,
        "party": party,
        "loot": loot,
        "quests": quests,
    })))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ImportBody {
    pub data: serde_json::Value,
}

async fn import_campaign(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Json(body): Json<ImportBody>,
) -> AppResult<(StatusCode, Json<Campaign>)> {
    body.validate()?;
    let d = &body.data;
    let camp = d.get("campaign").cloned().unwrap_or_else(|| serde_json::json!({}));
    let name = camp.get("name").and_then(|v| v.as_str()).unwrap_or("Imported Campaign");
    let mut tx = s.db.begin().await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "insert into campaigns (name, description, master_id, icon_url, leveling, settings)
         values ($1, $2, $3, $4, coalesce($5::leveling_mode, 'xp'), coalesce($6, '{}'::jsonb))
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(name)
    .bind(camp.get("description").and_then(|v| v.as_str()))
    .bind(uid)
    .bind(camp.get("icon_url").and_then(|v| v.as_str()))
    .bind(camp.get("leveling").and_then(|v| v.as_str()))
    .bind(camp.get("settings").cloned())
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("insert into memberships (campaign_id, user_id, role) values ($1, $2, 'master')")
        .bind(c.id).bind(uid).execute(&mut *tx).await?;
    sqlx::query("insert into parties (campaign_id) values ($1)").bind(c.id).execute(&mut *tx).await?;
    sqlx::query("insert into campaign_calendar (campaign_id) values ($1) on conflict do nothing")
        .bind(c.id).execute(&mut *tx).await?;
    let cid = c.id;

    // helper: email → user id
    if let Some(cal) = d.get("calendar").and_then(|v| v.as_object()) {
        if !cal.is_empty() {
            sqlx::query(
                "update campaign_calendar set year = $2, month = $3, day = $4, days_per_month = $5, months = $6, weekdays = $7, notes = $8 where campaign_id = $1")
                .bind(cid)
                .bind(cal.get("year").and_then(|v| v.as_i64()).unwrap_or(1492) as i32)
                .bind(cal.get("month").and_then(|v| v.as_i64()).unwrap_or(1) as i32)
                .bind(cal.get("day").and_then(|v| v.as_i64()).unwrap_or(1) as i32)
                .bind(cal.get("days_per_month").and_then(|v| v.as_i64()).unwrap_or(30) as i32)
                .bind(cal.get("months").cloned().unwrap_or_else(|| serde_json::json!([])))
                .bind(cal.get("weekdays").cloned().unwrap_or_else(|| serde_json::json!([])))
                .bind(cal.get("notes").and_then(|v| v.as_str()).unwrap_or(""))
                .execute(&mut *tx).await?;
            sqlx::query("update campaign_calendar set weather = $2 where campaign_id = $1")
                .bind(cid)
                .bind(cal.get("weather").and_then(|v| v.as_str()).unwrap_or(""))
                .execute(&mut *tx).await?;
            sqlx::query("update campaign_calendar set holidays = $2, moon_phases = $3 where campaign_id = $1")
                .bind(cid)
                .bind(cal.get("holidays").cloned().unwrap_or_else(|| serde_json::json!([])))
                .bind(cal.get("moon_phases").cloned().unwrap_or_else(|| serde_json::json!([])))
                .execute(&mut *tx).await?;
        }
    }

    // factions (map old id → new id)
    let mut faction_map: std::collections::HashMap<Uuid, Uuid> = std::collections::HashMap::new();
    for f in d.get("factions").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let old: Uuid = f.get("id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok()).unwrap_or_else(Uuid::new_v4);
        let new: Uuid = sqlx::query_scalar(
            "insert into factions (campaign_id, name, banner_color, description, attitude, visibility)
             values ($1, $2, $3, $4, $5, coalesce($6::visibility, 'players'))
             returning id")
            .bind(cid)
            .bind(f.get("name").and_then(|v| v.as_str()).unwrap_or("Faction"))
            .bind(f.get("color").and_then(|v| v.as_str()).unwrap_or("#8b6914"))
            .bind(f.get("description").and_then(|v| v.as_str()))
            .bind(f.get("attitude").and_then(|v| v.as_str()).unwrap_or("neutral"))
            .bind(f.get("visibility").and_then(|v| v.as_str()))
            .fetch_one(&mut *tx).await?;
        faction_map.insert(old, new);
    }

    // npcs (faction mapped)
    for n in d.get("npcs").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let fk = n.get("faction_id").and_then(|v| v.as_str()).and_then(|s| s.parse().ok())
            .and_then(|old: Uuid| faction_map.get(&old).copied());
        sqlx::query(
            "insert into npcs (campaign_id, name, role, faction_id, description, stats, image_key, visibility)
             values ($1, $2, $3, $4, $5, $6, $7, coalesce($8::visibility, 'master'))")
            .bind(cid)
            .bind(n.get("name").and_then(|v| v.as_str()).unwrap_or("NPC"))
            .bind(n.get("role").and_then(|v| v.as_str()))
            .bind(fk)
            .bind(n.get("description").and_then(|v| v.as_str()))
            .bind(n.get("stats").cloned().unwrap_or_else(|| serde_json::json!({})))
            .bind(n.get("image_key").and_then(|v| v.as_str()))
            .bind(n.get("visibility").and_then(|v| v.as_str()))
            .execute(&mut *tx).await?;
    }

    // lore + news
    for row in d.get("lore").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        sqlx::query(
            "insert into lore_entries (campaign_id, title, category, body, visibility)
             values ($1, $2, $3, $4, coalesce($5::visibility, 'players'))")
            .bind(cid)
            .bind(row.get("title").and_then(|v| v.as_str()).unwrap_or("Lore"))
            .bind(row.get("category").and_then(|v| v.as_str()).unwrap_or("General"))
            .bind(row.get("body").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(row.get("visibility").and_then(|v| v.as_str()))
            .execute(&mut *tx).await?;
    }
    for row in d.get("news").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        sqlx::query(
            "insert into news_entries (campaign_id, title, body, visibility)
             values ($1, $2, $3, coalesce($4::visibility, 'players'))")
            .bind(cid)
            .bind(row.get("title").and_then(|v| v.as_str()).unwrap_or("News"))
            .bind(row.get("body").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(row.get("visibility").and_then(|v| v.as_str()))
            .execute(&mut *tx).await?;
    }

    // sessions + attendance
    for row in d.get("sessions").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let created_by = match row.get("created_by").and_then(|v| v.as_str()) {
            Some(e) => tx_uid(&mut tx, e).await.unwrap_or(uid),
            None => uid,
        };
        let sid: Uuid = sqlx::query_scalar(
            "insert into campaign_sessions (campaign_id, title, session_number, played_at, status, recap, visibility, created_by)
             values ($1, $2, $3, $4, coalesce($5::session_status, 'completed'), $6, coalesce($7::visibility, 'players'), $8)
             returning id")
            .bind(cid)
            .bind(row.get("title").and_then(|v| v.as_str()).unwrap_or("Session"))
            .bind(row.get("session_number").and_then(|v| v.as_i64()).map(|v| v as i32))
            .bind(row.get("played_at").and_then(|v| v.as_str()))
            .bind(row.get("status").and_then(|v| v.as_str()))
            .bind(row.get("recap").and_then(|v| v.as_str()))
            .bind(row.get("visibility").and_then(|v| v.as_str()))
            .bind(created_by)
            .fetch_one(&mut *tx).await?;
        if let Some(att) = row.get("attendance").and_then(|v| v.as_array()) {
            for email in att {
                if let Some(e) = email.as_str() {
                    if let Some(u) = tx_uid(&mut tx, e).await {
                        sqlx::query("insert into session_attendance (session_id, user_id) values ($1, $2) on conflict do nothing")
                            .bind(sid).bind(u).execute(&mut *tx).await?;
                    }
                }
            }
        }
    }

    // characters (owner by email, fallback: uid)
    for row in d.get("characters").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let owner = match row.get("owner").and_then(|v| v.as_str()) {
            Some(e) => tx_uid(&mut tx, e).await.unwrap_or(uid),
            None => uid,
        };
        sqlx::query(
            "insert into characters (campaign_id, owner_id, name, race, level_total, sheet, portrait_url)
             values ($1, $2, $3, $4, coalesce($5, 1), coalesce($6, '{}'::jsonb), $7)")
            .bind(cid)
            .bind(owner)
            .bind(row.get("name").and_then(|v| v.as_str()).unwrap_or("Character"))
            .bind(row.get("race").and_then(|v| v.as_str()))
            .bind(row.get("level_total").and_then(|v| v.as_i64()).map(|v| v as i16))
            .bind(row.get("sheet").cloned())
            .bind(row.get("portrait_url").and_then(|v| v.as_str()))
            .execute(&mut *tx).await?;
    }

    // campaign spells
    for row in d.get("campaign_spells").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        sqlx::query(
            "insert into campaign_spells (campaign_id, slug, name, level, school, casting_time, range_text,
                                          components, duration, classes, ritual, concentration, description,
                                          higher_levels, effects)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
             on conflict (campaign_id, slug) do nothing")
            .bind(cid)
            .bind(row.get("slug").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(row.get("name").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(row.get("level").and_then(|v| v.as_i64()).unwrap_or(0) as i16)
            .bind(row.get("school").and_then(|v| v.as_str()).unwrap_or("Evocation"))
            .bind(row.get("casting_time").and_then(|v| v.as_str()))
            .bind(row.get("range_text").and_then(|v| v.as_str()))
            .bind(row.get("components").and_then(|v| v.as_str()))
            .bind(row.get("duration").and_then(|v| v.as_str()))
            .bind(row.get("classes").cloned().unwrap_or_else(|| serde_json::json!([])))
            .bind(row.get("ritual").and_then(|v| v.as_bool()).unwrap_or(false))
            .bind(row.get("concentration").and_then(|v| v.as_bool()).unwrap_or(false))
            .bind(row.get("description").and_then(|v| v.as_str()).unwrap_or(""))
            .bind(row.get("higher_levels").and_then(|v| v.as_str()))
            .bind(row.get("effects").cloned().unwrap_or_else(|| serde_json::json!({})))
            .execute(&mut *tx).await?;
    }

    // maps + pins
    for row in d.get("maps").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let map_id: Uuid = sqlx::query_scalar(
            "insert into maps (campaign_id, name, image_key) values ($1, $2, $3) returning id")
            .bind(cid)
            .bind(row.get("name").and_then(|v| v.as_str()).unwrap_or("Map"))
            .bind(row.get("image_key").and_then(|v| v.as_str()))
            .fetch_one(&mut *tx).await?;
        if let Some(pins) = row.get("pins").and_then(|v| v.as_array()) {
            for p in pins {
                sqlx::query(
                    "insert into map_pins (map_id, x, y, kind, note, is_party) values ($1, $2, $3, $4, $5, $6)")
                    .bind(map_id)
                    .bind(p.get("x").and_then(|v| v.as_f64()).unwrap_or(50.0))
                    .bind(p.get("y").and_then(|v| v.as_f64()).unwrap_or(50.0))
                    .bind(p.get("kind").and_then(|v| v.as_str()).unwrap_or("pin"))
                    .bind(p.get("note").and_then(|v| v.as_str()))
                    .bind(p.get("is_party").and_then(|v| v.as_bool()).unwrap_or(false))
                    .execute(&mut *tx).await?;
            }
        }
    }

    // loot + quests
    for row in d.get("loot").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        let party_id: Uuid = sqlx::query_scalar("select id from parties where campaign_id = $1")
            .bind(cid).fetch_one(&mut *tx).await?;
        sqlx::query(
            "insert into loot_items (party_id, name, quantity, description, value_gp)
             values ($1, $2, $3, $4, $5)")
            .bind(party_id)
            .bind(row.get("name").and_then(|v| v.as_str()).unwrap_or("Item"))
            .bind(row.get("qty").and_then(|v| v.as_i64()).unwrap_or(1) as i32)
            .bind(row.get("description").and_then(|v| v.as_str()))
            .bind(row.get("value_gp").and_then(|v| v.as_f64()).unwrap_or(0.0))
            .execute(&mut *tx).await?;
    }
    for row in d.get("quests").and_then(|v| v.as_array()).cloned().unwrap_or_default() {
        sqlx::query(
            "insert into quests (campaign_id, title, description, status)
             values ($1, $2, $3, coalesce($4::quest_status, 'active'))")
            .bind(cid)
            .bind(row.get("title").and_then(|v| v.as_str()).unwrap_or("Quest"))
            .bind(row.get("description").and_then(|v| v.as_str()))
            .bind(row.get("status").and_then(|v| v.as_str()))
            .execute(&mut *tx).await?;
    }

    tx.commit().await?;
    crate::ws::publish(cid, serde_json::json!({"type":"campaign_created","id":cid}).to_string());
    Ok((StatusCode::CREATED, Json(c)))
}

// helper: resolve email → user id inside a tx
async fn tx_uid(tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, email: &str) -> Option<Uuid> {
    sqlx::query_scalar("select id from users where email = $1")
        .bind(email)
        .fetch_optional(&mut **tx)
        .await
        .ok()
        .flatten()
}

async fn archive(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "update campaigns set archived_at = now() where id = $1
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(c))
}

async fn restore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Campaign>> {
    rbac::require_master(&s.db, uid, id).await?;
    let c: Campaign = sqlx::query_as::<_, Campaign>(
        "update campaigns set archived_at = null where id = $1
         returning id, name, description, master_id, icon_url,
                   leveling::text as leveling, settings, created_at, archived_at",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(c))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, id).await?;
    sqlx::query("delete from campaigns where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Serialize, FromRow)]
pub struct Member {
    pub user_id: Uuid,
    pub display_name: String,
    pub email: String,
    pub role: String,
    pub character_limit: i32,
}

async fn list_members(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<Member>>> {
    rbac::require_member(&s.db, uid, id).await?;
    let rows: Vec<Member> = sqlx::query_as::<_, Member>(
        r#"select u.id as user_id, u.display_name, u.email::text as email, m.role::text as role, m.character_limit
           from memberships m join users u on u.id = m.user_id
           where m.campaign_id = $1 order by m.joined_at"#,
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Debug, Deserialize)]
pub struct AddMember {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct MemberUpdate {
    #[validate(range(min = 1, max = 20))]
    pub character_limit: Option<i32>,
    pub role: Option<String>,
}

async fn update_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((campaign_id, target)): Path<(Uuid, Uuid)>,
    Json(body): Json<MemberUpdate>,
) -> AppResult<Json<Member>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, campaign_id).await?;
    if let Some(r) = &body.role {
        if r != "player" && r != "master" {
            return Err(AppError::BadRequest("invalid role".into()));
        }
    }
    sqlx::query(
        "update memberships set
           character_limit = coalesce($3, character_limit),
           role            = coalesce($4::membership_role, role)
         where campaign_id = $1 and user_id = $2",
    )
    .bind(campaign_id)
    .bind(target)
    .bind(body.character_limit)
    .bind(&body.role)
    .execute(&s.db)
    .await?;
    let m: Member = sqlx::query_as::<_, Member>(
        r#"select u.id as user_id, u.display_name, u.email::text as email, m.role::text as role, m.character_limit
           from memberships m join users u on u.id = m.user_id
           where m.campaign_id = $1 and m.user_id = $2"#,
    )
    .bind(campaign_id).bind(target).fetch_optional(&s.db).await?
    .ok_or(AppError::NotFound)?;
    crate::ws::publish(
        campaign_id,
        serde_json::json!({
            "type": "member_updated", "user_id": target
        })
        .to_string(),
    );
    Ok(Json(m))
}

async fn remove_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((campaign_id, target)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, campaign_id).await?;
    let campaign_master: Uuid = sqlx::query_scalar("select master_id from campaigns where id = $1")
        .bind(campaign_id)
        .fetch_one(&s.db)
        .await?;
    if target == campaign_master {
        return Err(AppError::BadRequest("cannot remove campaign master".into()));
    }
    let res = sqlx::query("delete from memberships where campaign_id = $1 and user_id = $2")
        .bind(campaign_id)
        .bind(target)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    crate::ws::publish(
        campaign_id,
        serde_json::json!({
            "type": "member_removed", "user_id": target
        })
        .to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn add_member(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddMember>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    rbac::require_master(&s.db, uid, id).await?;
    if body.role != "player" && body.role != "master" {
        return Err(AppError::BadRequest("invalid role".into()));
    }
    let target: Uuid = sqlx::query_scalar("select id from users where email = $1")
        .bind(&body.email)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;

    let already: Option<i64> =
        sqlx::query_scalar("select 1 from memberships where campaign_id = $1 and user_id = $2")
            .bind(id)
            .bind(target)
            .fetch_optional(&s.db)
            .await?;
    if already.is_some() {
        return Err(AppError::Conflict("already a member".into()));
    }

    sqlx::query(
        "insert into campaign_invitations (campaign_id, user_id, role, invited_by)
         values ($1, $2, $3::membership_role, $4)
         on conflict (campaign_id, user_id) do update
           set role = excluded.role, invited_by = excluded.invited_by,
               responded_at = null, accepted = null, created_at = now()",
    )
    .bind(id)
    .bind(target)
    .bind(&body.role)
    .bind(uid)
    .execute(&s.db)
    .await?;

    let inv_id: Uuid = sqlx::query_scalar(
        "select id from campaign_invitations where campaign_id = $1 and user_id = $2",
    )
    .bind(id)
    .bind(target)
    .fetch_one(&s.db)
    .await?;

    let campaign_name: String = sqlx::query_scalar("select name from campaigns where id = $1")
        .bind(id)
        .fetch_one(&s.db)
        .await
        .unwrap_or_else(|e| {
            warn!(%e, "campaign name lookup failed");
            String::new()
        });
    emit(
        &s.db,
        NewNotif {
            user_id: target,
            campaign_id: Some(id),
            kind: "campaign.invitation",
            title: &format!("Invitation to {campaign_name}"),
            body: Some(&format!("Role: {}", body.role)),
            ref_kind: Some("invitation"),
            ref_id: Some(inv_id),
        },
    )
    .await;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "invitation_id": inv_id, "pending": true,
        })),
    ))
}
