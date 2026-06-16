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
        .route("/encounters/{id}/effects", get(list_by_encounter))
        .route("/combatants/{id}/effects", get(list_by_combatant).post(apply_manual))
        .route("/combatants/{id}/effects/apply-spell", post(apply_spell))
        .route("/combatants/{id}/effects/{effect_id}", axum::routing::patch(update).delete(remove))
}

// =====================================================================
// DTOs
// =====================================================================

#[derive(Debug, Serialize, FromRow)]
pub struct Effect {
    pub id: Uuid,
    pub combatant_id: Uuid,
    pub name: String,
    pub kind: String,
    pub icon: String,
    pub duration_unit: String,
    pub duration_value: Option<i32>,
    pub remaining: Option<i32>,
    pub tick_trigger: String,
    pub concentration: bool,
    pub caster_combatant_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_name: Option<String>,
    pub source_spell_slug: Option<String>,
    pub modifiers: serde_json::Value,
    pub active: bool,
    pub applied_at_round: i32,
    pub applied_at_turn_index: i32,
    pub created_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct ApplyManual {
    #[validate(length(min = 1, max = 80))]
    pub name: String,
    pub kind: String, // buff | debuff | neutral | condition
    #[validate(length(min = 1, max = 40))]
    pub icon: Option<String>,
    pub duration_unit: String, // rounds | minutes | hours | permanent
    pub duration_value: Option<i32>,
    pub remaining: Option<i32>,
    pub tick_trigger: Option<String>,
    pub concentration: Option<bool>,
    pub caster_combatant_id: Option<Uuid>,
    pub source_type: Option<String>,
    pub source_name: Option<String>,
    pub modifiers: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ApplySpell {
    pub spell_slug: String,
    pub caster_combatant_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateEffect {
    #[validate(length(min = 1, max = 80))]
    pub name: Option<String>,
    pub active: Option<bool>,
    pub remaining: Option<i32>,
}

// =====================================================================
// Auth helpers
// =====================================================================

async fn require_effect_owner(
    db: &sqlx::PgPool,
    uid: Uuid,
    effect_id: Uuid,
) -> AppResult<(Effect, Uuid)> {
    let e: Effect = sqlx::query_as::<_, Effect>(
        "select id, combatant_id, name, kind::text, icon, duration_unit::text, duration_value, remaining,
                tick_trigger::text, concentration, caster_combatant_id, source_type, source_name, source_spell_slug,
                modifiers, active, applied_at_round, applied_at_turn_index, created_at
         from combatant_effects where id = $1")
        .bind(effect_id)
        .fetch_optional(db).await?.ok_or(AppError::NotFound)?;

    let campaign_id: Uuid = sqlx::query_scalar(
        "select e.campaign_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(e.combatant_id)
        .fetch_one(db).await?;

    let role = rbac::require_member(db, uid, campaign_id).await?;
    if role != Role::Master {
        // Player may only edit their own character-linked combatant
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(e.combatant_id)
            .fetch_optional(db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    Ok((e, campaign_id))
}

async fn can_modify_combatant(
    db: &sqlx::PgPool,
    uid: Uuid,
    combatant_id: Uuid,
) -> AppResult<Uuid> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select e.campaign_id from combatants c join encounters e on e.id = c.encounter_id where c.id = $1")
        .bind(combatant_id)
        .fetch_one(db).await?;
    let role = rbac::require_member(db, uid, campaign_id).await?;
    if role != Role::Master {
        let owner: Option<Uuid> = sqlx::query_scalar(
            "select ch.owner_id from combatants c left join characters ch on ch.id = c.character_id where c.id = $1")
            .bind(combatant_id)
            .fetch_optional(db).await?;
        if owner != Some(uid) {
            return Err(AppError::Forbidden);
        }
    }
    Ok(campaign_id)
}

// =====================================================================
// Concentration helper
// =====================================================================

async fn break_concentration_tx(conn: &mut sqlx::PgConnection, caster_combatant_id: Uuid) -> AppResult<()> {
    sqlx::query(
        "update combatant_effects set active = false
         where caster_combatant_id = $1 and concentration = true and active = true")
        .bind(caster_combatant_id)
        .execute(conn).await?;
    Ok(())
}

// =====================================================================
// Endpoints
// =====================================================================

async fn list_by_encounter(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Effect>>> {
    let campaign_id: Uuid = sqlx::query_scalar(
        "select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db).await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let rows: Vec<Effect> = sqlx::query_as::<_, Effect>(
        r#"select ce.id, ce.combatant_id, ce.name, ce.kind::text, ce.icon, ce.duration_unit::text,
                  ce.duration_value, ce.remaining, ce.tick_trigger::text, ce.concentration,
                  ce.caster_combatant_id, ce.source_type, ce.source_name, ce.source_spell_slug,
                  ce.modifiers, ce.active, ce.applied_at_round, ce.applied_at_turn_index, ce.created_at
           from combatant_effects ce
           join combatants c on c.id = ce.combatant_id
           where c.encounter_id = $1
           order by ce.active desc, ce.created_at desc"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn list_by_combatant(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(combatant_id): Path<Uuid>,
) -> AppResult<Json<Vec<Effect>>> {
    let campaign_id = can_modify_combatant(&s.db, uid, combatant_id).await?;
    let _role = rbac::require_member(&s.db, uid, campaign_id).await?;

    let rows: Vec<Effect> = sqlx::query_as::<_, Effect>(
        r#"select id, combatant_id, name, kind::text, icon, duration_unit::text,
                  duration_value, remaining, tick_trigger::text, concentration,
                  caster_combatant_id, source_type, source_name, source_spell_slug,
                  modifiers, active, applied_at_round, applied_at_turn_index, created_at
           from combatant_effects
           where combatant_id = $1 and active = true
           order by created_at desc"#,
    )
    .bind(combatant_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn apply_manual(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(combatant_id): Path<Uuid>,
    Json(body): Json<ApplyManual>,
) -> AppResult<(StatusCode, Json<Effect>)> {
    body.validate()?;
    let campaign_id = can_modify_combatant(&s.db, uid, combatant_id).await?;

    let kind = match body.kind.as_str() {
        "buff" | "debuff" | "neutral" | "condition" => body.kind.clone(),
        _ => return Err(AppError::BadRequest("kind must be buff|debuff|neutral|condition".into())),
    };
    let unit = match body.duration_unit.as_str() {
        "rounds" | "minutes" | "hours" | "permanent" => body.duration_unit.clone(),
        _ => return Err(AppError::BadRequest("duration_unit must be rounds|minutes|hours|permanent".into())),
    };
    let tick = match body.tick_trigger.as_deref().unwrap_or("round_end") {
        "round_end" | "target_turn_start" | "target_turn_end" | "caster_turn_start" | "caster_turn_end" | "never" => body.tick_trigger.clone().unwrap_or_else(|| "round_end".into()),
        _ => return Err(AppError::BadRequest("tick_trigger invalid".into())),
    };
    if let Some(ref st) = body.source_type {
        if !matches!(st.as_str(), "spell" | "ability" | "item" | "weapon" | "manual" | "condition") {
            return Err(AppError::BadRequest("source_type must be spell|ability|item|weapon|manual|condition".into()));
        }
    }

    // Fetch current encounter round/turn for applied_at tracking
    let (round, turn_index): (i32, i32) = sqlx::query_as(
        "select e.round, e.turn_index from encounters e join combatants c on c.encounter_id = e.id where c.id = $1")
        .bind(combatant_id)
        .fetch_one(&s.db).await?;

    let concentration = body.concentration.unwrap_or(false);
    let caster_id = body.caster_combatant_id;

    let mut tx = s.db.begin().await?;

    // Break existing concentration from same caster (inside tx)
    if concentration {
        if let Some(cid) = caster_id {
            break_concentration_tx(&mut *tx, cid).await?;
        }
    }

    let remaining = body.remaining.or(body.duration_value);

    let e: Effect = sqlx::query_as::<_, Effect>(
        r#"insert into combatant_effects
           (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
            concentration, caster_combatant_id, source_type, source_name, modifiers, applied_at_round, applied_at_turn_index)
           values ($1, $2, $3::effect_kind, $4, $5::duration_unit, $6, $7, $8::tick_trigger,
                   $9, $10, $11, $12, $13, $14, $15)
           returning id, combatant_id, name, kind::text, icon, duration_unit::text,
                     duration_value, remaining, tick_trigger::text, concentration,
                     caster_combatant_id, source_type, source_name, source_spell_slug,
                     modifiers, active, applied_at_round, applied_at_turn_index, created_at"#,
    )
    .bind(combatant_id)
    .bind(&body.name)
    .bind(&kind)
    .bind(body.icon.as_deref().unwrap_or("circle-dot"))
    .bind(&unit)
    .bind(body.duration_value)
    .bind(remaining)
    .bind(&tick)
    .bind(concentration)
    .bind(caster_id)
    .bind(body.source_type.as_deref())
    .bind(body.source_name.as_deref())
    .bind(body.modifiers.unwrap_or_else(|| json!({})))
    .bind(round)
    .bind(turn_index)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    ws::publish(campaign_id, json!({"type":"effects_change","combatant_id":combatant_id}).to_string());
    Ok((StatusCode::CREATED, Json(e)))
}

async fn apply_spell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(combatant_id): Path<Uuid>,
    Json(body): Json<ApplySpell>,
) -> AppResult<(StatusCode, Json<Vec<Effect>>)> {
    let campaign_id = can_modify_combatant(&s.db, uid, combatant_id).await?;

    // Fetch spell effect templates
    let templates: serde_json::Value = sqlx::query_scalar(
        "select effects from spells where slug = $1")
        .bind(&body.spell_slug)
        .fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;

    let template_arr: Vec<serde_json::Value> = serde_json::from_value(templates)
        .map_err(|e| AppError::BadRequest(format!("invalid effect template: {e}")))?;

    if template_arr.is_empty() {
        return Err(AppError::BadRequest("spell has no effect templates".into()));
    }

    let (round, turn_index): (i32, i32) = sqlx::query_as(
        "select e.round, e.turn_index from encounters e join combatants c on c.encounter_id = e.id where c.id = $1")
        .bind(combatant_id)
        .fetch_one(&s.db).await?;

    let caster_id = body.caster_combatant_id;
    let mut created = Vec::new();

    let mut tx = s.db.begin().await?;

    for t in template_arr {
        let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("Effect").to_string();
        let kind = t.get("kind").and_then(|v| v.as_str()).unwrap_or("neutral").to_string();
        let icon = t.get("icon").and_then(|v| v.as_str()).unwrap_or("circle-dot").to_string();
        let duration_unit = t.get("duration_unit").and_then(|v| v.as_str()).unwrap_or("rounds").to_string();
        let duration_value = t.get("duration_value").and_then(|v| v.as_i64()).map(|v| v as i32);
        let tick_trigger = t.get("tick_trigger").and_then(|v| v.as_str()).unwrap_or("round_end").to_string();
        let concentration = t.get("concentration").and_then(|v| v.as_bool()).unwrap_or(false);
        let modifiers = t.get("modifiers").cloned().unwrap_or_else(|| json!({}));

        let remaining = duration_value;

        // Break concentration if needed (inside tx)
        if concentration {
            if let Some(cid) = caster_id {
                break_concentration_tx(&mut *tx, cid).await?;
            }
        }

        let e: Effect = sqlx::query_as::<_, Effect>(
            r#"insert into combatant_effects
               (combatant_id, name, kind, icon, duration_unit, duration_value, remaining, tick_trigger,
                concentration, caster_combatant_id, source_type, source_name, source_spell_slug, modifiers,
                applied_at_round, applied_at_turn_index)
               values ($1, $2, $3::effect_kind, $4, $5::duration_unit, $6, $7, $8::tick_trigger,
                       $9, $10, 'spell', $11, $12, $13, $14, $15)
               returning id, combatant_id, name, kind::text, icon, duration_unit::text,
                         duration_value, remaining, tick_trigger::text, concentration,
                         caster_combatant_id, source_type, source_name, source_spell_slug,
                         modifiers, active, applied_at_round, applied_at_turn_index, created_at"#,
        )
        .bind(combatant_id)
        .bind(&name)
        .bind(&kind)
        .bind(&icon)
        .bind(&duration_unit)
        .bind(duration_value)
        .bind(remaining)
        .bind(&tick_trigger)
        .bind(concentration)
        .bind(caster_id)
        .bind(&body.spell_slug) // source_name = spell slug for reference
        .bind(&body.spell_slug)
        .bind(modifiers)
        .bind(round)
        .bind(turn_index)
        .fetch_one(&mut *tx)
        .await?;
        created.push(e);
    }

    tx.commit().await?;

    ws::publish(campaign_id, json!({"type":"effects_change","combatant_id":combatant_id}).to_string());
    Ok((StatusCode::CREATED, Json(created)))
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((combatant_id, effect_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateEffect>,
) -> AppResult<Json<Effect>> {
    body.validate()?;
    let (effect, campaign_id) = require_effect_owner(&s.db, uid, effect_id).await?;
    if effect.combatant_id != combatant_id {
        return Err(AppError::BadRequest("effect does not belong to combatant".into()));
    }

    let e: Effect = sqlx::query_as::<_, Effect>(
        r#"update combatant_effects set
             name      = coalesce($2, name),
             active    = coalesce($3, active),
             remaining = coalesce($4, remaining)
           where id = $1
           returning id, combatant_id, name, kind::text, icon, duration_unit::text,
                     duration_value, remaining, tick_trigger::text, concentration,
                     caster_combatant_id, source_type, source_name, source_spell_slug,
                     modifiers, active, applied_at_round, applied_at_turn_index, created_at"#,
    )
    .bind(effect_id)
    .bind(body.name)
    .bind(body.active)
    .bind(body.remaining)
    .fetch_one(&s.db)
    .await?;

    ws::publish(campaign_id, json!({"type":"effects_change","combatant_id":combatant_id}).to_string());
    Ok(Json(e))
}

async fn remove(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((combatant_id, effect_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let (effect, campaign_id) = require_effect_owner(&s.db, uid, effect_id).await?;
    if effect.combatant_id != combatant_id {
        return Err(AppError::BadRequest("effect does not belong to combatant".into()));
    }

    sqlx::query("delete from combatant_effects where id = $1")
        .bind(effect_id)
        .execute(&s.db)
        .await?;

    ws::publish(campaign_id, json!({"type":"effects_change","combatant_id":combatant_id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
