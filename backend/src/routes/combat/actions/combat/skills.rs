// skill_check, roll_save, computed_stats — minor stat-check endpoints.
use super::*;
use super::super::economy::require_action_auth;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct SkillCheckBody {
    #[validate(length(min = 1, max = 32))]
    pub skill: String,
    #[validate(range(min = 0, max = 50))]
    pub dc: Option<i32>,
    pub advantage: bool,
    pub disadvantage: bool,
    #[validate(length(max = 80))]
    pub label: Option<String>,
}

pub async fn skill_check(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SkillCheckBody>,
) -> AppResult<Json<combat_engine::SkillCheckResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    // MED-5: auth + status + round + role in one query (was 3).
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
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

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_skill_checks",
            "combatant_id": id,
            "skill": result.skill,
            "total": result.total,
            "dc": result.dc,
            "passed": result.passed,
        }),
    )
    .await;

    Ok(Json(result))
}

#[derive(Debug, Deserialize, Validate)]
pub struct SaveBody {
    #[validate(length(min = 1, max = 8))]
    pub ability: String,
    #[validate(range(min = 0, max = 50))]
    pub dc: i32,
    pub advantage: bool,
    pub disadvantage: bool,
    #[validate(length(max = 80))]
    pub label: Option<String>,
    pub is_magical: Option<bool>,
}

pub async fn roll_save(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SaveBody>,
) -> AppResult<Json<combat_engine::SaveResult>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    // MED-5: auth + status + round + role in one query (was 3).
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;

    let snap = combat_engine::load_snapshot(&s.db, id).await?;
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

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_save",
            "combatant_id": id,
            "passed": result.passed,
            "save_total": result.save_total,
            "dc": result.dc,
            "natural_roll": result.natural_roll,
        }),
    )
    .await;

    Ok(Json(result))
}

pub async fn computed_stats(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<combat_engine::ComputedStats>> {
    // computed_stats stays on the 3-RT pattern (load_snapshot + campaign_id +
    // require_member) rather than require_action_auth: it's a read endpoint
    // and the standard helper enforces target ownership + active encounter,
    // which would regress the test that asserts a master can view stats on
    // a non-active encounter. MED-5 only applies to write/combat handlers.
    let snap = combat_engine::load_snapshot(&s.db, id).await?;
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(snap.encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;
    let stats = combat_engine::compute_stats(&snap);
    Ok(Json(stats))
}
