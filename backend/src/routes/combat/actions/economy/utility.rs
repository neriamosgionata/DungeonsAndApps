// search_action, use_object — minor utility handlers.
use super::*;
use super::auth::consume_action_or_bonus;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct SearchBody {
    pub label: Option<String>,
}

pub async fn search_action(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<SearchBody>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let encounter_id = auth.encounter_id;
    let round = auth.round;

    let mut tx = s.db.begin().await?;
    consume_action_or_bonus(&mut tx, id, false).await?;

    let label = body.label.unwrap_or_else(|| "Search".to_string());
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, action, note) values ($1, $2, $3, $4, $5)")
        .bind(encounter_id).bind(round).bind(id).bind(&label).bind("search")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({"type":"combatant_searches","id":id,"label":label}),
    )
    .await;
    Ok(Json(c))
}

#[derive(Debug, Deserialize)]
pub struct UseObjectBody {
    pub label: Option<String>,
    pub target_id: Option<Uuid>,
}

pub async fn use_object(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UseObjectBody>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let campaign_id = auth.campaign_id;
    let encounter_id = auth.encounter_id;
    let round = auth.round;

    let mut tx = s.db.begin().await?;
    consume_action_or_bonus(&mut tx, id, false).await?;

    let label = body.label.unwrap_or_else(|| "Use an Object".to_string());
    sqlx::query(
        "insert into combat_events (encounter_id, round, actor_combatant, target_combatant, action, note) values ($1, $2, $3, $4, $5, $6)")
        .bind(encounter_id).bind(round).bind(id).bind(body.target_id).bind(&label).bind("use_object")
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    let c = super::super::super::refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        campaign_id,
        json!({"type":"combatant_uses_object","id":id,"label":label}),
    )
    .await;
    Ok(Json(c))
}
