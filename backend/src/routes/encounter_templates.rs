// Encounter templates: save a composition of NPCs, spawn it into any
// encounter (bestiary-style quick prep).
use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser, rbac};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/encounter-templates", get(list).post(create))
        .route("/campaigns/{id}/encounter-templates/{template_id}", axum::routing::delete(delete))
        .route("/encounters/{id}/spawn-from-template", post(spawn))
}

#[derive(Debug, Serialize, FromRow)]
pub struct EncounterTemplate {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub combatants: serde_json::Value,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TemplateCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    /// [{ display_name, hp_max, ac, stats, count }]
    pub combatants: Option<serde_json::Value>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<EncounterTemplate>>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let rows: Vec<EncounterTemplate> = sqlx::query_as::<_, EncounterTemplate>(
        "select id, campaign_id, name, combatants from encounter_templates
         where campaign_id = $1 order by name",
    )
    .bind(cid)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<TemplateCreate>,
) -> AppResult<(StatusCode, Json<EncounterTemplate>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let t: EncounterTemplate = sqlx::query_as::<_, EncounterTemplate>(
        "insert into encounter_templates (campaign_id, name, combatants)
         values ($1, $2, coalesce($3, '[]'::jsonb))
         returning id, campaign_id, name, combatants",
    )
    .bind(cid)
    .bind(&body.name)
    .bind(body.combatants)
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(t)))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, tid)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, cid).await?;
    let res = sqlx::query("delete from encounter_templates where id = $1 and campaign_id = $2")
        .bind(tid)
        .bind(cid)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct SpawnBody {
    pub template_id: Uuid,
}

async fn spawn(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(eid): Path<Uuid>,
    Json(body): Json<SpawnBody>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    let row: (Uuid, String) =
        sqlx::query_as("select campaign_id, status::text from encounters where id = $1")
            .bind(eid)
            .fetch_optional(&s.db)
            .await?
            .ok_or(AppError::NotFound)?;
    let (cid, status) = row;
    rbac::require_master(&s.db, uid, cid).await?;
    if status != "planned" && status != "active" {
        return Err(AppError::BadRequest("encounter not spawnable".into()));
    }
    let t: EncounterTemplate = sqlx::query_as::<_, EncounterTemplate>(
        "select id, campaign_id, name, combatants from encounter_templates
         where id = $1 and campaign_id = $2",
    )
    .bind(body.template_id)
    .bind(cid)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;

    let mut added = 0i32;
    let combatants = t.combatants.as_array().cloned().unwrap_or_default();
    for c in &combatants {
        let name = c.get("display_name").and_then(|v| v.as_str()).unwrap_or("Creature");
        let count = c.get("count").and_then(|v| v.as_i64()).unwrap_or(1).clamp(1, 100) as i32;
        let stats = c.get("stats").cloned().unwrap_or_else(|| serde_json::json!({}));
        let hp_max = c.get("hp_max").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
        let ac = c.get("ac").and_then(|v| v.as_i64()).unwrap_or(10) as i32;
        let mut tx = s.db.begin().await?;
        for i in 0..count {
            let suffix = if count > 1 { format!(" {}", i + 1) } else { String::new() };
            let npc_id: Uuid = sqlx::query_scalar(
                "insert into npcs (campaign_id, name, stats) values ($1, $2, $3) returning id",
            )
            .bind(cid)
            .bind(format!("{name}{suffix}"))
            .bind(&stats)
            .fetch_one(&mut *tx)
            .await?;
            sqlx::query(
                "insert into combatants (encounter_id, ref_type, npc_id, display_name, hp_max, hp_current, ac)
                 values ($1, 'npc', $2, $3, $4, $5, $6)",
            )
            .bind(eid)
            .bind(npc_id)
            .bind(format!("{name}{suffix}"))
            .bind(hp_max)
            .bind(hp_max)
            .bind(ac)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        added += count;
    }
    crate::ws::publish(
        cid,
        serde_json::json!({"type": "encounter_updated", "id": eid}).to_string(),
    );
    Ok(Json(serde_json::json!({ "added": added, "template": t.name })))
}
