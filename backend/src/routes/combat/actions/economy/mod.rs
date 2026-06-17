// Economy & utility action route handlers: dodge, disengage, help, dash, hide,
// opportunity_attack, two_weapon_fight, delay_turn, contested_hide, search, use_object.
// Extracted from economy.rs (956 lines) to keep each handler group under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
pub use super::super::combatants::Combatant;
pub use crate::rbac::Role;

use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac, ws,
    AppState,
};
use axum::{
    Json,
    extract::{Path, State},
};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub mod auth;
pub mod contested;
pub mod delay;
pub mod dodges;
pub mod help;
pub mod movement;
pub mod opportunity;
pub mod twf;
pub mod utility;

pub use auth::{consume_action_or_bonus, require_action_auth, ActionAuth};
pub use contested::{
    contested_hide, ContestedHideBody, ContestedHideResult, HideObserverResult,
};
pub use delay::{delay_turn, DelayBody};
pub use dodges::{dodge, disengage, ShoveBody, ShoveResult};
pub use help::help_action;
pub use movement::{dash, hide};
pub use opportunity::{opportunity_attack, OppAttackBody};
pub use twf::{two_weapon_fight, TwoWeaponFightBody};
pub use utility::{search_action, use_object, SearchBody, UseObjectBody};
