use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/characters", get(list).post(create))
        .route("/characters/{id}", get(read).patch(update).delete(delete))
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

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<CharacterCreate>,
) -> AppResult<(StatusCode, Json<Character>)> {
    body.validate()?;
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
            .bind(campaign_id).bind(owner).fetch_optional(&mut *tx).await?.unwrap_or(1);
        
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
               values ($1, $2, $3, $4, coalesce($5, 1), coalesce($6, '{}'::jsonb), $7)
               returning id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at"#,
        )
        .bind(campaign_id)
        .bind(owner)
        .bind(&body.name)
        .bind(&body.race)
        .bind(body.level_total)
        .bind(body.sheet)
        .bind(&body.portrait_url)
        .fetch_one(&mut *tx)
        .await?;
        
        tx.commit().await?;
        return Ok((StatusCode::CREATED, Json(c)));
    }
    
    // Masters bypass the limit
    let c: Character = sqlx::query_as::<_, Character>(
        r#"insert into characters (campaign_id, owner_id, name, race, level_total, sheet, portrait_url)
           values ($1, $2, $3, $4, coalesce($5, 1), coalesce($6, '{}'::jsonb), $7)
           returning id, campaign_id, owner_id, name, race, level_total, sheet, portrait_url, updated_at"#,
    )
    .bind(campaign_id)
    .bind(owner)
    .bind(&body.name)
    .bind(&body.race)
    .bind(body.level_total)
    .bind(body.sheet)
    .bind(&body.portrait_url)
    .fetch_one(&s.db)
    .await?;
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
    let prev_hp_cur = prev.sheet.get("hp").and_then(|h| h.get("current")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let prev_hp_max = prev.sheet.get("hp").and_then(|h| h.get("max")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let prev_temp   = prev.sheet.get("hp").and_then(|h| h.get("temp")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let prev_ac     = prev.sheet.get("ac").and_then(|v| v.as_i64()).map(|v| v as i32);
    let hp_current = c.sheet.get("hp").and_then(|h| h.get("current")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let hp_max     = c.sheet.get("hp").and_then(|h| h.get("max")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let temp_hp    = c.sheet.get("hp").and_then(|h| h.get("temp")).and_then(|v| v.as_i64()).map(|v| v as i32);
    let ac         = c.sheet.get("ac").and_then(|v| v.as_i64()).map(|v| v as i32);
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
    fetch_authz(&s, uid, id, true).await?;
    sqlx::query("delete from characters where id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}
