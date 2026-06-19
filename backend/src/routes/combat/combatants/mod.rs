// Combatant CRUD + movement + action toggle route handlers.
// Extracted from combatants.rs (875 lines) to keep each handler group under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
use sqlx::FromRow;

pub use super::Encounter;
pub use crate::rbac::Role;

use crate::{
    combat_engine,
    error::{AppError, AppResult},
    extract::AuthUser,
    rbac, ws,
};
use serde::Serialize;
use uuid::Uuid;
use validator::Validate;

pub mod action;
pub mod bulk;
pub mod create;
pub mod delete;
pub mod list;
pub mod move_combatant;
pub mod types;
pub mod update;

pub use action::use_action;
pub use bulk::{bulk_add_combatants};
pub use create::add_combatant;
pub use delete::delete_combatant;
pub use list::list_combatants;
pub use move_combatant::move_combatant;
pub use types::{BulkAddBody, BulkAddError, BulkAddResult, CombatantCreate, CombatantMove, CombatantUpdate, UseAction};
pub use update::update_combatant;

#[derive(Debug, Serialize, FromRow)]
pub struct Combatant {
    pub id: Uuid,
    pub encounter_id: Uuid,
    pub ref_type: String,
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    pub display_name: String,
    pub initiative: i32,
    pub dex_tiebreaker: i16,
    pub hp_current: i32,
    pub hp_max: i32,
    pub temp_hp: i32,
    pub ac: i32,
    pub conditions: Vec<String>,
    pub notes: Option<String>,
    pub is_visible: bool,
    pub turn_order: i32,
    pub initiative_rolled: bool,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    pub token_color: Option<String>,
    pub token_on_map: bool,
    pub token_image: Option<String>,
    pub portrait_url: Option<String>,
    pub token_moved_round: Option<i32>,
    pub action_used: bool,
    pub bonus_action_used: bool,
    pub reaction_used: bool,
    pub movement_used_ft: i32,
    pub legendary_actions_max: i32,
    pub legendary_actions_used: i32,
    pub legendary_resistances_max: i32,
    pub legendary_resistances_used: i32,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: i32,
    pub delayed_turn: bool,
    pub action_spell_level: i16,
    pub bonus_action_spell_level: i16,
    pub last_hit_attack_total: Option<i32>,
    pub last_hit_damage: Option<i32>,
    pub spell_being_cast: Option<String>,
    pub level_override: i32,
    pub faction: String,
    pub vision_range: Option<i32>,
    pub pending_hits: serde_json::Value,
}
