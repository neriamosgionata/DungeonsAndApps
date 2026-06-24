// Encounter request types.
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;
use validator::Validate;


#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Encounter {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub name: String,
    pub status: String,
    pub round: i32,
    pub turn_index: i32,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub map_grid_size: i32,
    pub show_grid: bool,
    pub grid_type: String,
    pub lair_action_used: bool,
    pub updated_at: OffsetDateTime,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterCreate {
    #[validate(length(min = 1, max = 120))]
    pub name: String,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct EncounterUpdate {
    #[validate(length(min = 1, max = 120))]
    pub name: Option<String>,
    pub notes: Option<String>,
    pub map_image: Option<String>,
    pub clear_map_image: Option<bool>,
    #[validate(range(min = 20, max = 200))]
    pub map_grid_size: Option<i32>,
    pub show_grid: Option<bool>,
    pub grid_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SetInitiativeEntry {
    pub combatant_id: Uuid,
    pub initiative: i32,
}

#[derive(Debug, Deserialize)]
pub struct SetInitiativeBody {
    pub combatants: Vec<SetInitiativeEntry>,
}

#[derive(Debug, Deserialize)]
pub struct GotoTurnBody {
    pub turn_index: i32,
    /// Optional explicit round to jump to (>= 1). When omitted, the current
    /// round is kept (the frontend only changes turn_index within a round).
    pub round: Option<i32>,
}
