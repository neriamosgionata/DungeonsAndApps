use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct GrappleEscapeBody {
    pub grappler_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct GrappleEscapeResult {
    pub success: bool,
    pub escapee_roll: i32,
    pub escapee_total: i32,
    pub grappler_roll: i32,
    pub grappler_total: i32,
    pub escaped: bool,
}

pub async fn grapple_escape(
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

    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(escapee_snap.encounter_id)
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

    let action_used: bool = sqlx::query_scalar("select action_used from combatants where id = $1")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    if action_used {
        return Err(AppError::BadRequest("action already used".into()));
    }

    if !super::super::has_condition(&escapee_snap.conditions, "grappled") {
        return Err(AppError::BadRequest("not grappled".into()));
    }

    let mut rng = rand::rngs::StdRng::from_os_rng();

    let escapee_stats = combat_engine::compute_stats(&escapee_snap);
    let athletics = escapee_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "athletics")
        .map(|(_, m)| *m)
        .unwrap_or(0);
    let acrobatics = escapee_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "acrobatics")
        .map(|(_, m)| *m)
        .unwrap_or(0);
    let escapee_mod = athletics.max(acrobatics);

    let grappler_stats = combat_engine::compute_stats(&grappler_snap);
    let grappler_athletics = grappler_stats
        .skill_mods
        .iter()
        .find(|(s, _)| s == "athletics")
        .map(|(_, m)| *m)
        .unwrap_or(0);

    let esc_expr = format!("1d20+{}", escapee_mod);
    let grap_expr = format!("1d20+{}", grappler_athletics);

    let esc_roll =
        crate::dice::roll(&esc_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;
    let grap_roll =
        crate::dice::roll(&grap_expr, &mut rng).map_err(|e| AppError::BadRequest(e.to_string()))?;

    let success = esc_roll.total >= grap_roll.total;
    let mut escaped = false;

    let mut tx = s.db.begin().await?;

    if success {
        let esc_conditions = super::super::remove_condition(escapee_snap.conditions.clone(), "grappled");
        let updated: Option<Uuid> = sqlx::query_scalar(
            "update combatants set conditions = $1, action_used = true
             where id = $2 and action_used = false returning id",
        )
        .bind(&esc_conditions)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if updated.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }

        let grap_conditions = super::super::remove_condition(grappler_snap.conditions.clone(), "grappling");
        sqlx::query("update combatants set conditions = $1 where id = $2")
            .bind(&grap_conditions)
            .bind(body.grappler_id)
            .execute(&mut *tx)
            .await?;

        escaped = true;
    } else {
        let updated: Option<Uuid> = sqlx::query_scalar(
            "update combatants set action_used = true
             where id = $1 and action_used = false returning id",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        if updated.is_none() {
            return Err(AppError::BadRequest("action already used".into()));
        }
    }

    tx.commit().await?;

    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({
            "type": "combatant_escapes_grapple",
            "escapee_id": id,
            "grappler_id": body.grappler_id,
            "success": success,
            "escaped": escaped,
        }),
    )
    .await;

    Ok(Json(GrappleEscapeResult {
        success,
        escapee_roll: esc_roll
            .terms
            .first()
            .and_then(|t| t.rolls.first().copied())
            .unwrap_or(0),
        escapee_total: esc_roll.total,
        grappler_roll: grap_roll
            .terms
            .first()
            .and_then(|t| t.rolls.first().copied())
            .unwrap_or(0),
        grappler_total: grap_roll.total,
        escaped,
    }))
}
