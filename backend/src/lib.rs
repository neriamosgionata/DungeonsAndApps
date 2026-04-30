pub mod auth;
pub mod config;
pub mod db;
pub mod dice;
pub mod error;
pub mod extract;
pub mod rbac;
pub mod routes;
pub mod state;
pub mod ws;

pub use state::AppState;

use axum::Router;
use tower_http::{
    cors::{CorsLayer, AllowOrigin},
    trace::TraceLayer,
};

pub fn app(state: AppState) -> Router {
    let cors = if state.cfg.cors_origin == "*" {
        // Only allow any origin in development
        CorsLayer::permissive()
    } else {
        // Production: explicit origins only (supports comma-separated list)
        let origins: Vec<axum::http::HeaderValue> = state.cfg.cors_origin
            .split(',')
            .map(|s| s.trim().parse().unwrap())
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
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}
