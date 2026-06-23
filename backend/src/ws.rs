use crate::{AppState, auth::decode_jwt};
use axum::{
    extract::{
        State, WebSocketUpgrade,
        ws::{Message, WebSocket},
    },
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tokio::time::interval;
use uuid::Uuid;

pub type Hub = Arc<DashMap<Uuid, broadcast::Sender<String>>>;

// Last access time tracking for cleanup
static HUB_LAST_ACCESS: Lazy<Arc<DashMap<Uuid, Instant>>> = Lazy::new(|| Arc::new(DashMap::new()));
static USER_HUB_LAST_ACCESS: Lazy<Arc<DashMap<Uuid, Instant>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

static HUB: Lazy<Hub> = Lazy::new(|| Arc::new(DashMap::new()));
static USER_HUB: Lazy<Hub> = Lazy::new(|| Arc::new(DashMap::new()));

/// Per-campaign presence: campaign_id → (user_id → open-socket count).
static PRESENCE: Lazy<Arc<DashMap<Uuid, DashMap<Uuid, u32>>>> =
    Lazy::new(|| Arc::new(DashMap::new()));

/// Stale channel cleanup threshold (1 hour of inactivity)
const CHANNEL_TTL: Duration = Duration::from_secs(3600);

/// Start background cleanup task. Call once at app startup.
pub fn start_cleanup_task() {
    tokio::spawn(async {
        let mut ticker = interval(Duration::from_secs(300)); // Every 5 minutes
        loop {
            ticker.tick().await;
            cleanup_stale_channels();
        }
    });
}

fn cleanup_stale_channels() {
    let now = Instant::now();
    let hub_stale: Vec<Uuid> = HUB_LAST_ACCESS
        .iter()
        .filter(|e| now.duration_since(*e.value()) > CHANNEL_TTL)
        .map(|e| *e.key())
        .collect();
    for id in hub_stale {
        HUB.remove(&id);
        HUB_LAST_ACCESS.remove(&id);
    }
    let user_hub_stale: Vec<Uuid> = USER_HUB_LAST_ACCESS
        .iter()
        .filter(|e| now.duration_since(*e.value()) > CHANNEL_TTL)
        .map(|e| *e.key())
        .collect();
    for id in user_hub_stale {
        USER_HUB.remove(&id);
        USER_HUB_LAST_ACCESS.remove(&id);
    }
}

pub fn online_users(campaign_id: Uuid) -> Vec<Uuid> {
    PRESENCE
        .get(&campaign_id)
        .map(|m| m.iter().map(|e| *e.key()).collect())
        .unwrap_or_default()
}

fn presence_join(campaign_id: Uuid, user_id: Uuid) -> bool {
    let mut first = false;
    PRESENCE
        .entry(campaign_id)
        .or_insert_with(DashMap::new)
        .entry(user_id)
        .and_modify(|n| *n += 1)
        .or_insert_with(|| {
            first = true;
            1
        });
    first
}

fn presence_leave(campaign_id: Uuid, user_id: Uuid) -> bool {
    let mut last = false;
    if let Some(map) = PRESENCE.get_mut(&campaign_id) {
        let mut drop_entry = false;
        if let Some(mut n) = map.get_mut(&user_id) {
            if *n > 1 {
                *n -= 1;
            } else {
                drop_entry = true;
                last = true;
            }
        }
        if drop_entry {
            map.remove(&user_id);
        }
    }
    last
}

pub fn channel(campaign_id: Uuid) -> broadcast::Sender<String> {
    HUB_LAST_ACCESS.insert(campaign_id, Instant::now());
    HUB.entry(campaign_id)
        .or_insert_with(|| broadcast::channel::<String>(256).0)
        .clone()
}

pub fn user_channel(user_id: Uuid) -> broadcast::Sender<String> {
    USER_HUB_LAST_ACCESS.insert(user_id, Instant::now());
    USER_HUB
        .entry(user_id)
        .or_insert_with(|| broadcast::channel::<String>(256).0)
        .clone()
}

pub fn publish(campaign_id: Uuid, event_json: String) {
    let _ = channel(campaign_id).send(event_json);
}

pub fn publish_user(user_id: Uuid, event_json: String) {
    let _ = user_channel(user_id).send(event_json);
}

/// M-F6 part 2: persist event to ws_events table + broadcast.
/// The seq field is added to the event_json before broadcast so clients
/// can track their last-received seq and request missed events on reconnect.
/// Returns the assigned seq, or None if the persist failed (the broadcast
/// still happens either way — replay is best-effort).
pub async fn publish_persist(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    event_json: serde_json::Value,
) -> Option<i64> {
    // Extract the type field for the dedicated column. Falls back to "unknown"
    // for malformed payloads (shouldn't happen — all callers set "type").
    let ty = event_json.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    // INSERT (campaign_id, type, payload). The BEFORE INSERT trigger
    // populates seq from ws_events_seq_per_campaign (per-campaign monotonic).
    let row: Result<Option<(i64,)>, sqlx::Error> = sqlx::query_as(
        "INSERT INTO ws_events (campaign_id, type, payload) VALUES ($1, $2, $3) RETURNING seq"
    )
    .bind(campaign_id)
    .bind(&ty)
    .bind(&event_json)
    .fetch_optional(db)
    .await;
    let seq = match row {
        Ok(Some((s,))) => s,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!(campaign_id = %campaign_id, "ws_events persist failed: {e}");
            return None;
        }
    };
    // Augment event_json with seq, then broadcast. We mutate the JSON to
    // avoid re-serializing the whole payload.
    let mut augmented = event_json;
    if let Some(obj) = augmented.as_object_mut() {
        obj.insert("seq".to_string(), serde_json::json!(seq));
    }
    publish(campaign_id, augmented.to_string());
    Some(seq)
}

/// Replay events for a campaign with seq > `since`, ordered by seq ASC.
/// Used by the client on WS reconnect to catch up on missed events.
/// Default cap 500 events (longer than any realistic disconnect window).
pub async fn replay_events(
    db: &sqlx::PgPool,
    campaign_id: Uuid,
    since: i64,
    limit: i64,
) -> Result<Vec<serde_json::Value>, sqlx::Error> {
    let limit = limit.clamp(1, 1000);
    let rows: Vec<(serde_json::Value,)> = sqlx::query_as(
        "SELECT payload FROM ws_events
         WHERE campaign_id = $1 AND seq > $2
         ORDER BY seq ASC
         LIMIT $3"
    )
    .bind(campaign_id)
    .bind(since)
    .bind(limit)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(|(p,)| p).collect())
}

/// Extract JWT token from Sec-WebSocket-Protocol header using subprotocol auth.
/// Format: `Authorization.bearer.<base64url_token>` or `auth.<base64url_token>`
fn extract_token_from_headers(headers: &HeaderMap) -> Option<String> {
    let proto = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())?;
    for p in proto.split(',').map(|s| s.trim()) {
        if let Some(token) = p.strip_prefix("auth.") {
            return Some(token.to_string());
        }
        if let Some(token_part) = p.strip_prefix("Authorization.bearer.") {
            return Some(token_part.to_string());
        }
    }
    None
}

fn extract_campaign_from_headers(headers: &HeaderMap) -> Option<Uuid> {
    // 1. Explicit x-campaign-id header
    if let Some(cid) = headers
        .get("x-campaign-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok())
    {
        return Some(cid);
    }
    // 2. campaign.<uuid> subprotocol (browser WS API cannot set custom headers)
    let proto = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())?;
    for p in proto.split(',').map(|s| s.trim()) {
        if let Some(cid_str) = p.strip_prefix("campaign.") {
            if let Ok(cid) = cid_str.parse::<Uuid>() {
                return Some(cid);
            }
        }
    }
    None
}

/// WebSocket connection rate limiting: user_id → (count, first_attempt_time)
static WS_CONNECT_RATE: Lazy<Arc<DashMap<Uuid, (u32, Instant)>>> =
    Lazy::new(|| Arc::new(DashMap::new()));
const WS_MAX_CONNECTS_PER_MINUTE: u32 = 60;
const WS_RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

fn check_ws_rate_limit(user_id: Uuid) -> bool {
    let now = Instant::now();
    let mut allowed = true;

    WS_CONNECT_RATE
        .entry(user_id)
        .and_modify(|(count, first)| {
            if now.duration_since(*first) > WS_RATE_LIMIT_WINDOW {
                // Reset window
                *count = 1;
                *first = now;
            } else if *count >= WS_MAX_CONNECTS_PER_MINUTE {
                allowed = false;
            } else {
                *count += 1;
            }
        })
        .or_insert((1, now));

    // Cleanup old entries periodically (simple approach: 1% chance per check)
    use rand::Rng;
    if rand::rng().random_range(0..100) < 1 {
        let stale_keys: Vec<Uuid> = WS_CONNECT_RATE
            .iter()
            .filter(|e| now.duration_since(e.value().1) > WS_RATE_LIMIT_WINDOW * 2)
            .map(|e| *e.key())
            .collect();
        for key in stale_keys {
            WS_CONNECT_RATE.remove(&key);
        }
    }

    allowed
}

pub async fn handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    headers: HeaderMap,
) -> impl IntoResponse {
    // Support both new header-based auth and legacy query param (for backward compat)
    let token = match extract_token_from_headers(&headers) {
        Some(t) => t,
        None => {
            // Legacy: check query param (still allow for migration period)
            // Log deprecation warning since token in URL is a security risk
            tracing::warn!(
                "WebSocket connection without Sec-WebSocket-Protocol header - token may be exposed in logs/URL"
            );
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let claims = match decode_jwt(&token, &state.cfg.jwt_secret) {
        Ok(c) => c,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };
    let user_id = claims.sub;

    // Verify user still exists and token version matches (logout / password-change invalidation)
    let row: Option<(Uuid, i32)> =
        match sqlx::query_as("select id, token_version from users where id = $1")
            .bind(user_id)
            .fetch_optional(&state.db)
            .await
        {
            Ok(r) => r,
            Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
        };
    let (db_id, tv) = match row {
        Some(r) => r,
        None => return StatusCode::UNAUTHORIZED.into_response(),
    };
    if tv != claims.tv {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let user_id = db_id;

    if !check_ws_rate_limit(user_id) {
        tracing::warn!(%user_id, "WebSocket connection rate limit exceeded");
        return StatusCode::TOO_MANY_REQUESTS.into_response();
    }

    let campaign: Option<Uuid> = extract_campaign_from_headers(&headers);

    if let Some(cid) = campaign {
        let member = sqlx::query_scalar::<_, i64>(
            "select count(*)::bigint from memberships where user_id = $1 and campaign_id = $2",
        )
        .bind(user_id)
        .bind(cid)
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
        // app admins override membership
        let is_admin =
            sqlx::query_scalar::<_, String>("select role::text from users where id = $1")
                .bind(user_id)
                .fetch_optional(&state.db)
                .await
                .ok()
                .flatten()
                .as_deref()
                == Some("admin");
        if member == 0 && !is_admin {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    // Echo back subprotocols so the browser accepts the WS handshake (RFC 6455)
    let protos: Vec<String> = headers
        .get("sec-websocket-protocol")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(',').map(|p| p.trim().to_string()).collect())
        .unwrap_or_default();

    ws.protocols(protos)
        .on_upgrade(move |socket| connection(socket, user_id, claims.tv, campaign, state.db.clone()))
        .into_response()
}

async fn connection(
    mut socket: WebSocket,
    user_id: Uuid,
    claims_tv: i32,
    campaign: Option<Uuid>,
    db: sqlx::PgPool,
) {
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
            publish(
                cid,
                serde_json::json!({"type":"presence_joined","user_id":user_id}).to_string(),
            );
        }
    }

    // F4: mid-session token_version re-check. The handshake validates
    // `claims.tv == users.token_version` but logout / password-change only
    // bumps the DB row, not the JWT. Without re-checking, an open socket keeps
    // receiving events post-revocation. 30s interval is a balance between
    // promptness and DB load.
    let mut revocation_check = tokio::time::interval(Duration::from_secs(30));
    revocation_check.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    // First tick fires immediately — skip it (the handshake already validated).
    revocation_check.tick().await;

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
            },
            _ = revocation_check.tick() => {
                match sqlx::query_scalar::<_, i32>(
                    "select token_version from users where id = $1")
                    .bind(user_id)
                    .fetch_optional(&db)
                    .await
                {
                    Ok(Some(tv)) if tv != claims_tv => {
                        tracing::info!(%user_id, "WS connection revoked (token_version mismatch)");
                        break;
                    }
                    Ok(_) => {} // match: keep connection alive
                    Err(e) => tracing::warn!(%user_id, "WS revocation check failed: {e}"),
                }
            }
        }
    }

    if let Some(cid) = campaign {
        if presence_leave(cid, user_id) {
            publish(
                cid,
                serde_json::json!({"type":"presence_left","user_id":user_id}).to_string(),
            );
        }
    }
}
