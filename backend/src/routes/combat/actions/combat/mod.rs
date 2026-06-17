// Combat action route handlers: attack, deal_damage, heal, death_save, skill_check, save, computed_stats.
// Extracted from combat.rs (1168 lines) to keep each handler under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility.
pub use super::super::tactical::{is_between, is_flanking, segments_intersect};

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
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

pub mod ammo;
pub mod attack;
pub mod attack_apply;
pub mod damage;
pub mod death_save;
pub mod heal;
pub mod skills;

pub use ammo::{decrement_ammo, decrement_thrown_weapon, infer_ammo_type};
pub use attack::{attack, AttackBody};
pub use attack_apply::apply_attack_outcome;
pub use damage::{deal_damage, DamageBody};
pub use death_save::{death_save, DeathSaveBody};
pub use heal::{heal, HealBody};
pub use skills::{computed_stats, roll_save, skill_check, SaveBody, SkillCheckBody};
