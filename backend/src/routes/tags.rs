// Tags: campaign-scoped labels for NPCs / lore / news (and other resources).
use crate::{AppState, error::{AppError, AppResult}, extract::AuthUser, rbac};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/tags", get(list_tags).post(create_tag))
        .route("/campaigns/{id}/tags/{tag_id}", axum::routing::delete(delete_tag))
        .route("/campaigns/{id}/tags/apply", post(apply_tag))
        .route(
            "/campaigns/{id}/tags/{tag_id}/resources/{resource_type}/{resource_id}",
            axum::routing::delete(remove_tag),
        )
}

#[derive(Debug, Serialize, FromRow)]
pub struct Tag {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Deserialize, Validate)]
pub struct TagCreate {
    #[validate(length(min = 1, max = 40))]
    pub name: String,
    #[validate(length(max = 16))]
    pub color: Option<String>,
}

async fn list_tags(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Query(q): Query<ListTagsQ>,
) -> AppResult<Json<serde_json::Value>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let tags: Vec<Tag> = sqlx::query_as::<_, Tag>(
        "select id, campaign_id, name, color from tags where campaign_id = $1 order by name",
    )
    .bind(cid)
    .fetch_all(&s.db)
    .await?;
    // Optional resource-scoped lookup: which tags are on resource X?
    let resource_tags = if let (Some(rt), Some(rid)) = (&q.resource_type, &q.resource_id) {
        let rid = rid.parse::<Uuid>().ok();
        let rows: Vec<Tag> = sqlx::query_as::<_, Tag>(
            "select t.id, t.campaign_id, t.name, t.color
             from tags t join taggings tg on tg.tag_id = t.id
             where t.campaign_id = $1 and tg.resource_type = $2 and tg.resource_id = $3",
        )
        .bind(cid)
        .bind(rt)
        .bind(rid)
        .fetch_all(&s.db)
        .await?;
        rows
    } else {
        Vec::new()
    };
    Ok(Json(serde_json::json!({ "tags": tags, "resource_tags": resource_tags })))
}

#[derive(Debug, Deserialize)]
pub struct ListTagsQ {
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
}

async fn create_tag(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<TagCreate>,
) -> AppResult<(StatusCode, Json<Tag>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let tag: Tag = sqlx::query_as::<_, Tag>(
        "insert into tags (campaign_id, name, color) values ($1, $2, $3)
         on conflict (campaign_id, name) do update set color = excluded.color
         returning id, campaign_id, name, color",
    )
    .bind(cid)
    .bind(&body.name)
    .bind(body.color.unwrap_or_else(|| "#8b6914".into()))
    .fetch_one(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(tag)))
}

async fn delete_tag(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, tag_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, cid).await?;
    let res = sqlx::query("delete from tags where id = $1 and campaign_id = $2")
        .bind(tag_id)
        .bind(cid)
        .execute(&s.db)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct ApplyTag {
    #[validate(length(min = 1, max = 40))]
    pub resource_type: String,
    pub resource_id: Uuid,
}

async fn apply_tag(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, tag_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<ApplyTag>,
) -> AppResult<StatusCode> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    // 2nd-pass: tag must belong to THIS campaign (cross-campaign tags
    // created dangling rows) and the resource_type is whitelisted.
    let tag_ok: Option<Uuid> = sqlx::query_scalar(
        "select id from tags where id = $1 and campaign_id = $2",
    )
    .bind(tag_id)
    .bind(cid)
    .fetch_optional(&s.db)
    .await?;
    if tag_ok.is_none() {
        return Err(AppError::BadRequest("tag not found in this campaign".into()));
    }
    const RESOURCE_TYPES: &[&str] = &["npc", "lore", "news", "quest", "map"];
    if !RESOURCE_TYPES.contains(&body.resource_type.as_str()) {
        return Err(AppError::BadRequest(format!(
            "resource_type must be one of {RESOURCE_TYPES:?}"
        )));
    }
    sqlx::query(
        "insert into taggings (tag_id, resource_type, resource_id) values ($1, $2, $3) on conflict do nothing",
    )
    .bind(tag_id)
    .bind(&body.resource_type)
    .bind(body.resource_id)
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_tag(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((cid, tag_id, resource_type, resource_id)): Path<(Uuid, Uuid, String, Uuid)>,
) -> AppResult<StatusCode> {
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query(
        "delete from taggings where tag_id = $1 and resource_type = $2 and resource_id = $3",
    )
    .bind(tag_id)
    .bind(&resource_type)
    .bind(resource_id)
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
