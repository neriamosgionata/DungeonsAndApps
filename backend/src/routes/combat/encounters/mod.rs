// Encounter CRUD + initiative + turn management route handlers.
// Extracted from encounters.rs (622 lines) to keep each handler under the 500-line cap
// (per AGENTS.md §1.4). Public re-exports preserve call-site compatibility with mod.rs.
pub mod create;
pub mod delete;
pub mod end;
pub mod initiative;
pub mod list;
pub mod read;
pub mod start;
pub mod turns;
pub mod types;
pub mod update;

pub use create::create;
pub use delete::delete;
pub use end::end_encounter;
pub use initiative::set_initiative;
pub use list::list;
pub use read::read;
pub use start::start;
pub use turns::{goto_turn, next_turn, prev_turn};
pub use types::{EncounterCreate, EncounterUpdate, GotoTurnBody, SetInitiativeBody};
pub use update::update;
