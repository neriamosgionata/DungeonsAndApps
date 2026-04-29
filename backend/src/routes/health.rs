use crate::AppState;
use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let db_ok = sqlx::query_scalar::<_, i32>("select 1").fetch_one(&state.db).await.is_ok();
    Json(json!({ "ok": true, "db": db_ok }))
}
