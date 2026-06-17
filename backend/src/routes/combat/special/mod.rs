// Special combat action route handlers: grapple, shove, multiattack, legendary, class feature.
// Extracted from special.rs (1490 lines) to keep each handler group under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
pub use super::Encounter;
pub use super::combatants::Combatant;
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

pub mod class_feature;
pub mod escape;
pub mod grapple;
pub mod legendary;
pub mod multiattack;
pub mod parse_multiattack;
pub mod shove;

pub use class_feature::{class_feature, ClassFeatureBody, ClassFeatureResult};
pub use escape::{grapple_escape, GrappleEscapeBody, GrappleEscapeResult};
pub use grapple::{grapple, GrappleBody, GrappleResult};
pub use legendary::{lair_action, legendary_action, LegendaryActionResult};
pub use multiattack::{multiattack, trigger_ready, MultiAttackBody, MultiAttackResult, MultiAttackTarget};
pub use parse_multiattack::{parse_multiattack, parse_npc_multiattack, try_parse_npc_multiattack, ParsedMultiAttack, ParsedSubAttack};
pub use shove::{shove, stand_up, ShoveBody, ShoveResult};
