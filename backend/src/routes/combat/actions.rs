// Route handlers and helpers for combat actions.
// All handlers have been extracted to actions/{sync,reactions,combat,economy}.rs.
// This file is now a re-export shim; no handler logic remains.
use super::*;

pub mod sync;
pub mod reactions;
pub mod combat;
pub mod economy;
pub use sync::*;
pub use reactions::*;
pub use combat::*;
pub use economy::*;

