// Spell casting route handlers: cast_spell + parse_spell_range_ft.
// Extracted from spells.rs (827 lines) to keep each handler under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
pub use crate::rbac::Role;
pub mod apply;
pub mod cast;
pub mod range;

pub use cast::cast_spell;
pub use range::parse_spell_range_ft;
