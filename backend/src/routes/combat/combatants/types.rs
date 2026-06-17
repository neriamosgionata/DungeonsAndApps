// Combatant request/response types.
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use validator::Validate;

use super::Combatant;

#[derive(Debug, Deserialize, Validate)]
pub struct CombatantCreate {
    pub ref_type: String,
    pub character_id: Option<Uuid>,
    pub npc_id: Option<Uuid>,
    #[validate(length(min = 1, max = 80))]
    pub display_name: String,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub ac: Option<i32>,
    pub is_visible: Option<bool>,
    pub initiative_rolled: Option<bool>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CombatantUpdate {
    #[validate(length(min = 1, max = 80))]
    pub display_name: Option<String>,
    pub initiative: Option<i32>,
    pub dex_tiebreaker: Option<i16>,
    pub hp_current: Option<i32>,
    pub hp_max: Option<i32>,
    pub temp_hp: Option<i32>,
    pub ac: Option<i32>,
    pub conditions: Option<Vec<String>>,
    pub notes: Option<String>,
    pub is_visible: Option<bool>,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    #[validate(length(min = 3, max = 20))]
    pub token_color: Option<String>,
    pub token_on_map: Option<bool>,
    pub token_image: Option<String>,
    pub clear_token_image: Option<bool>,
    pub action_used: Option<bool>,
    pub bonus_action_used: Option<bool>,
    pub reaction_used: Option<bool>,
    pub movement_used_ft: Option<i32>,
    pub legendary_actions_used: Option<i32>,
    pub legendary_resistances_used: Option<i32>,
    pub readied_action: Option<serde_json::Value>,
    pub cover_bonus: Option<i32>,
    pub delayed_turn: Option<bool>,
    pub faction: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CombatantMove {
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub movement_cost: Option<f32>,
}

#[derive(Debug, Deserialize)]
pub struct UseAction {
    pub action: String,
}

#[derive(Debug, Deserialize)]
pub struct BulkAddBody {
    pub combatants: Vec<CombatantCreate>,
}

#[derive(Debug, Serialize)]
pub struct BulkAddResult {
    pub added: usize,
    pub failed: usize,
    pub combatants: Vec<Combatant>,
    pub errors: Vec<BulkAddError>,
}

#[derive(Debug, Serialize)]
pub struct BulkAddError {
    pub index: usize,
    pub display_name: Option<String>,
    pub error: String,
}
