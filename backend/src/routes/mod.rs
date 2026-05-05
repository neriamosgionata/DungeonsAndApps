pub mod admin;
pub mod auth;
pub mod campaigns;
pub mod characters;
pub mod combat;
pub mod dice;
pub mod effects;
pub mod group;
pub mod health;
pub mod invitations;
pub mod maps;
pub mod messages;
pub mod notifications;
pub mod recap;
pub mod spells;
pub mod uploads;
pub mod users;
pub mod world;

use crate::AppState;
use axum::Router;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(health::router())
        .merge(auth::router())
        .merge(campaigns::router())
        .merge(characters::router())
        .merge(dice::router())
        .merge(spells::router())
        .merge(combat::router())
        .merge(recap::router())
        .merge(maps::router())
        .merge(world::router())
        .merge(group::router())
        .merge(messages::router())
        .merge(notifications::router())
        .merge(invitations::router())
        .merge(uploads::router())
        .merge(users::router())
        .merge(effects::router())
        .merge(admin::router())
}
