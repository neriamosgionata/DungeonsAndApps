use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    routes::notifications::emit_campaign,
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        // factions
        .route(
            "/campaigns/{id}/factions",
            get(list_factions).post(create_faction),
        )
        .route(
            "/factions/{id}",
            get(read_faction)
                .patch(update_faction)
                .delete(delete_faction),
        )
        // npcs
        .route("/campaigns/{id}/npcs", get(list_npcs).post(create_npc))
        .route(
            "/npcs/{id}",
            get(read_npc).patch(update_npc).delete(delete_npc),
        )
        // lore
        .route("/campaigns/{id}/lore", get(list_lore).post(create_lore))
        .route(
            "/lore/{id}",
            get(read_lore).patch(update_lore).delete(delete_lore),
        )
        // news
        .route("/campaigns/{id}/news", get(list_news).post(create_news))
        .route(
            "/news/{id}",
            get(read_news).patch(update_news).delete(delete_news),
        )
}

// Helper: returns true if role can see master-only content
fn can_see_all(role: Role) -> bool {
    role == Role::Master
}

// =====================================================================
// factions
// =====================================================================
#[derive(Debug, Serialize, FromRow)]
pub struct Faction {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub banner_color: Option<String>,
    pub description: Option<String>,
    pub attitude: Option<String>,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FactionCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub banner_color: Option<String>,
    pub description: Option<String>,
    pub attitude: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct FactionUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub banner_color: Option<String>,
    pub description: Option<String>,
    pub attitude: Option<String>,
    pub visibility: Option<String>,
}

async fn list_factions(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Faction>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query branches instead of format!
    let rows: Vec<Faction> = if can_see_all(role) {
        sqlx::query_as::<_, Faction>(
            "select id, campaign_id, name, banner_color, description, attitude,
                    visibility::text as visibility, updated_at
             from factions where campaign_id = $1 order by name",
        )
        .bind(cid)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query_as::<_, Faction>(
            "select id, campaign_id, name, banner_color, description, attitude,
                    visibility::text as visibility, updated_at
             from factions where campaign_id = $1 and visibility = 'players' order by name",
        )
        .bind(cid)
        .fetch_all(&s.db)
        .await?
    };
    Ok(Json(rows))
}

async fn create_faction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<FactionCreate>,
) -> AppResult<(StatusCode, Json<Faction>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("master");
    let f: Faction = sqlx::query_as::<_, Faction>(
        "insert into factions (campaign_id, name, banner_color, description, attitude, visibility)
         values ($1, $2, $3, $4, $5, $6::visibility)
         returning id, campaign_id, name, banner_color, description, attitude,
                   visibility::text as visibility, updated_at",
    )
    .bind(cid)
    .bind(&body.name)
    .bind(&body.banner_color)
    .bind(&body.description)
    .bind(&body.attitude)
    .bind(vis)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"faction_created","id":f.id}).to_string());
    Ok((StatusCode::CREATED, Json(f)))
}

async fn read_faction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Faction>> {
    let f: Faction = sqlx::query_as::<_, Faction>(
        "select id, campaign_id, name, banner_color, description, attitude,
                visibility::text as visibility, updated_at
         from factions where id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, f.campaign_id).await?;
    if role == Role::Player && f.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(f))
}

async fn update_faction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<FactionUpdate>,
) -> AppResult<Json<Faction>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from factions where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let f: Faction = sqlx::query_as::<_, Faction>(
        "update factions set
           name          = coalesce($2, name),
           banner_color  = coalesce($3, banner_color),
           description   = coalesce($4, description),
           attitude      = coalesce($5, attitude),
           visibility    = coalesce($6::visibility, visibility)
         where id = $1
         returning id, campaign_id, name, banner_color, description, attitude,
                   visibility::text as visibility, updated_at",
    )
    .bind(id)
    .bind(body.name)
    .bind(body.banner_color)
    .bind(body.description)
    .bind(body.attitude)
    .bind(body.visibility)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"faction_updated","id":id}).to_string());
    Ok(Json(f))
}

async fn delete_faction(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from factions where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from factions where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"faction_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================
// npcs
// =====================================================================
#[derive(Debug, Serialize, FromRow)]
pub struct Npc {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub role: Option<String>,
    pub faction_id: Option<Uuid>,
    pub description: Option<String>,
    pub stats: Value,
    pub image_key: Option<String>,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct NpcCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub role: Option<String>,
    pub faction_id: Option<Uuid>,
    pub description: Option<String>,
    pub stats: Option<Value>,
    pub image_key: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct NpcUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub role: Option<String>,
    pub faction_id: Option<Uuid>,
    pub description: Option<String>,
    pub stats: Option<Value>,
    pub image_key: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NpcListQ {
    pub role: Option<String>,
    pub faction_id: Option<Uuid>,
}

async fn list_npcs(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Query(q): Query<NpcListQ>,
) -> AppResult<Json<Vec<Npc>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // `role` / `faction_id` are optional filters; null binds match everything
    // via the `$N is null or col = $N` idiom (still parameterized).
    let rows: Vec<Npc> = if can_see_all(role) {
        sqlx::query_as::<_, Npc>(
            "select id, campaign_id, name, role, faction_id, description, stats, image_key,
                    visibility::text as visibility, updated_at
             from npcs
             where campaign_id = $1
               and ($2::text is null or role = $2)
               and ($3::uuid is null or faction_id = $3)
             order by name",
        )
        .bind(cid)
        .bind(&q.role)
        .bind(q.faction_id)
        .fetch_all(&s.db)
        .await?
    } else {
        sqlx::query_as::<_, Npc>(
            "select id, campaign_id, name, role, faction_id, description, stats, image_key,
                    visibility::text as visibility, updated_at
             from npcs
             where campaign_id = $1 and visibility = 'players'
               and ($2::text is null or role = $2)
               and ($3::uuid is null or faction_id = $3)
             order by name",
        )
        .bind(cid)
        .bind(&q.role)
        .bind(q.faction_id)
        .fetch_all(&s.db)
        .await?
    };
    Ok(Json(rows))
}

async fn create_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<NpcCreate>,
) -> AppResult<(StatusCode, Json<Npc>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("master");
    let n: Npc = sqlx::query_as::<_, Npc>(
        "insert into npcs (campaign_id, name, role, faction_id, description, stats, image_key, visibility)
         values ($1, $2, $3, $4, $5, coalesce($6, '{}'::jsonb), $7, $8::visibility)
         returning id, campaign_id, name, role, faction_id, description, stats, image_key,
                   visibility::text as visibility, updated_at")
        .bind(cid).bind(&body.name).bind(&body.role).bind(body.faction_id).bind(&body.description)
        .bind(body.stats).bind(&body.image_key).bind(vis).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"npc_created","id":n.id}).to_string());
    Ok((StatusCode::CREATED, Json(n)))
}

async fn read_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Npc>> {
    let n: Npc = sqlx::query_as::<_, Npc>(
        "select id, campaign_id, name, role, faction_id, description, stats, image_key,
                visibility::text as visibility, updated_at
         from npcs where id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, n.campaign_id).await?;
    if role == Role::Player && n.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(n))
}

async fn update_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<NpcUpdate>,
) -> AppResult<Json<Npc>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from npcs where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let n: Npc = sqlx::query_as::<_, Npc>(
        "update npcs set
           name        = coalesce($2, name),
           role        = coalesce($3, role),
           faction_id  = coalesce($4, faction_id),
           description = coalesce($5, description),
           stats       = coalesce($6, stats),
           image_key   = coalesce($7, image_key),
           visibility  = coalesce($8::visibility, visibility)
         where id = $1
         returning id, campaign_id, name, role, faction_id, description, stats, image_key,
                   visibility::text as visibility, updated_at",
    )
    .bind(id)
    .bind(body.name)
    .bind(body.role)
    .bind(body.faction_id)
    .bind(body.description)
    .bind(body.stats)
    .bind(body.image_key)
    .bind(body.visibility)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"npc_updated","id":id}).to_string());
    Ok(Json(n))
}

async fn delete_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from npcs where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    // Prevent deleting an NPC that is active in an encounter
    let active_count: i64 = sqlx::query_scalar(
        r#"select count(*) from combatants c
           join encounters e on e.id = c.encounter_id
           where c.npc_id = $1 and e.status in ('active','planned')"#,
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    if active_count > 0 {
        return Err(AppError::BadRequest(
            "cannot delete NPC while they are in an active or planned encounter".into(),
        ));
    }
    sqlx::query("delete from npcs where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"npc_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================
// lore
// =====================================================================
#[derive(Debug, Serialize, FromRow)]
pub struct Lore {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub title: String,
    pub category: Option<String>,
    pub body: String,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoreCreate {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    pub category: Option<String>,
    #[validate(length(min = 1))]
    pub body: String,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LoreUpdate {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,
    pub category: Option<String>,
    pub body: Option<String>,
    pub visibility: Option<String>,
}

async fn list_lore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Lore>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query branches instead of format!
    let rows: Vec<Lore> = if can_see_all(role) {
        sqlx::query_as::<_, Lore>(
            "select id, campaign_id, title, category, body, visibility::text as visibility, updated_at
             from lore_entries where campaign_id = $1 order by title")
            .bind(cid)
            .fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Lore>(
            "select id, campaign_id, title, category, body, visibility::text as visibility, updated_at
             from lore_entries where campaign_id = $1 and visibility = 'players' order by title")
            .bind(cid)
            .fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create_lore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<LoreCreate>,
) -> AppResult<(StatusCode, Json<Lore>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("master");
    let l: Lore = sqlx::query_as::<_, Lore>(
        "insert into lore_entries (campaign_id, title, category, body, visibility)
         values ($1, $2, $3, $4, $5::visibility)
         returning id, campaign_id, title, category, body, visibility::text as visibility, updated_at")
        .bind(cid).bind(&body.title).bind(&body.category).bind(&body.body).bind(vis)
        .fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"lore_created","id":l.id}).to_string());
    Ok((StatusCode::CREATED, Json(l)))
}

async fn read_lore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Lore>> {
    let l: Lore = sqlx::query_as::<_, Lore>(
        "select id, campaign_id, title, category, body, visibility::text as visibility, updated_at
         from lore_entries where id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, l.campaign_id).await?;
    if role == Role::Player && l.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(l))
}

async fn update_lore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<LoreUpdate>,
) -> AppResult<Json<Lore>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from lore_entries where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let l: Lore = sqlx::query_as::<_, Lore>(
        "update lore_entries set
           title      = coalesce($2, title),
           category   = coalesce($3, category),
           body       = coalesce($4, body),
           visibility = coalesce($5::visibility, visibility)
         where id = $1
         returning id, campaign_id, title, category, body, visibility::text as visibility, updated_at")
        .bind(id).bind(body.title).bind(body.category).bind(body.body).bind(body.visibility)
        .fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"lore_updated","id":id}).to_string());
    Ok(Json(l))
}

async fn delete_lore(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from lore_entries where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from lore_entries where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"lore_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// =====================================================================
// news
// =====================================================================
#[derive(Debug, Serialize, FromRow)]
pub struct News {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub title: String,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub published_at: OffsetDateTime,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct NewsCreate {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    #[validate(length(min = 1))]
    pub body: String,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct NewsUpdate {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,
    pub body: Option<String>,
    pub visibility: Option<String>,
}

async fn list_news(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<News>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query branches instead of format!
    let rows: Vec<News> = if can_see_all(role) {
        sqlx::query_as::<_, News>(
            "select id, campaign_id, title, body, published_at, visibility::text as visibility, created_at, updated_at
             from news_entries where campaign_id = $1 order by published_at desc")
            .bind(cid)
            .fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, News>(
            "select id, campaign_id, title, body, published_at, visibility::text as visibility, created_at, updated_at
             from news_entries where campaign_id = $1 and visibility = 'players' order by published_at desc")
            .bind(cid)
            .fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create_news(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<NewsCreate>,
) -> AppResult<(StatusCode, Json<News>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let vis = body.visibility.as_deref().unwrap_or("players");
    let n: News = sqlx::query_as::<_, News>(
        "insert into news_entries (campaign_id, title, body, visibility)
         values ($1, $2, $3, $4::visibility)
         returning id, campaign_id, title, body, published_at, visibility::text as visibility, created_at, updated_at")
        .bind(cid).bind(&body.title).bind(&body.body).bind(vis).fetch_one(&s.db).await?;
    // Don't leak title to everyone when the news is master-only. Players who
    // can see it will refetch by id via the WS-triggered list reload.
    let ws_payload = if n.visibility == "master" {
        json!({"type":"news_posted","id":n.id})
    } else {
        json!({"type":"news_posted","id":n.id,"title":n.title})
    };
    ws::publish(cid, ws_payload.to_string());
    if n.visibility != "master" {
        emit_campaign(
            &s.db,
            cid,
            Some(uid),
            "news.posted",
            &format!("News: {}", n.title),
            None,
            Some("news"),
            Some(n.id),
        )
        .await;
    }
    Ok((StatusCode::CREATED, Json(n)))
}

async fn read_news(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<News>> {
    let n: News = sqlx::query_as::<_, News>(
        "select id, campaign_id, title, body, published_at, visibility::text as visibility, created_at, updated_at
         from news_entries where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, n.campaign_id).await?;
    if role == Role::Player && n.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    Ok(Json(n))
}

async fn update_news(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<NewsUpdate>,
) -> AppResult<Json<News>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from news_entries where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let n: News = sqlx::query_as::<_, News>(
        "update news_entries set
           title      = coalesce($2, title),
           body       = coalesce($3, body),
           visibility = coalesce($4::visibility, visibility)
         where id = $1
         returning id, campaign_id, title, body, published_at, visibility::text as visibility, created_at, updated_at")
        .bind(id).bind(body.title).bind(body.body).bind(body.visibility).fetch_one(&s.db).await?;
    ws::publish(cid, json!({"type":"news_updated","id":id}).to_string());
    Ok(Json(n))
}

async fn delete_news(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from news_entries where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from news_entries where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"news_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
