use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;
use rand::SeedableRng;

fn reset_short_resources(sheet: &Value) -> Value {
    let resources = sheet.get("resources").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let new_res: Vec<Value> = resources.into_iter().map(|r| {
        let reset = r.get("reset").and_then(|v| v.as_str()).unwrap_or("");
        if reset == "short" || reset == "long" {
            let max = r.get("max").and_then(|v| v.as_i64()).unwrap_or(0);
            let mut m = r.clone();
            if let Some(obj) = m.as_object_mut() { obj.insert("current".into(), serde_json::json!(max)); }
            m
        } else { r }
    }).collect();
    serde_json::to_value(new_res).unwrap_or(Value::Null)
}

fn reset_all_resources(sheet: &Value) -> Value {
    let resources = sheet.get("resources").and_then(|r| r.as_array()).cloned().unwrap_or_default();
    let new_res: Vec<Value> = resources.into_iter().map(|r| {
        let reset = r.get("reset").and_then(|v| v.as_str()).unwrap_or("");
        if reset != "none" {
            let max = r.get("max").and_then(|v| v.as_i64()).unwrap_or(0);
            let mut m = r.clone();
            if let Some(obj) = m.as_object_mut() { obj.insert("current".into(), serde_json::json!(max)); }
            m
        } else { r }
    }).collect();
    serde_json::to_value(new_res).unwrap_or(Value::Null)
}

fn reset_features_by_reset(sheet: &Value, reset_filter: &[&str]) -> Value {
    let features = sheet.get("features").and_then(|f| f.as_array()).cloned().unwrap_or_default();
    let new_feat: Vec<Value> = features.into_iter().map(|f| {
        let reset = f.get("uses").and_then(|u| u.get("reset")).and_then(|v| v.as_str()).unwrap_or("");
        if reset_filter.contains(&reset) {
            let max = f.get("uses").and_then(|u| u.get("max")).and_then(|v| v.as_i64()).unwrap_or(0);
            let mut m = f.clone();
            if let Some(obj) = m.as_object_mut() {
                if let Some(uses) = obj.get_mut("uses").and_then(|u| u.as_object_mut()) {
                    uses.insert("current".into(), serde_json::json!(max));
                }
            }
            m
        } else { f }
    }).collect();
    serde_json::to_value(new_feat).unwrap_or(Value::Null)
}

fn sheet_i32(v: Option<&Value>, default: i64, min: i32, max: i32) -> i32 {
    let raw = v.and_then(|v| v.as_i64()).unwrap_or(default);
    raw.clamp(min as i64, max as i64) as i32
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/characters", get(list).post(create))
        .route("/characters/{id}", get(read).patch(update).delete(delete))
        .route("/characters/{id}/short-rest", post(short_rest))
        .route("/characters/{id}/long-rest", post(long_rest))
        .route("/campaigns/{id}/award-xp", post(award_xp))
        .route("/characters/{id}/spells", get(list_spells).post(add_spell))
        .route("/characters/{id}/spells/{spell_id}", patch(update_spell).delete(remove_spell))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Character {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub race: Option<String>,
    pub level_total: i16,
    pub sheet: Value,
    pub portrait_url: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CharacterCreate {
    #[validate(length(min = 1, max = 80))]
    pub name: String,
    pub race: Option<String>,
    #[validate(range(min = 1, max = 20))]
    pub level_total: Option<i16>,
    pub sheet: Option<Value>,
    pub owner_id: Option<Uuid>,
    pub portrait_url: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CharacterUpdate {
    #[validate(length(min = 1, max = 80))]
    pub name: Option<String>,
    pub race: Option<String>,
    #[validate(range(min = 1, max = 20))]
    pub level_total: Option<i16>,
    pub sheet: Option<Value>,
    pub portrait_url: Option<String>,
    pub clear_portrait: Option<bool>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct CharacterSpell {
    pub spell_id: Uuid,
    pub name: String,
    pub slug: String,
    pub level: i16,
    pub prepared: bool,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CharacterSpellCreate {
    pub spell_id: Uuid,
    pub prepared: Option<bool>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CharacterSpellUpdate {
    pub prepared: Option<bool>,
    pub notes: Option<Option<String>>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<Json<Vec<Character>>> {
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let rows: Vec<Character> = if role == Role::Master {
        sqlx::query_as::<_, Character>(
            "select id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at
             from characters where campaign_id = $1 order by updated_at desc",
        )
        .bind(campaign_id)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query_as::<_, Character>(
            "select id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at
             from characters where campaign_id = $1 and owner_id = $2 order by updated_at desc",
        )
        .bind(campaign_id)
        .bind(uid)
        .fetch_all(&s.db)
        .await?
    };
    Ok(Json(rows))
}

fn default_sheet() -> Value {
    serde_json::json!({
        "abilities": {"str": 10, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10},
        "hp": {"current": 1, "max": 1},
        "ac": 10,
        "hit_dice": {"current": 1, "max": 1, "die": "d8"},
        "death_saves": {"successes": 0, "failures": 0},
        "alive": true,
        "inspiration": false,
        "exhaustion": 0,
        "slots": {},
        "xp": 0,
    })
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<CharacterCreate>,
) -> AppResult<(StatusCode, Json<Character>)> {
    body.validate()?;
    let sheet = body.sheet.clone().unwrap_or_else(default_sheet);
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let owner = if role == Role::Master {
        body.owner_id.unwrap_or(uid)
    } else {
        if let Some(o) = body.owner_id {
            if o != uid { return Err(AppError::Forbidden); }
        }
        uid
    };
    // Verify `owner` is actually a member of this campaign. Prevents master
    // from creating phantom characters for arbitrary users.
    if owner != uid {
        let is_member: Option<i64> = sqlx::query_scalar(
            "select 1 from memberships where campaign_id = $1 and user_id = $2")
            .bind(campaign_id).bind(owner).fetch_optional(&s.db).await?;
        if is_member.is_none() {
            return Err(AppError::BadRequest("owner_id is not a member of this campaign".into()));
        }
    }

    // per-player cap (masters bypass) - using transaction to prevent TOCTOU race
    if role != Role::Master {
        let mut tx = s.db.begin().await?;
        
        // Lock the membership row to prevent concurrent character creation
        let limit: i32 = sqlx::query_scalar(
            "select character_limit from memberships where campaign_id = $1 and user_id = $2 for update")
            .bind(campaign_id).bind(owner).fetch_optional(&mut *tx).await?
            .ok_or(AppError::Forbidden)?;
        
        let used: i64 = sqlx::query_scalar(
            "select count(*) from characters where campaign_id = $1 and owner_id = $2")
            .bind(campaign_id).bind(owner).fetch_one(&mut *tx).await?;
        
        if (used as i32) >= limit {
            tx.rollback().await?;
            return Err(AppError::Conflict(
                format!("character limit reached ({limit}); ask the master to raise it")));
        }
        
        // Insert within the transaction to maintain consistency
        let c: Character = sqlx::query_as::<_, Character>(
            r#"insert into characters (campaign_id, owner_id, name, race, level_total, sheet, portrait_url)
               values ($1, $2, $3, $4, coalesce($5, 1), $6, $7)
               returning id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at"#,
        )
        .bind(campaign_id)
        .bind(owner)
        .bind(&body.name)
        .bind(&body.race)
        .bind(body.level_total)
        .bind(&sheet)
        .bind(&body.portrait_url)
        .fetch_one(&mut *tx)
        .await?;
        
        tx.commit().await?;
        ws::publish(campaign_id, serde_json::json!({
            "type":"character_created","id":c.id
        }).to_string());
        return Ok((StatusCode::CREATED, Json(c)));
    }
    
    // Masters bypass the limit
    let c: Character = sqlx::query_as::<_, Character>(
        r#"insert into characters (campaign_id, owner_id, name, race, level_total, sheet, portrait_url)
           values ($1, $2, $3, $4, coalesce($5, 1), $6, $7)
           returning id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at"#,
    )
    .bind(campaign_id)
    .bind(owner)
    .bind(&body.name)
    .bind(&body.race)
    .bind(body.level_total)
    .bind(&sheet)
    .bind(&body.portrait_url)
    .fetch_one(&s.db)
    .await?;
    ws::publish(campaign_id, serde_json::json!({
        "type":"character_created","id":c.id
    }).to_string());
    Ok((StatusCode::CREATED, Json(c)))
}

async fn fetch_authz(s: &AppState, uid: Uuid, id: Uuid, mutate: bool) -> AppResult<Character> {
    let c: Character = sqlx::query_as::<_, Character>(
        "select id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at
         from characters where id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, c.campaign_id).await?;
    if mutate {
        // Only the owner may modify a character sheet. Master/admin can view
        // but not edit — they have their own tools (combat view, NPCs, etc.).
        if c.owner_id != uid { return Err(AppError::Forbidden); }
    } else if role != Role::Master && c.owner_id != uid {
        return Err(AppError::Forbidden);
    }
    Ok(c)
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Character>> {
    Ok(Json(fetch_authz(&s, uid, id, false).await?))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CharacterUpdate>,
) -> AppResult<Json<Character>> {
    body.validate()?;
    let prev = fetch_authz(&s, uid, id, true).await?;

    // Guard: non-masters cannot flip their own `alive` flag. That transition
    // must come from death-save logic (3 successes → revive, 3 failures → die)
    // or from a master/admin override.
    let mut new_sheet = body.sheet.clone();
    if let Some(sheet) = &mut new_sheet {
        let role = rbac::require_member(&s.db, uid, prev.campaign_id).await?;
        if role != Role::Master {
            let prev_alive = prev.sheet.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
            let new_alive  = sheet.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
            let new_fails  = sheet.get("death_saves").and_then(|d| d.get("failures")).and_then(|v| v.as_i64()).unwrap_or(0);
            let new_succ   = sheet.get("death_saves").and_then(|d| d.get("successes")).and_then(|v| v.as_i64()).unwrap_or(0);
            if new_alive != prev_alive {
                let allowed = (prev_alive && !new_alive && new_fails >= 3) // die: 3 fails
                    || (!prev_alive && new_alive && new_succ == 0 && new_fails == 0); // revive via stabilize
                if !allowed {
                    if let Some(obj) = sheet.as_object_mut() {
                        obj.insert("alive".into(), serde_json::Value::Bool(prev_alive));
                    }
                }
            }
        }
    }

    let clear_portrait = body.clear_portrait.unwrap_or(false);
    let c: Character = sqlx::query_as::<_, Character>(
        r#"update characters set
             name         = coalesce($2, name),
             race         = coalesce($3, race),
             level_total  = coalesce($4, level_total),
             sheet        = coalesce($5, sheet),
             portrait_url = case when $7 then null else coalesce($6, portrait_url) end
           where id = $1
           returning id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at"#,
    )
    .bind(id)
    .bind(body.name)
    .bind(body.race)
    .bind(body.level_total)
    .bind(new_sheet)
    .bind(body.portrait_url)
    .bind(clear_portrait)
    .fetch_one(&s.db)
    .await?;

    crate::ws::publish(c.campaign_id, serde_json::json!({
        "type":"character_updated","id":c.id
    }).to_string());

    // If the character is now dead, pull them out of any active encounters.
    let alive = c.sheet.get("alive").and_then(|v| v.as_bool()).unwrap_or(true);
    if !alive {
        let removed: Vec<(Uuid, Uuid)> = sqlx::query_as(
            r#"delete from combatants c
               using encounters e
               where c.encounter_id = e.id
                 and c.character_id = $1
                 and e.status in ('planned','active')
               returning c.id, e.id"#,
        )
        .bind(c.id).fetch_all(&s.db).await.unwrap_or_default();
        for (_cid, enc_id) in &removed {
            crate::ws::publish(c.campaign_id, serde_json::json!({
                "type":"combatant_removed","id":_cid,"encounter_id":enc_id
            }).to_string());
        }
    }

    // Sync HP/AC into combatants for active encounters ONLY when the value
    // actually changed vs the previous sheet. Prevents a feedback loop with
    // combat → sheet → combat writes.
    let prev_hp_cur = prev.sheet.get("hp").and_then(|h| h.get("current")).map(|v| sheet_i32(Some(v), 0, 0, 9999));
    let prev_hp_max = prev.sheet.get("hp").and_then(|h| h.get("max")).map(|v| sheet_i32(Some(v), 1, 0, 9999));
    let prev_temp   = prev.sheet.get("hp").and_then(|h| h.get("temp")).map(|v| sheet_i32(Some(v), 0, 0, 9999));
    let prev_ac     = prev.sheet.get("ac").map(|v| sheet_i32(Some(v), 10, 0, 99));
    let hp_current = c.sheet.get("hp").and_then(|h| h.get("current")).map(|v| sheet_i32(Some(v), 0, 0, 9999));
    let hp_max     = c.sheet.get("hp").and_then(|h| h.get("max")).map(|v| sheet_i32(Some(v), 1, 0, 9999));
    let temp_hp    = c.sheet.get("hp").and_then(|h| h.get("temp")).map(|v| sheet_i32(Some(v), 0, 0, 9999));
    let ac         = c.sheet.get("ac").map(|v| sheet_i32(Some(v), 10, 0, 99));
    let changed = hp_current != prev_hp_cur || hp_max != prev_hp_max || temp_hp != prev_temp || ac != prev_ac;
    if changed && (hp_current.is_some() || hp_max.is_some() || temp_hp.is_some() || ac.is_some()) {
        let updated: Vec<(Uuid, Uuid)> = sqlx::query_as(
            r#"update combatants c
               set hp_current = coalesce($2, c.hp_current),
                   hp_max     = coalesce($3, c.hp_max),
                   temp_hp    = coalesce($4, c.temp_hp),
                   ac         = coalesce($5, c.ac)
               from encounters e
               where c.encounter_id = e.id
                 and c.character_id = $1
                 and e.status in ('planned','active')
               returning c.id, e.id"#,
        )
        .bind(c.id).bind(hp_current).bind(hp_max).bind(temp_hp).bind(ac)
        .fetch_all(&s.db).await.unwrap_or_default();
        for (_cid, enc_id) in &updated {
            crate::ws::publish(c.campaign_id, serde_json::json!({
                "type":"combatant_updated","id":_cid,"encounter_id":enc_id,
                "hp_current":hp_current,"hp_max":hp_max,"temp_hp":temp_hp,"ac":ac
            }).to_string());
        }
    }

    Ok(Json(c))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let c = fetch_authz(&s, uid, id, true).await?;
    // Prevent deleting a character that is active in an encounter
    let active_count: i64 = sqlx::query_scalar(
        r#"select count(*) from combatants c
           join encounters e on e.id = c.encounter_id
           where c.character_id = $1 and e.status in ('active','planned')"#)
        .bind(id).fetch_one(&s.db).await?;
    if active_count > 0 {
        return Err(AppError::BadRequest("cannot delete character while they are in an active or planned encounter".into()));
    }
    sqlx::query("delete from characters where id = $1").bind(id).execute(&s.db).await?;
    ws::publish(c.campaign_id, serde_json::json!({
        "type":"character_deleted","id":id
    }).to_string());
    Ok(StatusCode::NO_CONTENT)
}

async fn list_spells(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Vec<CharacterSpell>>> {
    let _c = fetch_authz(&s, uid, id, false).await?;
    let rows: Vec<CharacterSpell> = sqlx::query_as::<_, CharacterSpell>(
        "select s.id as spell_id, s.name, s.slug, s.level, cs.prepared, cs.notes
         from character_spells cs
         join spells s on s.id = cs.spell_id
         where cs.character_id = $1
         order by s.level, s.name")
        .bind(id).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

async fn add_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<CharacterSpellCreate>,
) -> AppResult<StatusCode> {
    let _c = fetch_authz(&s, uid, id, true).await?;
    sqlx::query(
        "insert into character_spells (character_id, spell_id, prepared, notes)
         values ($1, $2, coalesce($3, false), $4)")
        .bind(id).bind(body.spell_id).bind(body.prepared).bind(body.notes)
        .execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn update_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((id, spell_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<CharacterSpellUpdate>,
) -> AppResult<StatusCode> {
    let _c = fetch_authz(&s, uid, id, true).await?;
    let set_notes = body.notes.is_some();
    let notes_val = body.notes.flatten();
    let res = sqlx::query(
        "update character_spells set
           prepared = coalesce($2, prepared),
           notes = case when $3 then $4 else notes end
         where character_id = $1 and spell_id = $5")
        .bind(id).bind(body.prepared).bind(set_notes).bind(notes_val).bind(spell_id)
        .execute(&s.db).await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((id, spell_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let _c = fetch_authz(&s, uid, id, true).await?;
    let res = sqlx::query("delete from character_spells where character_id = $1 and spell_id = $2")
        .bind(id).bind(spell_id).execute(&s.db).await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================
// Rest mechanics
// =====================================================================

#[derive(Debug, Deserialize, Validate)]
pub struct ShortRestBody {
    #[validate(range(min = 1, max = 20))]
    pub hit_dice_spent: i32,
}

#[derive(Debug, Serialize)]
pub struct ShortRestResult {
    pub hp_before: i32,
    pub hp_after: i32,
    pub hp_max: i32,
    pub hit_dice_before: i32,
    pub hit_dice_after: i32,
    pub roll_total: i32,
    pub con_mod: i32,
}

async fn short_rest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ShortRestBody>,
) -> AppResult<Json<ShortRestResult>> {
    body.validate()?;
    let c = fetch_authz(&s, uid, id, true).await?;

    let sheet = &c.sheet;
    let hp_current = sheet_i32(sheet.get("hp").and_then(|h| h.get("current")), 0, 0, 9999);
    let hp_max = sheet_i32(sheet.get("hp").and_then(|h| h.get("max")), 1, 0, 9999);
    let hit_dice_current = sheet_i32(sheet.get("hit_dice").and_then(|h| h.get("current")), 0, 0, 999);
    let _hit_dice_max = sheet_i32(sheet.get("hit_dice").and_then(|h| h.get("max")), 0, 0, 999);
    let die = sheet.get("hit_dice").and_then(|h| h.get("die")).and_then(|v| v.as_str()).unwrap_or("d8");
    let con_score = sheet.get("abilities").and_then(|a| a.get("con")).and_then(|v| v.as_i64()).unwrap_or(10).clamp(1, 30);
    let con_mod = ((con_score - 10) / 2) as i32;

    if body.hit_dice_spent > hit_dice_current {
        return Err(AppError::BadRequest(format!(
            "only {hit_dice_current} hit dice available"
        )));
    }

    // Roll hit dice
    let has_durable = sheet.get("feats")
        .and_then(|f| f.as_array())
        .map(|a| a.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("durable")))
        .unwrap_or(false);
    let durable_min = if has_durable { (2 * con_mod).max(2) } else { 0 };
    let mut rng = rand::rngs::StdRng::from_os_rng();
    let expr = format!("{}{}", body.hit_dice_spent, die);
    let roll_res = crate::dice::roll(&expr, &mut rng)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    // Durable: each die heals at least 2×CON mod (min 2)
    let clamped_total = if has_durable {
        roll_res.terms.first()
            .map(|t| t.rolls.iter().map(|&r| r.max(durable_min)).sum::<i32>())
            .unwrap_or(roll_res.total)
    } else {
        roll_res.total
    };
    let roll_total = clamped_total + con_mod * body.hit_dice_spent;
    let hp_after = (hp_current + roll_total).min(hp_max);
    let hit_dice_after = hit_dice_current - body.hit_dice_spent;

    // Warlock pact slot level by class level (PHB p.107)
    let pact_slot_level = |wl: i32| -> i32 {
        if wl >= 9 { 5 } else if wl >= 7 { 4 } else if wl >= 5 { 3 } else if wl >= 3 { 2 } else { 1 }
    };
    let warlock_level: i32 = sheet.get("classes")
        .and_then(|a| a.as_array())
        .map(|arr| arr.iter()
            .filter(|c| c.get("name").and_then(|n| n.as_str()).map(|n| n.eq_ignore_ascii_case("warlock")).unwrap_or(false))
            .filter_map(|c| c.get("level").and_then(|l| l.as_i64()))
            .sum::<i64>() as i32)
        .unwrap_or(0);
    let new_slots: Value = if warlock_level > 0 {
        let psl = pact_slot_level(warlock_level);
        let slots = sheet.get("slots").cloned().unwrap_or_else(|| serde_json::json!({}));
        if let Some(obj) = slots.as_object() {
            let mut m = obj.clone();
            if let Some(slot) = m.get_mut(&psl.to_string()).and_then(|s| s.as_object_mut()) {
                if let Some(max) = slot.get("max").and_then(|v| v.as_i64()) {
                    slot.insert("current".into(), serde_json::json!(max));
                }
            }
            serde_json::Value::Object(m)
        } else { slots }
    } else { Value::Null };

    let res = reset_short_resources(sheet);
    let feats = reset_features_by_reset(sheet, &["short", "long"]);

    // Update sheet
    let mut binds = 5u32;
    let mut sql = r#"update characters set sheet =
         coalesce(sheet, '{}'::jsonb)
         || jsonb_build_object(
              'hp', coalesce(sheet->'hp', '{}'::jsonb)
                    || jsonb_build_object('current', $2::int),
              'hit_dice', coalesce(sheet->'hit_dice', '{}'::jsonb)
                          || jsonb_build_object('current', $3::int),
              'resources', $4::jsonb,
              'features', $5::jsonb"#.to_string();
    let mut bound_slots: Option<Value> = None;
    if new_slots.is_object() {
        binds += 1;
        sql.push_str(&format!(", 'slots', ${binds}::jsonb"));
        bound_slots = Some(new_slots);
    }
    sql.push_str(") where id = $1");

    let mut q = sqlx::query(&sql)
        .bind(id)
        .bind(hp_after)
        .bind(hit_dice_after)
        .bind(res)
        .bind(feats);
    if let Some(ref sl) = bound_slots {
        q = q.bind(sl);
    }
    q.execute(&s.db).await?;

    // Sync HP into any active combatants
    let updated: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"update combatants c
           set hp_current = $2
           from encounters e
           where c.encounter_id = e.id
             and c.character_id = $1
             and e.status in ('planned','active')
           returning c.id, e.id"#,
    )
    .bind(id).bind(hp_after)
    .fetch_all(&s.db).await.unwrap_or_default();
    for (_cid, enc_id) in &updated {
        ws::publish(c.campaign_id, serde_json::json!({
            "type":"combatant_updated","id":_cid,"encounter_id":enc_id,
            "hp_current":hp_after
        }).to_string());
    }

    ws::publish(c.campaign_id, serde_json::json!({
        "type":"character_updated","id":id
    }).to_string());

    Ok(Json(ShortRestResult {
        hp_before: hp_current,
        hp_after,
        hp_max,
        hit_dice_before: hit_dice_current,
        hit_dice_after,
        roll_total,
        con_mod,
    }))
}

#[derive(Debug, Serialize)]
pub struct LongRestResult {
    pub hp_before: i32,
    pub hp_after: i32,
    pub hit_dice_before: i32,
    pub hit_dice_after: i32,
    pub hit_dice_max: i32,
    pub exhaustion_before: i32,
    pub exhaustion_after: i32,
}

async fn long_rest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<LongRestResult>> {
    let c = fetch_authz(&s, uid, id, true).await?;

    let sheet = &c.sheet;
    let hp_max = sheet_i32(sheet.get("hp").and_then(|h| h.get("max")), 1, 0, 9999);
    let hp_before = sheet_i32(sheet.get("hp").and_then(|h| h.get("current")), 0, 0, 9999);
    let hit_dice_current = sheet_i32(sheet.get("hit_dice").and_then(|h| h.get("current")), 0, 0, 999);
    let hit_dice_max = sheet_i32(sheet.get("hit_dice").and_then(|h| h.get("max")), 0, 0, 999);
    let exhaustion_before = sheet_i32(sheet.get("exhaustion"), 0, 0, 6);

    let hp_after = hp_max;
    let hit_dice_after = hit_dice_max.min(hit_dice_current + (hit_dice_max as f32 / 2.0).ceil() as i32);
    let exhaustion_after = (exhaustion_before - 1).max(0);

    // Build slot reset: set all slot.current = slot.max
    let slots = sheet.get("slots").cloned().unwrap_or_else(|| serde_json::json!({}));
    let mut new_slots = serde_json::Map::new();
    if let Some(obj) = slots.as_object() {
        for (k, v) in obj {
            if let Some(slot_obj) = v.as_object() {
                let mut new_slot = slot_obj.clone();
                if let Some(max) = slot_obj.get("max").and_then(|m| m.as_i64()) {
                    new_slot.insert("current".into(), serde_json::json!(max));
                }
                new_slots.insert(k.clone(), serde_json::json!(new_slot));
            }
        }
    }

    let lr_res = reset_all_resources(sheet);
    let lr_feats = reset_features_by_reset(sheet, &["short", "long"]);

    sqlx::query(
        r#"update characters set sheet =
             coalesce(sheet, '{}'::jsonb)
             || jsonb_build_object(
                  'hp', coalesce(sheet->'hp', '{}'::jsonb)
                        || jsonb_build_object('current', $2::int),
                  'hit_dice', coalesce(sheet->'hit_dice', '{}'::jsonb)
                              || jsonb_build_object('current', $3::int),
                  'exhaustion', $4::int,
                  'death_saves', jsonb_build_object('successes', 0, 'failures', 0),
                  'alive', true,
                  'slots', $5::jsonb,
                  'resources', $6::jsonb,
                  'features', $7::jsonb
                )
           where id = $1"#,
    )
    .bind(id)
    .bind(hp_after)
    .bind(hit_dice_after)
    .bind(exhaustion_after)
    .bind(serde_json::json!(new_slots))
    .bind(lr_res)
    .bind(lr_feats)
    .execute(&s.db).await?;

    // Sync HP into any active combatants
    let updated: Vec<(Uuid, Uuid)> = sqlx::query_as(
        r#"update combatants c
           set hp_current = $2, temp_hp = 0
           from encounters e
           where c.encounter_id = e.id
             and c.character_id = $1
             and e.status in ('planned','active')
           returning c.id, e.id"#,
    )
    .bind(id).bind(hp_after)
    .fetch_all(&s.db).await.unwrap_or_default();
    for (_cid, enc_id) in &updated {
        ws::publish(c.campaign_id, serde_json::json!({
            "type":"combatant_updated","id":_cid,"encounter_id":enc_id,
            "hp_current":hp_after,"temp_hp":0
        }).to_string());
    }

    ws::publish(c.campaign_id, serde_json::json!({
        "type":"character_updated","id":id
    }).to_string());

    Ok(Json(LongRestResult {
        hp_before,
        hp_after,
        hit_dice_before: hit_dice_current,
        hit_dice_after,
        hit_dice_max,
        exhaustion_before,
        exhaustion_after,
    }))
}


// =====================================================================
// XP Tracking
// =====================================================================

fn xp_for_level(level: i32) -> i32 {
    match level {
        1 => 0,
        2 => 300,
        3 => 900,
        4 => 2700,
        5 => 6500,
        6 => 14000,
        7 => 23000,
        8 => 34000,
        9 => 48000,
        10 => 64000,
        11 => 85000,
        12 => 100000,
        13 => 120000,
        14 => 140000,
        15 => 165000,
        16 => 195000,
        17 => 225000,
        18 => 265000,
        19 => 305000,
        20 => 355000,
        _ => 355000,
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct AwardXpBody {
    pub character_ids: Vec<Uuid>,
    #[validate(range(min = 1, max = 500_000))]
    pub xp_each: i32,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AwardXpResult {
    pub characters_awarded: Vec<XpAwardEntry>,
}

#[derive(Debug, Serialize)]
pub struct XpAwardEntry {
    pub character_id: Uuid,
    pub character_name: String,
    pub xp_before: i32,
    pub xp_after: i32,
    pub xp_gained: i32,
    pub leveled_up: bool,
    pub new_level: i16,
}

async fn award_xp(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<AwardXpBody>,
) -> AppResult<Json<AwardXpResult>> {
    body.validate()?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let mut tx = s.db.begin().await?;
    let mut characters_awarded = Vec::new();

    for chid in &body.character_ids {
        let c: Character = sqlx::query_as::<_, Character>(
            "select id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at
             from characters where id = $1 and campaign_id = $2")
            .bind(chid).bind(campaign_id).fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;

        let xp_before = sheet_i32(c.sheet.get("xp"), 0, 0, 355_000);
        let xp_after = xp_before.saturating_add(body.xp_each);
        let mut new_level = c.level_total;
        let mut leveled_up = false;

        // Check for level-up
        for lvl in (c.level_total + 1)..=20i16 {
            if xp_after >= xp_for_level(lvl as i32) {
                new_level = lvl;
                leveled_up = true;
            } else {
                break;
            }
        }

        sqlx::query(
            r#"update characters set
                 sheet = coalesce(sheet, '{}'::jsonb)
                         || jsonb_build_object('xp', $2::int),
                 level_total = $3
               where id = $1"#,
        )
        .bind(chid)
        .bind(xp_after)
        .bind(new_level)
        .execute(&mut *tx).await?;

        characters_awarded.push(XpAwardEntry {
            character_id: *chid,
            character_name: c.name.clone(),
            xp_before,
            xp_after,
            xp_gained: body.xp_each,
            leveled_up,
            new_level,
        });
    }

    tx.commit().await?;

    for entry in &characters_awarded {
        ws::publish(campaign_id, serde_json::json!({
            "type": "xp_awarded",
            "character_id": entry.character_id,
            "character_name": entry.character_name,
            "xp_gained": entry.xp_gained,
            "xp_after": entry.xp_after,
            "leveled_up": entry.leveled_up,
            "new_level": entry.new_level,
            "reason": body.reason,
        }).to_string());
    }

    Ok(Json(AwardXpResult { characters_awarded }))
}
