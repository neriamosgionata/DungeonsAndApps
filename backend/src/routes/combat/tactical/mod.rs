// Tactical combat route handlers: overlays, conditions, difficulty, hazards, surprise, positioning.
// Extracted from tactical.rs (1291 lines) to keep each handler group under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
pub use super::combatants::Combatant;
pub use super::Encounter;
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
    extract::{Path, Query, State},
    http::StatusCode,
};
use rand::SeedableRng;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;

pub mod conditions;
pub mod difficulty;
pub mod hazards;
pub mod overlays;
pub mod positioning;
pub mod surprise;

pub use conditions::{
    add_condition, check_condition_immunity, ConditionBody, PatchEffectsBody, PatchEffectsResult,
};
pub use difficulty::{encounter_difficulty, DifficultyResult, DifficultyThresholds};
pub use hazards::{overlay_damage, OverlayDamageBody, OverlayDamageResult, OverlayTargetResult};
pub use overlays::{create_overlay, delete_overlay, list_overlays, Overlay, OverlayCreate};
pub use positioning::{
    calculate_cover, check_flanking, is_between, is_flanking, segments_intersect, CoverQuery,
    CoverResult, FlankPair, FlankResult,
};
pub use surprise::{
    surprise_auto, surprise_round, SurpriseAutoBody, SurpriseAutoResult, SurpriseBody,
    SurprisePerception, SurpriseStealthRoll,
};
