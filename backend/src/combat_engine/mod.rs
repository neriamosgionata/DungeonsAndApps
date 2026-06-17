// D&D 5e Combat Resolution Engine
// Pure functions for attack / damage / save / derived-stat computation.
// DB interaction wrappers live in load.rs.
// Extracted from a single 2,585-line file into submodules:
//   types.rs      — NPC types + ComputedStats + CombatantSnapshot + EffectSnapshot
//   stats.rs      — compute_stats, apply_modifier, all stat helpers
//   resolvers.rs  — resolve_attack, resolve_save, resolve_heal, concentration, etc.
//   load.rs       — load_snapshot, load_snapshots_batch (DB interaction)

pub mod types;
pub mod stats;
pub mod resolvers;
pub mod load;

pub use types::*;
pub use stats::*;
pub use resolvers::*;
pub use load::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proficiency_from_level() {
        assert_eq!(proficiency_from_level(1), 2);
        assert_eq!(proficiency_from_level(4), 2);
        assert_eq!(proficiency_from_level(5), 3);
        assert_eq!(proficiency_from_level(8), 3);
        assert_eq!(proficiency_from_level(9), 4);
        assert_eq!(proficiency_from_level(17), 6);
    }

    #[test]
    fn test_crit_double_dice() {
        assert_eq!(crit_double_dice("1d8+3"), "2d8+3");
        assert_eq!(crit_double_dice("2d6+1d4+5"), "4d6+2d4+5");
        assert_eq!(crit_double_dice("1d10"), "2d10");
        assert_eq!(crit_double_dice("5"), "5");
    }

    #[test]
    fn test_apply_damage_type() {
        let mut stats = ComputedStats::default();
        stats.resistances.insert("fire".into());
        stats.vulnerabilities.insert("cold".into());
        stats.immunities.insert("poison".into());

        assert_eq!(apply_damage_type(10, "fire", &stats, false), (5, true, false, false));
        assert_eq!(apply_damage_type(10, "cold", &stats, false), (20, false, true, false));
        assert_eq!(apply_damage_type(10, "poison", &stats, false), (0, false, false, true));
        assert_eq!(apply_damage_type(10, "slashing", &stats, false), (10, false, false, false));
    }

    #[test]
    fn test_apply_hp_damage() {
        assert_eq!(apply_hp_damage(20, 5, 3), (20, 2));
        assert_eq!(apply_hp_damage(20, 5, 7), (18, 0));
        assert_eq!(apply_hp_damage(20, 0, 5), (15, 0));
        assert_eq!(apply_hp_damage(20, 10, 0), (20, 10));
    }
}