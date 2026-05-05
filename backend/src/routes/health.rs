use crate::AppState;
use axum::{Json, Router, extract::State, routing::get};
use serde_json::{Value, json};

pub fn router() -> Router<AppState> {
    Router::new().route("/health", get(health))
}

async fn health(State(state): State<AppState>) -> Json<Value> {
    let db_ok = sqlx::query_scalar::<_, i32>("select 1").fetch_one(&state.db).await.is_ok();
    let s3_configured = state.cfg.s3.is_some();
    let s3_details = state.cfg.s3.as_ref().map(|s3| {
        json!({
            "endpoint": s3.endpoint,
            "bucket": s3.bucket,
            "region": s3.region,
            "has_access_key": !s3.access_key.is_empty(),
            "has_secret_key": !s3.secret_key.is_empty(),
        })
    });
    Json(json!({ 
        "ok": true, 
        "db": db_ok,
        "s3_configured": s3_configured,
        "s3": s3_details,
    }))
}
