// Shops / merchants: GM-managed vendors, player buy/sell with coin.
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
        .route("/campaigns/{id}/shops", get(list).post(create))
        .route("/shops/{id}", get(read).patch(update).delete(delete))
        .route("/shops/{id}/items", post(add_item))
        .route("/shops/items/{item_id}", axum::routing::patch(update_item).delete(remove_item))
        .route("/shops/{id}/buy", post(buy))
        .route("/shops/{id}/sell", post(sell))
}

#[derive(Debug, Serialize, FromRow)]
pub struct Shop {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub description: String,
    pub npc_id: Option<Uuid>,
    pub visibility: String,
}

#[derive(Debug, Serialize, FromRow)]
pub struct ShopItem {
    pub id: Uuid,
    pub shop_id: Uuid,
    pub name: String,
    pub price_gp: f64,
    pub quantity: Option<i32>,
    pub item_slug: Option<String>,
}

async fn list(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let role = rbac::require_member(&s.db, uid, cid).await?;
    let shops: Vec<Shop> = if role == rbac::Role::Master {
        sqlx::query_as::<_, Shop>(
            "select id, campaign_id, name, description, npc_id, visibility::text as visibility
             from shops where campaign_id = $1 order by name")
            .bind(cid).fetch_all(&s.db).await?
    } else {
        sqlx::query_as::<_, Shop>(
            "select id, campaign_id, name, description, npc_id, visibility::text as visibility
             from shops where campaign_id = $1 and visibility = 'players' order by name")
            .bind(cid).fetch_all(&s.db).await?
    };
    let mut out = Vec::new();
    for sh in &shops {
        let items: Vec<ShopItem> = sqlx::query_as::<_, ShopItem>(
            "select id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug
             from shop_items where shop_id = $1 order by name")
            .bind(sh.id).fetch_all(&s.db).await?;
        out.push(serde_json::json!({ "shop": sh, "items": items }));
    }
    Ok(Json(serde_json::json!({ "shops": out })))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ShopCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub npc_id: Option<Uuid>,
    pub visibility: Option<String>,
}

async fn create(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(cid): Path<Uuid>,
    Json(body): Json<ShopCreate>,
) -> AppResult<(StatusCode, Json<Shop>)> {
    body.validate()?;
    rbac::require_master(&s.db, uid, cid).await?;
    let shop: Shop = sqlx::query_as::<_, Shop>(
        "insert into shops (campaign_id, name, description, npc_id, visibility)
         values ($1, $2, $3, $4, coalesce($5::visibility, 'players'))
         returning id, campaign_id, name, description, npc_id, visibility::text as visibility",
    )
    .bind(cid).bind(&body.name).bind(&body.description).bind(body.npc_id).bind(&body.visibility)
    .fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(shop)))
}

async fn read(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<serde_json::Value>> {
    let shop: Shop = sqlx::query_as::<_, Shop>(
        "select id, campaign_id, name, description, npc_id, visibility::text as visibility
         from shops where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, shop.campaign_id).await?;
    if role == rbac::Role::Player && shop.visibility != "players" {
        return Err(AppError::Forbidden);
    }
    let items: Vec<ShopItem> = sqlx::query_as::<_, ShopItem>(
        "select id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug
         from shop_items where shop_id = $1 order by name")
        .bind(id).fetch_all(&s.db).await?;
    Ok(Json(serde_json::json!({ "shop": shop, "items": items })))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ShopUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    #[validate(length(max = 2000))]
    pub description: Option<String>,
    pub npc_id: Option<Uuid>,
    pub visibility: Option<String>,
}

async fn update(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ShopUpdate>,
) -> AppResult<Json<Shop>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from shops where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let shop: Shop = sqlx::query_as::<_, Shop>(
        "update shops set
             name = coalesce($2, name),
             description = coalesce($3, description),
             npc_id = coalesce($4, npc_id),
             visibility = coalesce($5::visibility, visibility)
           where id = $1
           returning id, campaign_id, name, description, npc_id, visibility::text as visibility")
        .bind(id).bind(&body.name).bind(&body.description).bind(body.npc_id).bind(&body.visibility)
        .fetch_one(&s.db).await?;
    Ok(Json(shop))
}

async fn delete(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar("select campaign_id from shops where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from shops where id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct ItemCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    #[validate(range(min = 0.0, max = 1_000_000.0))]
    pub price_gp: f64,
    #[validate(range(min = 0, max = 1_000_000))]
    pub quantity: Option<i32>,
    #[validate(length(max = 64))]
    pub item_slug: Option<String>,
}

async fn add_item(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ItemCreate>,
) -> AppResult<(StatusCode, Json<ShopItem>)> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar("select campaign_id from shops where id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let item: ShopItem = sqlx::query_as::<_, ShopItem>(
        "insert into shop_items (shop_id, name, price_gp, quantity, item_slug)
         values ($1, $2, $3, $4, $5)
         returning id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug")
        .bind(id).bind(&body.name).bind(body.price_gp).bind(body.quantity).bind(&body.item_slug)
        .fetch_one(&s.db).await?;
    Ok((StatusCode::CREATED, Json(item)))
}

#[derive(Debug, Deserialize, Validate)]
pub struct ItemUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    #[validate(range(min = 0.0, max = 1_000_000.0))]
    pub price_gp: Option<f64>,
    #[validate(range(min = 0, max = 1_000_000))]
    pub quantity: Option<i32>,
}

async fn update_item(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<ItemUpdate>,
) -> AppResult<Json<ShopItem>> {
    body.validate()?;
    let cid: Uuid = sqlx::query_scalar(
        "select sh.campaign_id from shop_items si join shops sh on sh.id = si.shop_id where si.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    let item: ShopItem = sqlx::query_as::<_, ShopItem>(
        "update shop_items set
             name = coalesce($2, name),
             price_gp = coalesce($3, price_gp),
             quantity = coalesce($4, quantity)
           where id = $1
           returning id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug")
        .bind(id).bind(&body.name).bind(body.price_gp).bind(body.quantity)
        .fetch_one(&s.db).await?;
    Ok(Json(item))
}

async fn remove_item(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    let cid: Uuid = sqlx::query_scalar(
        "select sh.campaign_id from shop_items si join shops sh on sh.id = si.shop_id where si.id = $1")
        .bind(id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    rbac::require_master(&s.db, uid, cid).await?;
    sqlx::query("delete from shop_items where id = $1").bind(id).execute(&s.db).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Validate)]
pub struct BuyBody {
    pub character_id: Uuid,
    pub item_id: Uuid,
    #[validate(range(min = 1, max = 1000))]
    pub qty: i32,
}

async fn buy(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(body): Json<BuyBody>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    let row: Option<(Uuid,)> =
        sqlx::query_as("select campaign_id from shops where id = $1")
            .bind(shop_id).fetch_optional(&s.db).await?;
    let (cid,) = row.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, cid).await?;
    // MED (phase 3): players must not buy from (or enumerate) master-only
    // shops — `read` already gates visibility; buy didn't.
    if role != rbac::Role::Master {
        let vis: Option<String> = sqlx::query_scalar(
            "select visibility::text from shops where id = $1",
        )
        .bind(shop_id)
        .fetch_optional(&s.db)
        .await?
        .flatten();
        if vis.as_deref() != Some("players") {
            return Err(AppError::Forbidden);
        }
    }
    // Own character in the same campaign.
    let owner: Uuid = sqlx::query_scalar("select owner_id from characters where id = $1 and campaign_id = $2")
        .bind(body.character_id).bind(cid).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    if owner != uid && rbac::require_member(&s.db, uid, cid).await? != rbac::Role::Master {
        return Err(AppError::Forbidden);
    }
    let item: ShopItem = sqlx::query_as::<_, ShopItem>(
        "select id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug
         from shop_items where id = $1 and shop_id = $2")
        .bind(body.item_id).bind(shop_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let total = (item.price_gp * body.qty as f64).round() as i64;
    if total <= 0 {
        return Err(AppError::BadRequest("invalid price".into()));
    }
    // Available stock?
    if let Some(q) = item.quantity {
        if q < body.qty {
            return Err(AppError::BadRequest(format!("only {q} in stock")));
        }
    }
    let mut tx = s.db.begin().await?;
    sqlx::query("select id from characters where id = $1 for update")
        .bind(body.character_id).fetch_optional(&mut *tx).await?.ok_or(AppError::NotFound)?;
    let gp: i64 = sqlx::query_scalar(
        "select coalesce((sheet->'coin'->>'gp')::int, 0) from characters where id = $1")
        .bind(body.character_id).fetch_one(&mut *tx).await?;
    if gp < total {
        return Err(AppError::BadRequest(format!("not enough gold ({gp} gp < {total} gp)")));
    }
    // Deduct coin, add equipment row.
    sqlx::query(
        "update characters set sheet = jsonb_set(
           sheet, '{coin,gp}', to_jsonb($2::int))
         where id = $1")
        .bind(body.character_id).bind(gp - total).execute(&mut *tx).await?;
    sqlx::query(
        r#"update characters set sheet = jsonb_set(
             sheet, '{equipment}',
             coalesce(sheet->'equipment', '[]'::jsonb) || jsonb_build_array(
               jsonb_build_object('id', gen_random_uuid(), 'name', $2, 'qty', $3, 'equipped', false)))"#)
        .bind(body.character_id).bind(&item.name).bind(body.qty).execute(&mut *tx).await?;
    if let Some(q) = item.quantity {
        // atomic decrement — two concurrent buyers can't oversell the last unit
        let ok: Option<Uuid> = sqlx::query_scalar(
            "update shop_items set quantity = quantity - $2
             where id = $1 and quantity >= $2 returning id",
        )
        .bind(item.id)
        .bind(body.qty)
        .fetch_optional(&mut *tx)
        .await?;
        if ok.is_none() {
            return Err(AppError::BadRequest(format!(
                "only {q} in stock (sold out while buying)"
            )));
        }
    }
    tx.commit().await?;
    crate::ws::publish(cid, serde_json::json!({"type":"character_updated","id":body.character_id}).to_string());
    Ok(Json(serde_json::json!({ "item": item.name, "qty": body.qty, "cost_gp": total, "gp_remaining": gp - total })))
}

#[derive(Debug, Deserialize, Validate)]
pub struct SellBody {
    pub character_id: Uuid,
    pub item_id: Uuid,
    pub shop_id: Uuid,
    #[validate(range(min = 1, max = 1000))]
    pub qty: i32,
}

async fn sell(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Path(shop_id): Path<Uuid>,
    Json(body): Json<SellBody>,
) -> AppResult<Json<serde_json::Value>> {
    body.validate()?;
    // 2nd-pass: use the PATH shop id (the body field was dead + could
    // disagree with the URL) and gate master-only shops like buy.
    let row: Option<(Uuid,)> = sqlx::query_as("select campaign_id from shops where id = $1")
        .bind(shop_id).fetch_optional(&s.db).await?;
    let (cid,) = row.ok_or(AppError::NotFound)?;
    let role = rbac::require_member(&s.db, uid, cid).await?;
    let owner: Uuid = sqlx::query_scalar("select owner_id from characters where id = $1 and campaign_id = $2")
        .bind(body.character_id).bind(cid).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    if owner != uid && rbac::require_member(&s.db, uid, cid).await? != rbac::Role::Master {
        return Err(AppError::Forbidden);
    }
    // Only items the shop sells can be sold back (at 50%).
    let item: ShopItem = sqlx::query_as::<_, ShopItem>(
        "select id, shop_id, name, price_gp::float8 as price_gp, quantity, item_slug
         from shop_items where id = $1 and shop_id = $2")
        .bind(body.item_id).bind(body.shop_id).fetch_optional(&s.db).await?.ok_or(AppError::NotFound)?;
    let unit = (item.price_gp * 0.5).round() as i64;
    let total = unit * body.qty as i64;
    let mut tx = s.db.begin().await?;
    // Remove from character equipment by name (qty decrement).
    let eq: serde_json::Value = sqlx::query_scalar(
        "select coalesce(sheet->'equipment', '[]'::jsonb) from characters where id = $1")
        .bind(body.character_id).fetch_one(&mut *tx).await?;
    let mut removed = 0i64;
    let mut kept: Vec<serde_json::Value> = Vec::new();
    for e in eq.as_array().cloned().unwrap_or_default() {
        let name = e.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name == item.name && removed < body.qty as i64 {
            removed += 1;
            continue;
        }
        kept.push(e);
    }
    if removed < body.qty as i64 {
        return Err(AppError::BadRequest(format!("only {removed} of '{}' in inventory", item.name)));
    }
    let gp: i64 = sqlx::query_scalar(
        "select coalesce((sheet->'coin'->>'gp')::int, 0) from characters where id = $1")
        .bind(body.character_id).fetch_one(&mut *tx).await?;
    sqlx::query(
        "update characters set sheet =
           jsonb_set(jsonb_set(sheet, '{coin,gp}', to_jsonb($2::int)), '{equipment}', $3::jsonb)
         where id = $1")
        .bind(body.character_id).bind(gp + total).bind(serde_json::json!(kept)).execute(&mut *tx).await?;
    if let Some(q) = item.quantity {
        sqlx::query("update shop_items set quantity = $2 where id = $1")
            .bind(item.id).bind(q + body.qty).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    crate::ws::publish(cid, serde_json::json!({"type":"character_updated","id":body.character_id}).to_string());
    Ok(Json(serde_json::json!({ "item": item.name, "qty": removed, "gold": total, "gp_after": gp + total })))
}
