pub mod auth;
pub mod config;
pub mod db;
pub mod dice;
pub mod error;
pub mod extract;
pub mod rate_limit;
pub mod rbac;
pub mod routes;
pub mod state;
pub mod ws;

pub use state::AppState;

use axum::{Router, middleware};
use tower_http::{
    cors::{CorsLayer, AllowOrigin},
    trace::TraceLayer,
};

pub fn app(state: AppState) -> Router {
    let cors = if state.cfg.cors_origin == "*" {
        CorsLayer::permissive()
    } else {
        let origins: Vec<axum::http::HeaderValue> = state.cfg.cors_origin
            .split(',')
            .filter_map(|s| {
                let t = s.trim();
                if t.is_empty() { return None; }
                t.parse().ok().or_else(|| { tracing::warn!("invalid CORS origin '{t}', skipping"); None })
            })
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::PUT, axum::http::Method::PATCH, axum::http::Method::DELETE])
            .allow_credentials(true)
            .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE, axum::http::header::HeaderName::from_static("x-campaign-id")])
    };

    Router::new()
        .nest("/api/v1", routes::router())
        .route("/ws", axum::routing::get(ws::handler))
        .layer(middleware::from_fn(rate_limit::http_rate_limit))
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
