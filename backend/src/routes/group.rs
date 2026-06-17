use crate::{
    AppState,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac::{self, Role},
    ws,
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/campaigns/{id}/party", get(read_party).patch(update_party))
        .route("/campaigns/{id}/loot", get(list_loot).post(create_loot))
        .route(
            "/loot/{id}",
            axum::routing::patch(update_loot).delete(delete_loot),
        )
        .route(
            "/campaigns/{id}/quests",
            get(list_quests).post(create_quest),
        )
        .route(
            "/quests/{id}",
            get(read_quest).patch(update_quest).delete(delete_quest),
        )
        .route("/quests/{id}/npcs", post(link_npc))
        .route(
            "/quests/{id}/npcs/{npc_id}",
            axum::routing::delete(unlink_npc),
        )
}

// ============ party (coin + notes) ============
#[derive(Debug, Serialize, FromRow)]
pub struct Party {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub cp: i64,
    pub sp: i64,
    pub ep: i64,
    pub gp: i64,
    pub pp: i64,
    pub shared_notes: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct PartyUpdate {
    pub cp: Option<i64>,
    pub sp: Option<i64>,
    pub ep: Option<i64>,
    pub gp: Option<i64>,
    pub pp: Option<i64>,
    pub shared_notes: Option<String>,
}

async fn read_party(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Party>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let p: Party = sqlx::query_as::<_, Party>(
        "select id, campaign_id, cp, sp, ep, gp, pp, shared_notes, updated_at
         from parties where campaign_id = $1",
    )
    .bind(cid)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    Ok(Json(p))
}

async fn update_party(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<PartyUpdate>,
) -> AppResult<Json<Party>> {
    rbac::require_master(&s.db, uid, cid).await?;
    let p: Party = sqlx::query_as::<_, Party>(
        "update parties set
           cp = coalesce($2, cp),
           sp = coalesce($3, sp),
           ep = coalesce($4, ep),
           gp = coalesce($5, gp),
           pp = coalesce($6, pp),
           shared_notes = coalesce($7, shared_notes),
           updated_at = now()
         where campaign_id = $1
         returning id, campaign_id, cp, sp, ep, gp, pp, shared_notes, updated_at",
    )
    .bind(cid)
    .bind(body.cp)
    .bind(body.sp)
    .bind(body.ep)
    .bind(body.gp)
    .bind(body.pp)
    .bind(body.shared_notes)
    .fetch_one(&s.db)
    .await?;
    ws::publish(
        cid,
        json!({"type":"party_updated","cp":p.cp,"sp":p.sp,"ep":p.ep,"gp":p.gp,"pp":p.pp})
            .to_string(),
    );
    Ok(Json(p))
}

// ============ loot ============
#[derive(Debug, Serialize, FromRow)]
pub struct Loot {
    pub id: Uuid,
    pub party_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub quantity: i32,
    #[serde(serialize_with = "ser_bd")]
    pub value_gp: Option<sqlx::types::BigDecimal>,
    pub claimed_by: Option<Uuid>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

fn ser_bd<S: serde::Serializer>(
    v: &Option<sqlx::types::BigDecimal>,
    s: S,
) -> Result<S::Ok, S::Error> {
    match v {
        Some(d) => s.serialize_str(&d.to_string()),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Deserialize, Validate)]
pub struct LootCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 1_000_000))]
    pub quantity: Option<i32>,
    pub value_gp: Option<f64>,
    pub claimed_by: Option<Uuid>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct LootUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub description: Option<String>,
    #[validate(range(min = 0, max = 1_000_000))]
    pub quantity: Option<i32>,
    pub value_gp: Option<f64>,
    pub claimed_by: Option<Uuid>,
}

async fn list_loot(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Loot>>> {
    rbac::require_member(&s.db, uid, cid).await?;
    let rows: Vec<Loot> = sqlx::query_as::<_, Loot>(
        "select l.id, l.party_id, l.name, l.description, l.quantity, l.value_gp, l.claimed_by, l.created_at
         from loot_items l join parties p on p.id = l.party_id
         where p.campaign_id = $1 order by l.created_at desc")
        .bind(cid).fetch_all(&s.db).await?;
    Ok(Json(rows))
}

async fn create_loot(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<LootCreate>,
) -> AppResult<(StatusCode, Json<Loot>)> {
    body.validate()?;
    rbac::require_member(&s.db, uid, cid).await?;
    let party_id: Uuid = sqlx::query_scalar("select id from parties where campaign_id = $1")
        .bind(cid)
        .fetch_one(&s.db)
        .await?;
    let value = body
        .value_gp
        .map(|v| sqlx::types::BigDecimal::try_from(v).unwrap_or_default());
    let l: Loot = sqlx::query_as::<_, Loot>(
        "insert into loot_items (party_id, name, description, quantity, value_gp, claimed_by)
         values ($1, $2, $3, coalesce($4, 1), $5, $6)
         returning id, party_id, name, description, quantity, value_gp, claimed_by, created_at",
    )
    .bind(party_id)
    .bind(&body.name)
    .bind(&body.description)
    .bind(body.quantity)
    .bind(value)
    .bind(body.claimed_by)
    .fetch_one(&s.db)
    .await?;
    ws::publish(
        cid,
        json!({"type":"loot_added","id":l.id,"name":l.name}).to_string(),
    );
    Ok((StatusCode::CREATED, Json(l)))
}

async fn update_loot(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<LootUpdate>,
) -> AppResult<Json<Loot>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar(
        "select p.campaign_id from loot_items l join parties p on p.id = l.party_id where l.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, cid).await?;
    let value = body
        .value_gp
        .map(|v| sqlx::types::BigDecimal::try_from(v).unwrap_or_default());
    let l: Loot = sqlx::query_as::<_, Loot>(
        "update loot_items set
           name        = coalesce($2, name),
           description = coalesce($3, description),
           quantity    = coalesce($4, quantity),
           value_gp    = coalesce($5, value_gp),
           claimed_by  = coalesce($6, claimed_by)
         where id = $1
         returning id, party_id, name, description, quantity, value_gp, claimed_by, created_at",
    )
    .bind(id)
    .bind(body.name)
    .bind(body.description)
    .bind(body.quantity)
    .bind(value)
    .bind(body.claimed_by)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"loot_updated","id":l.id}).to_string());
    Ok(Json(l))
}

async fn delete_loot(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar(
        "select p.campaign_id from loot_items l join parties p on p.id = l.party_id where l.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_member(&s.db, uid, cid).await?;
    sqlx::query("delete from loot_items where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"loot_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}

// ============ quests ============
#[derive(Debug, Serialize, FromRow)]
pub struct Quest {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub reward: Option<String>,
    pub visibility: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct QuestCreate {
    #[validate(length(min = 1, max = 200))]
    pub title: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub reward: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct QuestUpdate {
    #[validate(length(min = 1, max = 200))]
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub reward: Option<String>,
    pub visibility: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct QuestNpcEntry {
    pub npc_id: Uuid,
    pub name: String,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct QuestDetail {
    #[serde(flatten)]
    pub quest: Quest,
    pub npcs: Vec<QuestNpcEntry>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct QuestNpcCreate {
    pub npc_id: Uuid,
    pub role: Option<String>,
}

async fn list_quests(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<Vec<Quest>>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // Fix: Use parameterized query branches instead of format!
    let rows: Vec<Quest> = if role == Role::Master {
        sqlx::query_as::<_, Quest>(
            "select id, campaign_id, title, description, status::text as status, reward,
                    visibility::text as visibility, updated_at
             from quests where campaign_id = $1 order by
               case status::text when 'active' then 0 when 'completed' then 1 else 2 end, updated_at desc")
            .bind(cid)
            .fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Quest>(
            "select id, campaign_id, title, description, status::text as status, reward,
                    visibility::text as visibility, updated_at
             from quests where campaign_id = $1 and visibility = 'players' order by
               case status::text when 'active' then 0 when 'completed' then 1 else 2 end, updated_at desc")
            .bind(cid)
            .fetch_all(&s.db).await?
    };
    Ok(Json(rows))
}

async fn create_quest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<QuestCreate>,
) -> AppResult<(StatusCode, Json<Quest>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let status = body.status.as_deref().unwrap_or("active");
    let vis = body.visibility.as_deref().unwrap_or("players");
    let q: Quest = sqlx::query_as::<_, Quest>(
        "insert into quests (campaign_id, title, description, status, reward, visibility)
         values ($1, $2, $3, $4::quest_status, $5, $6::visibility)
         returning id, campaign_id, title, description, status::text as status, reward,
                   visibility::text as visibility, updated_at",
    )
    .bind(cid)
    .bind(&body.title)
    .bind(&body.description)
    .bind(status)
    .bind(&body.reward)
    .bind(vis)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"quest_created","id":q.id}).to_string());
    Ok((StatusCode::CREATED, Json(q)))
}

async fn read_quest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<QuestDetail>> {
    let q: Quest = sqlx::query_as::<_, Quest>(
        "select id, campaign_id, title, description, status::text as status, reward,
                visibility::text as visibility, updated_at from quests where id = $1",
    )
    .bind(id)
    .fetch_optional(&s.db)
    .await?
    .ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, q.campaign_id).await?;
    if role == Role::Player && q.visibility == "master" {
        return Err(AppError::Forbidden);
    }
    let npcs: Vec<QuestNpcEntry> = sqlx::query_as::<_, QuestNpcEntry>(
        "select q.npc_id, n.name, q.role
         from quest_npcs q join npcs n on n.id = q.npc_id
         where q.quest_id = $1",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(QuestDetail { quest: q, npcs }))
}

async fn update_quest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<QuestUpdate>,
) -> AppResult<Json<Quest>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from quests where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let q: Quest = sqlx::query_as::<_, Quest>(
        "update quests set
           title       = coalesce($2, title),
           description = coalesce($3, description),
           status      = coalesce($4::quest_status, status),
           reward      = coalesce($5, reward),
           visibility  = coalesce($6::visibility, visibility)
         where id = $1
         returning id, campaign_id, title, description, status::text as status, reward,
                   visibility::text as visibility, updated_at",
    )
    .bind(id)
    .bind(body.title)
    .bind(body.description)
    .bind(body.status)
    .bind(body.reward)
    .bind(body.visibility)
    .fetch_one(&s.db)
    .await?;
    ws::publish(cid, json!({"type":"quest_updated","id":id}).to_string());
    Ok(Json(q))
}

async fn link_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<QuestNpcCreate>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from quests where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let res = sqlx::query(
        "insert into quest_npcs (quest_id, npc_id, role) values ($1, $2, $3) on conflict do nothing")
        .bind(id).bind(body.npc_id).bind(body.role)
        .execute(&s.db).await?;
    if res.rows_affected() == 0 {
        return Err(AppError::Conflict("npc already linked to quest".into()));
    }
    ws::publish(
        cid,
        json!({"type":"quest_npc_linked","quest_id":id,"npc_id":body.npc_id}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn unlink_npc(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path((id, npc_id)): Path<(Uuid, Uuid)>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from quests where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from quest_npcs where quest_id = $1 and npc_id = $2")
        .bind(id)
        .bind(npc_id)
        .execute(&s.db)
        .await?;
    ws::publish(
        cid,
        json!({"type":"quest_npc_unlinked","quest_id":id,"npc_id":npc_id}).to_string(),
    );
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_quest(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from quests where id = $1")
        .bind(id)
        .fetch_optional(&s.db)
        .await?
        .ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from quests where id = $1")
        .bind(id)
        .execute(&s.db)
        .await?;
    ws::publish(cid, json!({"type":"quest_deleted","id":id}).to_string());
    Ok(StatusCode::NO_CONTENT)
}
