// Combat resolvers split into focused submodules.
// Re-exports preserve the public API.

pub mod types;
pub mod attack;
pub mod two_weapon_fight;
pub mod damage;
pub mod save;
pub mod heal;
pub mod death_save;
pub mod skill_check;
pub mod damage_type;

pub use types::*;
pub use attack::*;
pub use two_weapon_fight::*;
pub use damage::*;
pub use save::*;
pub use heal::*;
pub use death_save::*;
pub use skill_check::*;
pub use damage_type::*;