use crate::{
    AppState,
    dice::{RollResult, roll},
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new().route("/campaigns/{id}/dice", get(history).post(cast).delete(clear))
}

#[derive(Debug, Deserialize, Validate)]
pub struct DiceReq {
    #[validate(length(min = 1, max = 64))]
    pub expression: String,
    pub label: Option<String>,
    pub character_id: Option<Uuid>,
    #[serde(default)]
    pub private: bool,
}

#[derive(Debug, Serialize, FromRow)]
pub struct DiceHistory {
    pub id: Uuid,
    pub user_id: Uuid,
    pub character_id: Option<Uuid>,
    pub expression: String,
    pub label: Option<String>,
    pub results: serde_json::Value,
    pub total: i32,
    pub private: bool,
    #[serde(with = "time::serde::rfc3339")]
    pub rolled_at: OffsetDateTime,
}

async fn cast(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Json(body): Json<DiceReq>,
) -> AppResult<(StatusCode, Json<serde_json::Value>)> {
    body.validate()?;
    rbac::require_member(&s.db, uid, campaign_id).await?;

    if let Some(chid) = body.character_id {
        let owner: Option<(Uuid, Uuid)> = sqlx::query_as(
            "select owner_id, campaign_id from characters where id = $1")
            .bind(chid).fetch_optional(&s.db).await?;
        let (owner_id, ch_campaign) = owner.ok_or(AppError::NotFound)?;
        if ch_campaign != campaign_id { return Err(AppError::BadRequest("character not in this campaign".into())); }
        if owner_id != uid {
            return Err(AppError::Forbidden);
        }
    }

    let mut rng = StdRng::from_os_rng();
    let result: RollResult = roll(&body.expression, &mut rng)
        .map_err(|e| AppError::BadRequest(e.to_string()))?;
    let results_json = serde_json::to_value(&result)?;

    let id: Uuid = sqlx::query_scalar(
        r#"insert into dice_rolls (campaign_id, user_id, character_id, expression, results, total, label, private)
           values ($1, $2, $3, $4, $5, $6, $7, $8)
           returning id"#,
    )
    .bind(campaign_id)
    .bind(uid)
    .bind(body.character_id)
    .bind(&body.expression)
    .bind(&results_json)
    .bind(result.total)
    .bind(&body.label)
    .bind(body.private)
    .fetch_one(&s.db)
    .await?;

    let event = json!({
        "type": "dice_roll",
        "id": id,
        "user_id": uid,
        "character_id": body.character_id,
        "expression": body.expression,
        "total": result.total,
        "label": body.label,
        "private": body.private,
        "results": results_json,
    });
    if !body.private {
        ws::publish(campaign_id, event.to_string());
    }

    Ok((StatusCode::CREATED, Json(json!({
        "id": id,
        "expression": body.expression,
        "total": result.total,
        "terms": result.terms,
    }))))
}

#[derive(Debug, Deserialize)]
pub struct HistoryQ {
    pub limit: Option<i64>,
}

async fn clear(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, campaign_id).await?;
    sqlx::query("delete from dice_rolls where campaign_id = $1")
        .bind(campaign_id)
        .execute(&s.db)
        .await?;
    ws::publish(campaign_id, json!({"type": "dice_cleared"}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

async fn history(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(campaign_id): Path<Uuid>,
    Query(q): Query<HistoryQ>,
) -> AppResult<Json<Vec<DiceHistory>>> {
    let role = rbac::require_member(&s.db, uid, campaign_id).await?;
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    let rows: Vec<DiceHistory> = if role == Role::Master {
        sqlx::query_as::<_, DiceHistory>(
            "select id, user_id, character_id, expression, label, results, total, private, rolled_at
             from dice_rolls where campaign_id = $1 order by rolled_at desc limit $2",
        )
        .bind(campaign_id)
        .bind(limit)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query_as::<_, DiceHistory>(
            "select id, user_id, character_id, expression, label, results, total, private, rolled_at
             from dice_rolls
             where campaign_id = $1 and user_id = $2
             order by rolled_at desc limit $3",
        )
        .bind(campaign_id)
        .bind(uid)
        .bind(limit)
        .fetch_all(&s.db)
        .await?
    };
    Ok(Json(rows))
}
