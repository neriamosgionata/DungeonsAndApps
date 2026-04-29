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
use tower_http::{cors::CorsLayer, trace::TraceLayer};

pub fn app(state: AppState) -> Router {
    Router::new()
        .nest("/api/v1", routes::router())
        .route("/ws", axum::routing::get(ws::handler))
        .layer(TraceLayer::new_for_http())
        .layer(CorsLayer::permissive())
        .with_state(state)
}
