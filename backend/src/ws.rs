use crate::{AppState, auth::decode_jwt};
use axum::{
    extract::{
        Query, State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    response::IntoResponse,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

pub type Hub = Arc<DashMap<Uuid, broadcast::Sender<String>>>;

static HUB: Lazy<Hub> = Lazy::new(|| Arc::new(DashMap::new()));
static USER_HUB: Lazy<Hub> = Lazy::new(|| Arc::new(DashMap::new()));

/// Per-campaign presence: campaign_id → (user_id → open-socket count).
static PRESENCE: Lazy<Arc<DashMap<Uuid, DashMap<Uuid, u32>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

pub fn online_users(campaign_id: Uuid) -> Vec<Uuid> {
    PRESENCE.get(&campaign_id)
        .map(|m| m.iter().map(|e| *e.key()).collect())
        .unwrap_or_default()
}

fn presence_join(campaign_id: Uuid, user_id: Uuid) -> bool {
    let map = PRESENCE.entry(campaign_id).or_insert_with(DashMap::new).clone();
    let mut first = false;
    map.entry(user_id).and_modify(|n| *n += 1).or_insert_with(|| { first = true; 1 });
    first
}

fn presence_leave(campaign_id: Uuid, user_id: Uuid) -> bool {
    let Some(map) = PRESENCE.get(&campaign_id).map(|r| r.clone()) else { return false; };
    let mut last = false;
    let mut drop_entry = false;
    if let Some(mut n) = map.get_mut(&user_id) {
        if *n > 1 { *n -= 1; } else { drop_entry = true; last = true; }
    }
    if drop_entry { map.remove(&user_id); }
    last
}

pub fn channel(campaign_id: Uuid) -> broadcast::Sender<String> {
    HUB.entry(campaign_id)
        .or_insert_with(|| broadcast::channel::<String>(256).0)
        .clone()
}

pub fn user_channel(user_id: Uuid) -> broadcast::Sender<String> {
    USER_HUB.entry(user_id)
        .or_insert_with(|| broadcast::channel::<String>(256).0)
        .clone()
}

pub fn publish(campaign_id: Uuid, event_json: String) {
    let _ = channel(campaign_id).send(event_json);
}

pub fn publish_user(user_id: Uuid, event_json: String) {
    let _ = user_channel(user_id).send(event_json);
}

#[derive(Debug, Deserialize)]
pub struct WsQuery {
    pub token: String,
    /// Campaign channel (optional — a plain user socket skips it).
    pub campaign: Option<Uuid>,
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(q): Query<WsQuery>,
) -> impl IntoResponse {
    let claims = match decode_jwt(&q.token, &state.cfg.jwt_secret) {
        Ok(c) => c,
        Err(_) => return axum::http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let user_id = claims.sub;

    if let Some(cid) = q.campaign {
        let member = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from memberships where user_id = $1 and campaign_id = $2",
        )
        .bind(user_id)
        .bind(cid)
        .fetch_one(&state.db)
        .await.unwrap_or(0);
        // app admins override membership
        let is_admin = sqlx::query_scalar::<_, String>("select role::text from users where id = $1")
            .bind(user_id).fetch_optional(&state.db).await.ok().flatten()
            .as_deref() == Some("admin");
        if member == 0 && !is_admin {
            return axum::http::StatusCode::FORBIDDEN.into_response();
        }
    }

    ws.on_upgrade(move |socket| connection(socket, user_id, q.campaign)).into_response()
}

async fn connection(mut socket: WebSocket, user_id: Uuid, campaign: Option<Uuid>) {
    let user_tx = user_channel(user_id);
    let mut user_rx = user_tx.subscribe();

    // optional campaign subscription
    let (mut camp_rx, _camp_keep) = match campaign {
        Some(c) => {
            let tx = channel(c);
            (Some(tx.subscribe()), Some(tx))
        }
        None => (None, None),
    };

    if let Some(cid) = campaign {
        if presence_join(cid, user_id) {
            publish(cid, serde_json::json!({"type":"presence_joined","user_id":user_id}).to_string());
        }
    }

    loop {
        tokio::select! {
            msg = user_rx.recv() => match msg {
                Ok(text) => { if socket.send(Message::Text(text.into())).await.is_err() { break; } }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            msg = async { match camp_rx.as_mut() { Some(r) => r.recv().await, None => std::future::pending().await } } => match msg {
                Ok(text) => { if socket.send(Message::Text(text.into())).await.is_err() { break; } }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            },
            client = socket.recv() => match client {
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(Message::Ping(p))) => { let _ = socket.send(Message::Pong(p)).await; }
                Some(Ok(_)) => continue,
                Some(Err(_)) => break,
            }
        }
    }

    if let Some(cid) = campaign {
        if presence_leave(cid, user_id) {
            publish(cid, serde_json::json!({"type":"presence_left","user_id":user_id}).to_string());
        }
    }
}
