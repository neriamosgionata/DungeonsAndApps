// Overlay CRUD endpoints.
use super::*;
use crate::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Serialize, FromRow)]
pub struct Overlay {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub kind: String,
    pub shape: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: String,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub active: bool,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub created_at: OffsetDateTime,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct OverlayCreate {
    pub kind: String,
    pub shape: String,
    pub origin_x: f64,
    pub origin_y: f64,
    pub end_x: Option<f64>,
    pub end_y: Option<f64>,
    pub radius_ft: Option<i32>,
    pub length_ft: Option<i32>,
    pub width_ft: Option<i32>,
    pub angle_deg: Option<f64>,
    pub points: Option<serde_json::Value>,
    pub color: Option<String>,
    pub label: Option<String>,
    pub zone_type: Option<String>,
    pub expires_at_round: Option<i32>,
    pub expires_at_turn: Option<i32>,
    pub source_spell_slug: Option<String>,
    pub created_by_combatant_id: Option<Uuid>,
    pub hazard_damage_expression: Option<String>,
    pub hazard_damage_type: Option<String>,
    pub hazard_save_ability: Option<String>,
    pub hazard_save_dc: Option<i32>,
    pub hazard_half_on_save: Option<bool>,
}

pub async fn list_overlays(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
) -> AppResult<Json<Vec<Overlay>>> {
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    let rows: Vec<Overlay> = sqlx::query_as::<_, Overlay>(
        r#"select id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                  radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                  active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                  hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save
           from encounter_overlays
           where encounter_id = $1 and active = true
           order by created_at desc"#,
    )
    .bind(encounter_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn create_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(encounter_id): Path<Uuid>,
    Json(body): Json<OverlayCreate>,
) -> AppResult<(StatusCode, Json<Overlay>)> {
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    let o: Overlay = sqlx::query_as::<_, Overlay>(
        r#"insert into encounter_overlays
           (encounter_id, kind, shape, origin_x, origin_y, end_x, end_y, radius_ft, length_ft, width_ft, angle_deg, points,
            color, label, zone_type, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id,
            hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, hazard_half_on_save)
           values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21, $22, $23, $24)
           returning id, encounter_id, kind, shape, origin_x, origin_y, end_x, end_y,
                     radius_ft, length_ft, width_ft, angle_deg, points, color, label, zone_type,
                     active, expires_at_round, expires_at_turn, source_spell_slug, created_by_combatant_id, created_at,
                     hazard_damage_expression, hazard_damage_type, hazard_save_ability, hazard_save_dc, coalesce(hazard_half_on_save, false) as hazard_half_on_save"#,
    )
    .bind(encounter_id)
    .bind(&body.kind)
    .bind(&body.shape)
    .bind(body.origin_x)
    .bind(body.origin_y)
    .bind(body.end_x)
    .bind(body.end_y)
    .bind(body.radius_ft)
    .bind(body.length_ft)
    .bind(body.width_ft)
    .bind(body.angle_deg)
    .bind(body.points)
    .bind(body.color.as_deref().unwrap_or("rgba(255,0,0,0.25)"))
    .bind(body.label.as_deref())
    .bind(body.zone_type.as_deref())
    .bind(body.expires_at_round)
    .bind(body.expires_at_turn)
    .bind(body.source_spell_slug.as_deref())
    .bind(body.created_by_combatant_id)
    .bind(body.hazard_damage_expression.as_deref())
    .bind(body.hazard_damage_type.as_deref())
    .bind(body.hazard_save_ability.as_deref())
    .bind(body.hazard_save_dc)
    .bind(body.hazard_half_on_save.unwrap_or(false))
    .fetch_one(&s.db)
    .await?;

    ws::publish(
        campaign_id,
        json!({"type":"overlay_adds","encounter_id":encounter_id,"id":o.id}).to_string(),
    );
    Ok((StatusCode::CREATED, Json(o)))
}

pub async fn delete_overlay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((encounter_id, overlay_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let campaign_id: Uuid = sqlx::query_scalar("select campaign_id from encounters where id = $1")
        .bind(encounter_id)
        .fetch_one(&s.db)
        .await?;
    rbac::require_master(&s.db, uid, campaign_id).await?;

    sqlx::query("update encounter_overlays set active = false where id = $1 and encounter_id = $2")
        .bind(overlay_id)
        .bind(encounter_id)
        .execute(&s.db)
        .await?;

    ws::publish(
        campaign_id,
        json!({"type":"overlay_removes","encounter_id":encounter_id,"id":overlay_id}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}
