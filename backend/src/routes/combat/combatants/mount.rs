// Mounted combat (PHB p.198): mount/dismount endpoints.
use super::*;
use super::super::actions::sync::refresh_combatant;
use super::Combatant;
use super::super::economy::require_action_auth;
use crate::AppState;
use crate::extract::AuthUser;
use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct MountBody {
    /// The mount combatant id (same encounter, alive).
    pub mount_id: Uuid,
}

pub async fn mount(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<MountBody>,
) -> AppResult<Json<Combatant>> {
    body.validate()
        .map_err(|e| AppError::BadRequest(format!("invalid body: {e}")))?;
    let auth = require_action_auth(&s.db, uid, id).await?;
    let mut tx = s.db.begin().await?;

    // Same encounter, mount alive, not already ridden (one rider per mount).
    let mount: Option<(i32,)> = sqlx::query_as(
        "select hp_current from combatants where id = $1 and encounter_id = $2 and id != $3 for update",
    )
    .bind(body.mount_id)
    .bind(auth.encounter_id)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let (mount_hp,) = mount.ok_or(AppError::BadRequest(
        "mount must be in the same encounter".into(),
    ))?;
    if mount_hp <= 0 {
        return Err(AppError::BadRequest("cannot mount a dead mount".into()));
    }
    let already_ridden: Option<Uuid> = sqlx::query_scalar(
        "select id from combatants where mounted_on = $1 limit 1",
    )
    .bind(body.mount_id)
    .fetch_optional(&mut *tx)
    .await?;
    if already_ridden.is_some() {
        return Err(AppError::BadRequest("mount already has a rider".into()));
    }
    // Rider must not already be mounted on something else.
    let cur: Option<Uuid> =
        sqlx::query_scalar("select mounted_on from combatants where id = $1")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .flatten();
    if cur.is_some() {
        return Err(AppError::BadRequest("already mounted — dismount first".into()));
    }

    // Rider takes the mount's position.
    let (mx, my): (Option<f32>, Option<f32>) = sqlx::query_as(
        "select token_x, token_y from combatants where id = $1",
    )
    .bind(body.mount_id)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "update combatants set mounted_on = $1, token_x = coalesce($2, token_x), token_y = coalesce($3, token_y) where id = $4",
    )
    .bind(body.mount_id)
    .bind(mx)
    .bind(my)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let c = refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        auth.campaign_id,
        json!({
            "type": "combatant_mounts",
            "rider_id": id,
            "mount_id": body.mount_id,
        }),
    )
    .await;
    Ok(Json(c))
}

pub async fn dismount(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Combatant>> {
    let auth = require_action_auth(&s.db, uid, id).await?;
    let updated: Option<Uuid> = sqlx::query_scalar(
        "update combatants set mounted_on = null where id = $1 and mounted_on is not null returning id",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?;
    if updated.is_none() {
        return Err(AppError::BadRequest("not mounted".into()));
    }
    let c = refresh_combatant(&s.db, id).await?;
    ws::publish_persist(
        &s.db,
        auth.campaign_id,
        json!({
            "type": "combatant_dismounts",
            "rider_id": id,
        }),
    )
    .await;
    Ok(Json(c))
}
