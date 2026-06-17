// skill_check, roll_save, computed_stats — minor stat-check endpoints.
use super::*;
use crate::rbac::Role;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SkillCheckBody {
    pub skill: String,
    pub dc: Option<i32>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

pub async fn skill_check(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkillCheckBody>,
) -> AppResult<Json<combat_engine::SkillCheckResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
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

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_skill_checks",
            "combatant_id": id,
            "skill": result.skill,
            "total": result.total,
            "dc": result.dc,
            "passed": result.passed,
        })
        .to_string(),
    );

    Ok(Json(result))
}

#[derive(Debug, Deserialize)]
pub struct SaveBody {
    pub ability: String,
    pub dc: i32,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
    pub is_magical: Option<bool>,
}

pub async fn roll_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<combat_engine::SaveResult>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
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
    let result =
        combat_engine::resolve_save(&snap, &req, &stats).map_err(|e| AppError::BadRequest(e))?;

    ws::publish(
        campaign_id,
        json!({
            "type": "combatant_save",
            "combatant_id": id,
            "passed": result.passed,
            "save_total": result.save_total,
            "dc": result.dc,
            "natural_roll": result.natural_roll,
        })
        .to_string(),
    );

    Ok(Json(result))
}

pub async fn computed_stats(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<combat_engine::ComputedStats>> {
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let stats = combat_engine::compute_stats(&snap);
    Ok(Json(stats))
}
