// Combat resolvers split into focused submodules.
// Re-exports preserve the public API.

pub mod attack;
pub mod damage;
pub mod damage_type;
pub mod death_save;
pub mod heal;
pub mod polearm;
pub mod save;
pub mod skill_check;
pub mod two_weapon_fight;
pub mod types;

pub use attack::*;
pub use damage::*;
pub use damage_type::*;
pub use death_save::*;
pub use heal::*;
pub use polearm::*;
pub use save::*;
pub use skill_check::*;
pub use two_weapon_fight::*;
pub use types::*;
