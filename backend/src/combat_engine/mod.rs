// D&D 5e Combat Resolution Engine
// Pure functions for attack / damage / save / derived-stat computation.
// DB interaction wrappers live in load.rs.
// Extracted from a single 2,585-line file into submodules:
//   types.rs      — NPC types + ComputedStats + CombatantSnapshot + EffectSnapshot
//   stats.rs      — compute_stats, apply_modifier, all stat helpers
//   resolvers.rs  — resolve_attack, resolve_save, resolve_heal, concentration, etc.
//   load.rs       — load_snapshot, load_snapshots_batch (DB interaction)

pub mod load;
pub mod resolvers;
pub mod stats;
pub mod types;

pub use load::*;
pub use resolvers::*;
pub use stats::*;
pub use types::*;

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

        assert_eq!(
            apply_damage_type(10, "fire", &stats, false),
            (5, true, false, false)
        );
        assert_eq!(
            apply_damage_type(10, "cold", &stats, false),
            (20, false, true, false)
        );
        assert_eq!(
            apply_damage_type(10, "poison", &stats, false),
            (0, false, false, true)
        );
        assert_eq!(
            apply_damage_type(10, "slashing", &stats, false),
            (10, false, false, false)
        );
    }

    #[test]
    fn test_apply_hp_damage() {
        assert_eq!(apply_hp_damage(20, 5, 3), (20, 2));
        assert_eq!(apply_hp_damage(20, 5, 7), (18, 0));
        assert_eq!(apply_hp_damage(20, 0, 5), (15, 0));
        assert_eq!(apply_hp_damage(20, 10, 0), (20, 10));
    }

    // HIGH-5: HP must clamp to 0 — never go negative. PHB p.197.
    #[test]
    fn hp_clamps_to_zero_at_zero_hp() {
        assert_eq!(apply_hp_damage(0, 0, 5), (0, 0));
        assert_eq!(apply_hp_damage(3, 0, 100), (0, 0));
    }

    // HIGH-3: cover="full" must reject the attack (PHB total cover — target
    // can't be targeted directly). Pre-fix it fell through to 0 AC bonus.
    #[test]
    fn cover_full_rejects_attack() {
        use super::resolvers::types::AttackReq;
        use super::resolvers::attack::resolve_attack;
        use super::types::CombatantSnapshot;
        use uuid::Uuid;
        let mut a = CombatantSnapshot::default();
        let mut t = CombatantSnapshot::default();
        a.token_x = Some(0.0); a.token_y = Some(50.0);
        a.level_total = 1; a.proficiency_bonus = 2;
        t.token_x = Some(20.0); t.token_y = Some(50.0);
        t.hp_current = 30; t.hp_max = 30;
        let attacker_stats = super::stats::compute_stats(&a);
        let target_stats = super::stats::compute_stats(&t);
        let req = AttackReq {
            target_id: Uuid::new_v4(),
            attack_expression: Some("1d20+5".into()),
            damage_expression: Some("1d8+2".into()),
            damage_type: "slashing".into(),
            cover: Some("full".into()),
            proficient: Some(true),
            ..Default::default()
        };
        let res = resolve_attack(&a, &t, &req, &attacker_stats, &target_stats);
        assert!(res.is_err(), "full cover must reject attack, got {:?}", res);
        assert!(res.unwrap_err().contains("total cover"), "error should mention total cover");
    }

    // HIGH-2: 1 cell = 5ft = 20% of the map. Place attacker 4ft (16%) from a
    // paralyzed target. Pre-fix the 5% threshold missed this. After the fix
    // the auto-crit fires because 16% < 20%.
    #[test]
    fn within_5ft_auto_crit_at_4ft() {
        use super::resolvers::types::AttackReq;
        use super::resolvers::attack::resolve_attack;
        use super::types::CombatantSnapshot;
        use uuid::Uuid;
        let mut a = CombatantSnapshot::default();
        let mut t = CombatantSnapshot::default();
        a.token_x = Some(40.0); a.token_y = Some(50.0);
        a.level_total = 1; a.proficiency_bonus = 2;
        // 4ft = 16% of map (1 cell = 20%)
        t.token_x = Some(56.0); t.token_y = Some(50.0);
        t.hp_current = 30; t.hp_max = 30;
        let attacker_stats = super::stats::compute_stats(&a);
        let mut target_stats = super::stats::compute_stats(&t);
        target_stats.paralyzed = true;
        let req = AttackReq {
            target_id: Uuid::new_v4(),
            attack_expression: Some("1d20+5".into()),
            damage_expression: Some("1d8+2".into()),
            damage_type: "slashing".into(),
            proficient: Some(true),
            ..Default::default()
        };
        let res = resolve_attack(&a, &t, &req, &attacker_stats, &target_stats).unwrap();
        if res.natural_roll != 20 {
            assert!(res.critical, "paralyzed at 4ft (16%) should auto-crit (within 5ft, 20% threshold)");
        }
    }

    // HIGH-2 (cont): at 6ft (24% of map) is OUTSIDE 5ft reach, so no auto-crit
    // even on paralyzed target (PHB: 5ft reach for melee).
    #[test]
    fn beyond_5ft_no_auto_crit_even_on_paralyzed() {
        use super::resolvers::types::AttackReq;
        use super::resolvers::attack::resolve_attack;
        use super::types::CombatantSnapshot;
        use uuid::Uuid;
        let mut a = CombatantSnapshot::default();
        let mut t = CombatantSnapshot::default();
        a.token_x = Some(40.0); a.token_y = Some(50.0);
        a.level_total = 1; a.proficiency_bonus = 2;
        // 6ft = 24% of map (beyond 1 cell)
        t.token_x = Some(64.0); t.token_y = Some(50.0);
        t.hp_current = 30; t.hp_max = 30;
        let attacker_stats = super::stats::compute_stats(&a);
        let mut target_stats = super::stats::compute_stats(&t);
        target_stats.paralyzed = true;
        let req = AttackReq {
            target_id: Uuid::new_v4(),
            attack_expression: Some("1d20+5".into()),
            damage_expression: Some("1d8+2".into()),
            damage_type: "slashing".into(),
            proficient: Some(true),
            ..Default::default()
        };
        let res = resolve_attack(&a, &t, &req, &attacker_stats, &target_stats).unwrap();
        if res.natural_roll != 20 {
            assert!(!res.critical, "paralyzed at 6ft (24%) is beyond 5ft, no auto-crit");
        }
    }
}
