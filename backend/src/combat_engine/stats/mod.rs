// Derived stat computation. Split into focused submodules:
//   compute.rs   — compute_stats (orchestrator), apply_modifier
//   abilities.rs — ability_mod, save_proficient, casting_ability, racial bonuses, proficiency
//   ac.rs        — compute_ac_from_sheet, parse_ac_base
//   hp.rs        — compute_max_hp_from_sheet
//   weapon.rs    — compute_weapon_damage_expression
// Extracted from a single 770-line file to keep each under the 500-line cap (AGENTS.md §1.4).

pub mod abilities;
pub mod ac;
pub mod compute;
pub mod hp;
pub mod weapon;

pub use abilities::{ability_mod, apply_racial_bonuses, casting_ability, proficiency_from_level, save_proficient};
pub use compute::{apply_modifier, compute_stats};
pub use weapon::compute_weapon_damage_expression;
pub use ac::{compute_ac_from_sheet, parse_ac_base};
pub use hp::compute_max_hp_from_sheet;
