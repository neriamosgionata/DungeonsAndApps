// Route handlers and helpers for combat actions.
// All handlers have been extracted to actions/{sync,reactions,combat,economy}.rs.
// This file is now a re-export shim; no handler logic remains.
use super::*;

pub mod combat;
pub mod economy;
pub mod reactions;
pub mod sync;
pub use combat::*;
pub use economy::*;
pub use reactions::*;
pub use sync::*;
