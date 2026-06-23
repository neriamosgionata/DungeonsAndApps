// M-F6 part 2: WS event replay endpoint.
// GET /api/v1/ws-events?campaign_id=X&since=<seq>
// Returns events with seq > since, ordered by seq ASC, capped at limit.
// The client calls this on WS reconnect (and on initial page load) to
// catch up on events that arrived during the disconnect window.
use crate::{
    AppState,
    error::AppResult,
    extract::AuthUser,
    rbac, ws,
};
use axum::{
    Json, Router,
    extract::{Query, State},
    routing::get,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ReplayQuery {
    pub campaign_id: Uuid,
    /// Last seq the client received. Replay returns events with seq > since.
    #[serde(default)]
    pub since: i64,
    /// Max events to return. Clamped to 1..=1000 server-side. Default 500.
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 { 500 }

#[derive(Debug, serde::Serialize)]
pub struct ReplayResponse {
    pub events: Vec<serde_json::Value>,
    /// The highest seq included in this response. Client uses this to
    /// advance its lastSeq. None if no events were returned.
    pub max_seq: Option<i64>,
}

pub async fn replay(
    State(s): State<AppState>,
    AuthUser(uid): AuthUser,
    Query(q): Query<ReplayQuery>,
) -> AppResult<Json<ReplayResponse>> {
    // Membership check — only campaign members can replay events.
    rbac::require_member(&s.db, uid, q.campaign_id).await?;

    let events = ws::replay_events(&s.db, q.campaign_id, q.since, q.limit).await?;
    let max_seq = events.iter()
        .filter_map(|e| e.get("seq").and_then(|v| v.as_i64()))
        .max();
    Ok(Json(ReplayResponse { events, max_seq }))
}

pub fn router() -> Router<AppState> {
    Router::new().route("/ws-events", get(replay))
}
