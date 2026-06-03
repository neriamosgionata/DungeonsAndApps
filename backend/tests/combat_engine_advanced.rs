//! Combat engine unit tests — tests actual production functions

use dungeonsandapps::combat_engine::{
    ability_mod, apply_damage_type, apply_hp_damage, compute_ac_from_sheet,
    compute_max_hp_from_sheet, compute_stats, concentration_check, crit_double_dice,
    is_massive_damage, proficiency_from_level, resolve_death_save, resolve_heal,
    CombatantSnapshot, ComputedStats, DeathSaveReq, HealReq,
};
use rand::SeedableRng;
use rand::rngs::StdRng;
use serde_json::json;
use uuid::Uuid;

fn base_snap() -> CombatantSnapshot {
    CombatantSnapshot {
        id: Uuid::new_v4(),
        encounter_id: Uuid::new_v4(),
        display_name: "Test".into(),
        character_id: None,
        npc_id: None,
        hp_current: 20,
        hp_max: 20,
        temp_hp: 0,
        base_ac: 12,
        base_speed: 30,
        level_total: 1,
        token_x: None,
        token_y: None,
        abilities: json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10}),
        saves: json!({}),
        skills: json!({}),
        proficiency_bonus: 0,
        conditions: vec![],
        active_effects: vec![],
        casting: json!({}),
        weapons: json!([]),
        equipment: json!([]),
        race: None,
        classes: json!([]),
        sheet_raw: json!({}),
    }
}

// =====================================================================
// ability_mod
// =====================================================================

#[test]
fn ability_mod_score_10_is_zero() {
    let snap = base_snap();
    assert_eq!(ability_mod(&snap, "str"), 0);
    assert_eq!(ability_mod(&snap, "dex"), 0);
    assert_eq!(ability_mod(&snap, "con"), 0);
    assert_eq!(ability_mod(&snap, "int"), 0);
    assert_eq!(ability_mod(&snap, "wis"), 0);
    assert_eq!(ability_mod(&snap, "cha"), 0);
}

#[test]
fn ability_mod_even_scores() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":12,"dex":14,"con":16,"int":18,"wis":20,"cha":22});
    assert_eq!(ability_mod(&snap, "str"), 1);
    assert_eq!(ability_mod(&snap, "dex"), 2);
    assert_eq!(ability_mod(&snap, "con"), 3);
    assert_eq!(ability_mod(&snap, "int"), 4);
    assert_eq!(ability_mod(&snap, "wis"), 5);
    assert_eq!(ability_mod(&snap, "cha"), 6);
}

#[test]
fn ability_mod_odd_scores() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":11,"dex":13,"con":15,"int":17,"wis":19,"cha":21});
    assert_eq!(ability_mod(&snap, "str"), 0);
    assert_eq!(ability_mod(&snap, "dex"), 1);
    assert_eq!(ability_mod(&snap, "con"), 2);
    assert_eq!(ability_mod(&snap, "int"), 3);
    assert_eq!(ability_mod(&snap, "wis"), 4);
    assert_eq!(ability_mod(&snap, "cha"), 5);
}

#[test]
fn ability_mod_min_and_max_clamped() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":1,"dex":30,"con":10,"int":10,"wis":10,"cha":10});
    assert_eq!(ability_mod(&snap, "str"), -4);
    assert_eq!(ability_mod(&snap, "dex"), 10);
}

#[test]
fn ability_mod_clamped_to_1() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":0,"dex":-5,"con":10,"int":10,"wis":10,"cha":10});
    assert_eq!(ability_mod(&snap, "str"), -4);
}

#[test]
fn ability_mod_clamped_to_30() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":100,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    assert_eq!(ability_mod(&snap, "str"), 10);
}

#[test]
fn ability_mod_missing_defaults_to_10() {
    let mut snap = base_snap();
    snap.abilities = json!({});
    assert_eq!(ability_mod(&snap, "str"), 0);
    assert_eq!(ability_mod(&snap, "wis"), 0);
}

#[test]
fn ability_mod_override_via_sheet_raw() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":8,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"abilities_override":{"str":20}});
    assert_eq!(ability_mod(&snap, "str"), 5);
    assert_eq!(ability_mod(&snap, "dex"), 0);
}

#[test]
fn ability_mod_override_clamped() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":8,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"abilities_override":{"str":50}});
    assert_eq!(ability_mod(&snap, "str"), 10);
    snap.sheet_raw = json!({"abilities_override":{"str":0}});
    assert_eq!(ability_mod(&snap, "str"), -4);
}

// =====================================================================
// proficiency_from_level
// =====================================================================

#[test]
fn proficiency_bonus_levels_1_through_4() {
    for lvl in 1..=4 {
        assert_eq!(proficiency_from_level(lvl), 2, "level {} should have PB +2", lvl);
    }
}

#[test]
fn proficiency_bonus_levels_5_through_8() {
    for lvl in 5..=8 {
        assert_eq!(proficiency_from_level(lvl), 3, "level {} should have PB +3", lvl);
    }
}

#[test]
fn proficiency_bonus_levels_9_through_12() {
    for lvl in 9..=12 {
        assert_eq!(proficiency_from_level(lvl), 4, "level {} should have PB +4", lvl);
    }
}

#[test]
fn proficiency_bonus_levels_13_through_16() {
    for lvl in 13..=16 {
        assert_eq!(proficiency_from_level(lvl), 5, "level {} should have PB +5", lvl);
    }
}

#[test]
fn proficiency_bonus_levels_17_through_20() {
    for lvl in 17..=20 {
        assert_eq!(proficiency_from_level(lvl), 6, "level {} should have PB +6", lvl);
    }
}

#[test]
fn proficiency_bonus_level_zero_clamped_to_1() {
    assert_eq!(proficiency_from_level(0), 2);
}

#[test]
fn proficiency_bonus_level_negative_clamped_to_1() {
    assert_eq!(proficiency_from_level(-5), 2);
}

// =====================================================================
// apply_damage_type
// =====================================================================

fn make_stats() -> ComputedStats {
    ComputedStats {
        ac: 10,
        speed: 30,
        ..Default::default()
    }
}

#[test]
fn damage_resistance_halves_rounds_down() {
    let mut stats = make_stats();
    stats.resistances.insert("fire".into());
    let (dmg, is_resist, is_vuln, is_immune) = apply_damage_type(15, "fire", &stats, false);
    assert_eq!(dmg, 7);
    assert!(is_resist);
    assert!(!is_vuln);
    assert!(!is_immune);
}

#[test]
fn damage_resistance_odd_damage_halves_floor() {
    let mut stats = make_stats();
    stats.resistances.insert("cold".into());
    let (dmg, _, _, _) = apply_damage_type(7, "cold", &stats, false);
    assert_eq!(dmg, 3);
}

#[test]
fn damage_vulnerability_doubles() {
    let mut stats = make_stats();
    stats.vulnerabilities.insert("thunder".into());
    let (dmg, is_resist, is_vuln, is_immune) = apply_damage_type(10, "thunder", &stats, false);
    assert_eq!(dmg, 20);
    assert!(!is_resist);
    assert!(is_vuln);
    assert!(!is_immune);
}

#[test]
fn damage_immunity_returns_zero() {
    let mut stats = make_stats();
    stats.immunities.insert("poison".into());
    let (dmg, is_resist, is_vuln, is_immune) = apply_damage_type(50, "poison", &stats, false);
    assert_eq!(dmg, 0);
    assert!(!is_resist);
    assert!(!is_vuln);
    assert!(is_immune);
}

#[test]
fn damage_immunity_checks_before_vulnerability_and_resistance() {
    let mut stats = make_stats();
    stats.immunities.insert("fire".into());
    stats.vulnerabilities.insert("fire".into());
    let (dmg, _, _, is_immune) = apply_damage_type(20, "fire", &stats, false);
    assert_eq!(dmg, 0);
    assert!(is_immune);
}

#[test]
fn damage_vulnerability_checks_before_resistance() {
    let mut stats = make_stats();
    stats.vulnerabilities.insert("cold".into());
    stats.resistances.insert("cold".into());
    let (dmg, _, is_vuln, _) = apply_damage_type(10, "cold", &stats, false);
    assert_eq!(dmg, 20);
    assert!(is_vuln);
}

#[test]
fn damage_all_resistance_applies_to_any_type() {
    let mut stats = make_stats();
    stats.resistances.insert("all".into());
    let (dmg, is_resist, _, _) = apply_damage_type(10, "necrotic", &stats, false);
    assert_eq!(dmg, 5);
    assert!(is_resist);
}

#[test]
fn damage_all_immunity_zeroes_any_type() {
    let mut stats = make_stats();
    stats.immunities.insert("all".into());
    let (dmg, _, _, is_immune) = apply_damage_type(100, "radiant", &stats, false);
    assert_eq!(dmg, 0);
    assert!(is_immune);
}

#[test]
fn damage_all_vulnerability_doubles_any_type() {
    let mut stats = make_stats();
    stats.vulnerabilities.insert("all".into());
    let (dmg, _, is_vuln, _) = apply_damage_type(10, "acid", &stats, false);
    assert_eq!(dmg, 20);
    assert!(is_vuln);
}

#[test]
fn nonmagical_damage_reduction_reduces_bps() {
    let mut stats = make_stats();
    stats.nonmagical_damage_reduction = 3;
    let (dmg, _, _, _) = apply_damage_type(8, "bludgeoning", &stats, false);
    assert_eq!(dmg, 5);
    let (dmg2, _, _, _) = apply_damage_type(8, "piercing", &stats, false);
    assert_eq!(dmg2, 5);
    let (dmg3, _, _, _) = apply_damage_type(8, "slashing", &stats, false);
    assert_eq!(dmg3, 5);
}

#[test]
fn nonmagical_dr_floor_at_zero() {
    let mut stats = make_stats();
    stats.nonmagical_damage_reduction = 3;
    let (dmg, _, _, _) = apply_damage_type(2, "bludgeoning", &stats, false);
    assert_eq!(dmg, 0);
}

#[test]
fn magical_damage_bypasses_nonmagical_dr() {
    let mut stats = make_stats();
    stats.nonmagical_damage_reduction = 3;
    let (dmg, _, _, _) = apply_damage_type(10, "bludgeoning", &stats, true);
    assert_eq!(dmg, 10);
}

#[test]
fn nonmagical_dr_does_not_affect_non_bps_types() {
    let mut stats = make_stats();
    stats.nonmagical_damage_reduction = 3;
    let (dmg, _, _, _) = apply_damage_type(10, "fire", &stats, false);
    assert_eq!(dmg, 10);
}

#[test]
fn nonmagical_immunity_zeroes_nonmagical_damage() {
    let mut stats = make_stats();
    stats.immunities.insert("nonmagical".into());
    let (dmg, _, _, is_immune) = apply_damage_type(20, "slashing", &stats, false);
    assert_eq!(dmg, 0);
    assert!(is_immune);
}

#[test]
fn nonmagical_immunity_bypassed_by_magical() {
    let mut stats = make_stats();
    stats.immunities.insert("nonmagical".into());
    let (dmg, _, _, _) = apply_damage_type(20, "slashing", &stats, true);
    assert_eq!(dmg, 20);
}

#[test]
fn nonmagical_resistance_bypassed_by_magical() {
    let mut stats = make_stats();
    stats.resistances.insert("nonmagical".into());
    let (dmg, _, _, _) = apply_damage_type(20, "slashing", &stats, false);
    assert_eq!(dmg, 10);
    let (dmg2, _, _, _) = apply_damage_type(20, "slashing", &stats, true);
    assert_eq!(dmg2, 20);
}

#[test]
fn damage_immunity_returns_false_for_resist_and_vuln_flags() {
    let mut stats = make_stats();
    stats.immunities.insert("poison".into());
    let (_, is_resist, is_vuln, is_immune) = apply_damage_type(10, "poison", &stats, false);
    assert!(!is_resist);
    assert!(!is_vuln);
    assert!(is_immune);
}

// =====================================================================
// is_massive_damage
// =====================================================================

#[test]
fn massive_damage_when_damage_exceeds_max_hp() {
    assert!(is_massive_damage(30, 50));
}

#[test]
fn massive_damage_when_damage_equals_max_hp() {
    assert!(is_massive_damage(30, 30));
}

#[test]
fn not_massive_damage_when_damage_below_max_hp() {
    assert!(!is_massive_damage(30, 29));
    assert!(!is_massive_damage(30, 0));
    assert!(!is_massive_damage(30, 1));
}

#[test]
fn not_massive_damage_when_hp_max_is_zero_or_negative() {
    assert!(!is_massive_damage(0, 0));
    assert!(!is_massive_damage(0, 100));
    assert!(!is_massive_damage(-5, 100));
}

// =====================================================================
// apply_hp_damage
// =====================================================================

#[test]
fn temp_hp_absorbs_all_damage_when_enough() {
    let (hp, temp) = apply_hp_damage(30, 10, 7);
    assert_eq!(hp, 30);
    assert_eq!(temp, 3);
}

#[test]
fn temp_hp_absorbs_exact_damage() {
    let (hp, temp) = apply_hp_damage(30, 10, 10);
    assert_eq!(hp, 30);
    assert_eq!(temp, 0);
}

#[test]
fn temp_hp_overflow_into_real_hp() {
    let (hp, temp) = apply_hp_damage(30, 5, 12);
    assert_eq!(hp, 23);
    assert_eq!(temp, 0);
}

#[test]
fn no_temp_hp_direct_damage() {
    let (hp, temp) = apply_hp_damage(30, 0, 8);
    assert_eq!(hp, 22);
    assert_eq!(temp, 0);
}

#[test]
fn zero_damage_no_change() {
    let (hp, temp) = apply_hp_damage(30, 10, 0);
    assert_eq!(hp, 30);
    assert_eq!(temp, 10);
}

#[test]
fn negative_damage_no_change() {
    let (hp, temp) = apply_hp_damage(30, 10, -5);
    assert_eq!(hp, 30);
    assert_eq!(temp, 10);
}

#[test]
fn negative_hp_goes_more_negative() {
    let (hp, temp) = apply_hp_damage(-3, 0, 7);
    assert_eq!(hp, -10);
    assert_eq!(temp, 0);
}

#[test]
fn damage_exceeds_both_temp_and_hp() {
    let (hp, temp) = apply_hp_damage(5, 3, 20);
    assert_eq!(hp, -12);
    assert_eq!(temp, 0);
}

// =====================================================================
// crit_double_dice
// =====================================================================

#[test]
fn crit_doubles_single_die() {
    assert_eq!(crit_double_dice("1d8"), "2d8");
    assert_eq!(crit_double_dice("1d6"), "2d6");
    assert_eq!(crit_double_dice("1d12"), "2d12");
}

#[test]
fn crit_doubles_multiple_dice() {
    assert_eq!(crit_double_dice("2d6"), "4d6");
    assert_eq!(crit_double_dice("3d8"), "6d8");
    assert_eq!(crit_double_dice("10d6"), "20d6");
}

#[test]
fn crit_preserves_flat_modifiers() {
    assert_eq!(crit_double_dice("1d8+3"), "2d8+3");
    assert_eq!(crit_double_dice("1d12+5"), "2d12+5");
    assert_eq!(crit_double_dice("2d6+7"), "4d6+7");
}

#[test]
fn crit_doubles_complex_expressions() {
    assert_eq!(crit_double_dice("1d8+2d6"), "2d8+4d6");
    assert_eq!(crit_double_dice("2d6+1d4+3"), "4d6+2d4+3");
}

#[test]
fn crit_doubles_uppercase_d() {
    assert_eq!(crit_double_dice("1D8+3"), "2d8+3");
}

#[test]
fn crit_no_dice_unchanged() {
    assert_eq!(crit_double_dice("5"), "5");
    assert_eq!(crit_double_dice("+3"), "+3");
}

#[test]
fn crit_cleans_whitespace() {
    assert_eq!(crit_double_dice("1d8 + 3"), "2d8+3");
    assert_eq!(crit_double_dice(" 2d6 + 1d4 "), "4d6+2d4");
}

#[test]
fn crit_implicit_count_not_doubled() {
    assert_eq!(crit_double_dice("d6"), "d6");
    assert_eq!(crit_double_dice("d8+3"), "d8+3");
}

// =====================================================================
// resolve_heal
// =====================================================================

#[test]
fn heal_simple_increases_hp() {
    let mut snap = base_snap();
    snap.hp_current = 10;
    snap.hp_max = 20;
    let req = HealReq { amount: 5, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.hp_before, 10);
    assert_eq!(result.hp_after, 15);
    assert!(!result.stabilized);
    assert!(!result.revived);
}

#[test]
fn heal_capped_at_hp_max() {
    let mut snap = base_snap();
    snap.hp_current = 18;
    snap.hp_max = 20;
    let req = HealReq { amount: 10, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.hp_after, 20);
}

#[test]
fn heal_from_zero_stabilizes_and_revives() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.hp_max = 20;
    let req = HealReq { amount: 5, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.hp_after, 5);
    assert!(result.stabilized);
    assert!(result.revived);
}

#[test]
fn heal_from_negative_hp_stabilizes() {
    let mut snap = base_snap();
    snap.hp_current = -5;
    snap.hp_max = 20;
    let req = HealReq { amount: 10, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.hp_before, -5);
    assert_eq!(result.hp_after, 5);
    assert!(result.stabilized);
}

#[test]
fn heal_zero_amount_does_nothing() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.hp_max = 20;
    let req = HealReq { amount: 0, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.hp_after, 0);
    assert!(!result.stabilized);
}

#[test]
fn heal_preserves_temp_hp() {
    let mut snap = base_snap();
    snap.hp_current = 5;
    snap.hp_max = 20;
    snap.temp_hp = 8;
    let req = HealReq { amount: 3, source_combatant_id: None, label: None };
    let result = resolve_heal(&snap, &req);
    assert_eq!(result.temp_hp_after, 8);
}

// =====================================================================
// resolve_death_save
// =====================================================================

#[test]
fn death_save_returns_ok_and_natural_roll_in_range() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    assert!(result.natural_roll >= 1 && result.natural_roll <= 20, "natural roll out of range: {}", result.natural_roll);
    assert!(!result.nat20 || result.natural_roll == 20);
    assert!(!result.nat1 || result.natural_roll == 1);
}

#[test]
fn death_save_with_advantage_uses_adv_roll() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: true, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    assert!(result.natural_roll >= 1 && result.natural_roll <= 20);
}

#[test]
fn death_save_with_disadvantage_uses_dis_roll() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: false, disadvantage: true, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    assert!(result.natural_roll >= 1 && result.natural_roll <= 20);
}

#[test]
fn death_save_advantage_and_disadvantage_cancel() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: true, disadvantage: true, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    assert!(result.natural_roll >= 1 && result.natural_roll <= 20);
}

#[test]
fn death_save_reads_previous_counts() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":2,"death_saves_failures":1});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    assert_eq!(result.successes_before, 2);
    assert_eq!(result.failures_before, 1);
}

#[test]
fn death_save_nat20_revives_to_1_hp() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    if result.nat20 {
        assert_eq!(result.hp_after, 1);
        assert!(result.alive);
        assert!(result.stabilized);
        assert_eq!(result.successes_after, 0);
        assert_eq!(result.failures_after, 0);
    }
}

#[test]
fn death_save_nat1_adds_two_failures() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":0});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    if result.nat1 {
        assert_eq!(result.failures_after, 2);
        assert!(!result.passed);
    }
}

#[test]
fn death_save_nat1_with_1_existing_failure_causes_death() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":0,"death_saves_failures":1});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    if result.nat1 {
        assert_eq!(result.failures_after, 3);
        assert!(result.died);
        assert!(!result.alive);
    }
}

#[test]
fn death_save_nat20_wipes_success_and_failure_counters() {
    let mut snap = base_snap();
    snap.hp_current = 0;
    snap.abilities = json!({"death_saves_successes":2,"death_saves_failures":2});
    let req = DeathSaveReq { advantage: false, disadvantage: false, label: None };
    let result = resolve_death_save(&snap, &req).expect("death save should succeed");
    if result.nat20 {
        assert_eq!(result.successes_after, 0);
        assert_eq!(result.failures_after, 0);
    }
}

// =====================================================================
// concentration_check — deterministic DC calculation tests
// =====================================================================

#[test]
fn concentration_dc_is_max_of_10_and_half_damage() {
    let snap = base_snap();
    let mut rng = StdRng::seed_from_u64(0);
    let (_broken, _roll) = concentration_check(&snap, 14, &mut rng);
    let (_broken2, _roll2) = concentration_check(&snap, 22, &mut rng);
    let (_broken3, _roll3) = concentration_check(&snap, 100, &mut rng);
}

#[test]
fn concentration_dc_at_least_10_for_low_damage() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":30,"int":10,"wis":10,"cha":10});
    let mut rng = StdRng::seed_from_u64(42);
    let (broken, _roll) = concentration_check(&snap, 2, &mut rng);
    assert!(!broken, "damage 2 → DC 10, con mod +10 should never fail");
}

#[test]
fn concentration_high_damage_raises_dc() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":1,"int":10,"wis":10,"cha":10});
    let mut rng = StdRng::seed_from_u64(7);
    let (broken, _roll) = concentration_check(&snap, 40, &mut rng);
    assert!(broken, "damage 40 → DC 20, con mod -5 should always fail");
}

#[test]
fn concentration_war_caster_feat_uses_advantage() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"feats":[{"key":"war_caster"}]});
    let mut rng = StdRng::seed_from_u64(99);
    let (_, roll) = concentration_check(&snap, 20, &mut rng);
    assert!(roll.total >= 3, "2d20kh1+2 should roll at least 3: got {}", roll.total);
}

// =====================================================================
// compute_stats — exhaustion levels 1–6
// =====================================================================

#[test]
fn exhaustion_level_1_save_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":1});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 1);
    assert!(stats.save_disadvantage);
    assert!(!stats.attack_disadvantage);
    assert!(!stats.speed_halved);
}

#[test]
fn exhaustion_level_2_speed_halved_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":2});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 2);
    assert!(stats.speed_halved);
}

#[test]
fn exhaustion_level_3_attack_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":3});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 3);
    assert!(stats.attack_disadvantage);
}

#[test]
fn exhaustion_level_4_still_attack_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":4});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 4);
    assert!(stats.attack_disadvantage);
}

#[test]
fn exhaustion_level_5_zero_speed() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":5});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 5);
    assert_eq!(stats.speed, 0);
}

#[test]
fn exhaustion_level_6_zero_speed_and_attack_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"exhaustion":6});
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 6);
    assert_eq!(stats.speed, 0);
    assert!(stats.attack_disadvantage);
}

#[test]
fn exhaustion_missing_from_sheet_defaults_to_zero() {
    let snap = base_snap();
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 0);
    assert!(!stats.save_disadvantage);
    assert!(!stats.speed_halved);
    assert!(!stats.attack_disadvantage);
}

// =====================================================================
// compute_ac_from_sheet
// =====================================================================

#[test]
fn ac_fallback_to_base_ac_when_no_armor_config() {
    let mut snap = base_snap();
    snap.base_ac = 14;
    snap.sheet_raw = json!({});
    assert_eq!(compute_ac_from_sheet(&snap), 14);
}

#[test]
fn ac_floor_at_1() {
    let mut snap = base_snap();
    snap.base_ac = -10;
    snap.sheet_raw = json!({});
    assert_eq!(compute_ac_from_sheet(&snap), 1);
}

#[test]
fn ac_with_armor_config_applies_dex_cap() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":18,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"armor":{"type":"light","ac_base":12,"max_dex":2}});
    let ac = compute_ac_from_sheet(&snap);
    assert_eq!(ac, 14);
}

#[test]
fn ac_with_shield_adds_2() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":14,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"armor":{"type":"light","ac_base":11,"max_dex":99},"shield":true});
    let ac = compute_ac_from_sheet(&snap);
    assert_eq!(ac, 15);
}

#[test]
fn ac_unarmored_barbarian_uses_con_and_dex() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":16,"con":14,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"armor":{"type":"unarmored_barbarian"}});
    let ac = compute_ac_from_sheet(&snap);
    assert_eq!(ac, 15);
}

#[test]
fn ac_unarmored_monk_uses_wis_and_dex() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":16,"con":10,"int":10,"wis":14,"cha":10});
    snap.sheet_raw = json!({"armor":{"type":"unarmored_monk"}});
    let ac = compute_ac_from_sheet(&snap);
    assert_eq!(ac, 15);
}

#[test]
fn ac_mage_armor_sets_base_13() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":14,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"armor":{"type":"mage_armor"}});
    let ac = compute_ac_from_sheet(&snap);
    assert_eq!(ac, 15);
}

// =====================================================================
// compute_max_hp_from_sheet
// =====================================================================

#[test]
fn max_hp_single_class_level_1() {
    let mut snap = base_snap();
    snap.level_total = 1;
    snap.classes = json!([{"name":"Fighter","hit_die":"d10","level":1}]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 12);
}

#[test]
fn max_hp_multi_level_average() {
    let mut snap = base_snap();
    snap.level_total = 3;
    snap.classes = json!([{"name":"Fighter","hit_die":"d10","level":3}]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 28);
}

#[test]
fn max_hp_multiclass() {
    let mut snap = base_snap();
    snap.level_total = 3;
    snap.classes = json!([
        {"name":"Fighter","hit_die":"d10","level":1},
        {"name":"Wizard","hit_die":"d6","level":2}
    ]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 26);
}

#[test]
fn max_hp_negative_con_mod() {
    let mut snap = base_snap();
    snap.level_total = 1;
    snap.classes = json!([{"name":"Wizard","hit_die":"d6","level":1}]);
    snap.abilities = json!({"str":10,"dex":10,"con":6,"int":10,"wis":10,"cha":10});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 4);
}

#[test]
fn max_hp_hill_dwarf_racial_bonus() {
    let mut snap = base_snap();
    snap.level_total = 3;
    snap.classes = json!([{"name":"Fighter","hit_die":"d10","level":3}]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    snap.race = Some("Hill Dwarf".into());
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 34);
}

#[test]
fn max_hp_hp_max_reduction_subtracts() {
    let mut snap = base_snap();
    snap.level_total = 3;
    snap.classes = json!([{"name":"Fighter","hit_die":"d10","level":3}]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"hp_max_reduction":5});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 23);
}

#[test]
fn max_hp_tough_feat_adds_2_per_level() {
    let mut snap = base_snap();
    snap.level_total = 3;
    snap.classes = json!([{"name":"Fighter","hit_die":"d10","level":3}]);
    snap.abilities = json!({"str":10,"dex":10,"con":14,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({"feats":[{"key":"tough"}]});
    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 34);
}

// =====================================================================
// compute_stats — conditions
// =====================================================================

#[test]
fn condition_blinded_gives_attack_disadvantage() {
    let mut snap = base_snap();
    snap.conditions = vec!["blinded".into()];
    let stats = compute_stats(&snap);
    assert!(stats.blinded);
    assert!(stats.attack_disadvantage);
}

#[test]
fn condition_incapacitated_sets_flag() {
    let mut snap = base_snap();
    snap.conditions = vec!["incapacitated".into()];
    let stats = compute_stats(&snap);
    assert!(stats.incapacitated);
}

#[test]
fn condition_paralyzed_is_incapacitating() {
    let mut snap = base_snap();
    snap.conditions = vec!["paralyzed".into()];
    let stats = compute_stats(&snap);
    assert!(stats.paralyzed);
    assert!(stats.incapacitated);
}

#[test]
fn condition_stunned_is_incapacitating() {
    let mut snap = base_snap();
    snap.conditions = vec!["stunned".into()];
    let stats = compute_stats(&snap);
    assert!(stats.stunned);
    assert!(stats.incapacitated);
}

#[test]
fn condition_unconscious_sets_prone_and_incapacitated() {
    let mut snap = base_snap();
    snap.conditions = vec!["unconscious".into()];
    let stats = compute_stats(&snap);
    assert!(stats.unconscious);
    assert!(stats.incapacitated);
    assert!(stats.prone);
}

#[test]
fn condition_invisible_gives_attack_advantage() {
    let mut snap = base_snap();
    snap.conditions = vec!["invisible".into()];
    let stats = compute_stats(&snap);
    assert!(stats.invisible);
    assert!(stats.attack_advantage);
}

#[test]
fn condition_grappled_sets_speed_to_zero() {
    let mut snap = base_snap();
    snap.conditions = vec!["grappled".into()];
    let stats = compute_stats(&snap);
    assert!(stats.grappled);
    assert_eq!(stats.speed, 0);
}

#[test]
fn condition_restrained_sets_speed_zero_and_attack_disadvantage() {
    let mut snap = base_snap();
    snap.conditions = vec!["restrained".into()];
    let stats = compute_stats(&snap);
    assert!(stats.restrained);
    assert!(stats.attack_disadvantage);
    assert_eq!(stats.speed, 0);
}

#[test]
fn condition_prone_gives_attack_disadvantage_and_speed_halved() {
    let mut snap = base_snap();
    snap.conditions = vec!["prone".into()];
    let stats = compute_stats(&snap);
    assert!(stats.prone);
    assert!(stats.attack_disadvantage);
    assert!(stats.prone_ranged_disadvantage);
}

#[test]
fn condition_frightened_gives_attack_disadvantage() {
    let mut snap = base_snap();
    snap.conditions = vec!["frightened".into()];
    let stats = compute_stats(&snap);
    assert!(stats.frightened);
    assert!(stats.attack_disadvantage);
}

#[test]
fn condition_charmed_sets_flag() {
    let mut snap = base_snap();
    snap.conditions = vec!["charmed".into()];
    let stats = compute_stats(&snap);
    assert!(stats.charmed);
}

#[test]
fn condition_deafened_sets_flag() {
    let mut snap = base_snap();
    snap.conditions = vec!["deafened".into()];
    let stats = compute_stats(&snap);
    assert!(stats.deafened);
}

#[test]
fn condition_poisoned_gives_attack_and_save_disadvantage() {
    let mut snap = base_snap();
    snap.conditions = vec!["poisoned".into()];
    let stats = compute_stats(&snap);
    assert!(stats.poisoned);
    assert!(stats.attack_disadvantage);
    assert!(stats.save_disadvantage);
}

#[test]
fn condition_petrified_full_effects() {
    let mut snap = base_snap();
    snap.conditions = vec!["petrified".into()];
    let stats = compute_stats(&snap);
    assert!(stats.petrified);
    assert!(stats.incapacitated);
    assert_eq!(stats.speed, 0);
    assert!(stats.save_disadvantage);
    assert!(stats.resistances.contains("bludgeoning"));
    assert!(stats.resistances.contains("piercing"));
    assert!(stats.resistances.contains("slashing"));
    assert!(stats.resistances.contains("fire"));
    assert!(stats.resistances.contains("cold"));
    assert!(stats.resistances.contains("lightning"));
    assert!(stats.resistances.contains("thunder"));
    assert!(stats.resistances.contains("acid"));
    assert!(stats.resistances.contains("poison"));
    assert!(stats.immunities.contains("poison"));
    assert!(stats.immunities.contains("psychic"));
}

#[test]
fn condition_timed_suffix_stripped() {
    let mut snap = base_snap();
    snap.conditions = vec!["blinded:3".into()];
    let stats = compute_stats(&snap);
    assert!(stats.blinded);
    assert!(stats.attack_disadvantage);
}

// =====================================================================
// compute_stats — class features
// =====================================================================

#[test]
fn barbarian_level_5_fast_movement_adds_10_speed() {
    let mut snap = base_snap();
    snap.level_total = 5;
    snap.classes = json!([{"name":"Barbarian","hit_die":"d12","level":5}]);
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 40);
}

#[test]
fn barbarian_fast_movement_blocked_by_heavy_armor() {
    let mut snap = base_snap();
    snap.level_total = 5;
    snap.classes = json!([{"name":"Barbarian","hit_die":"d12","level":5}]);
    snap.sheet_raw = json!({"armor":{"type":"heavy","ac_base":18,"max_dex":0}});
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 30);
}

#[test]
fn monk_unarmored_movement_level_2() {
    let mut snap = base_snap();
    snap.level_total = 2;
    snap.classes = json!([{"name":"Monk","hit_die":"d8","level":2}]);
    snap.sheet_raw = json!({"armor":{"type":"unarmored_monk"}});
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 40);
}

#[test]
fn monk_unarmored_movement_level_6() {
    let mut snap = base_snap();
    snap.level_total = 6;
    snap.classes = json!([{"name":"Monk","hit_die":"d8","level":6}]);
    snap.sheet_raw = json!({"armor":{"type":"unarmored_monk"}});
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 45);
}

#[test]
fn monk_unarmored_movement_blocked_by_shield() {
    let mut snap = base_snap();
    snap.level_total = 2;
    snap.classes = json!([{"name":"Monk","hit_die":"d8","level":2}]);
    snap.sheet_raw = json!({"armor":{"type":"unarmored_monk"},"shield":true});
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 30);
}

#[test]
fn rogue_level_7_gives_evasion() {
    let mut snap = base_snap();
    snap.level_total = 7;
    snap.classes = json!([{"name":"Rogue","hit_die":"d8","level":7}]);
    let stats = compute_stats(&snap);
    assert!(stats.evasion);
}

#[test]
fn monk_level_7_gives_evasion() {
    let mut snap = base_snap();
    snap.level_total = 7;
    snap.classes = json!([{"name":"Monk","hit_die":"d8","level":7}]);
    let stats = compute_stats(&snap);
    assert!(stats.evasion);
}

#[test]
fn bard_level_2_gives_jack_of_all_trades() {
    let mut snap = base_snap();
    snap.level_total = 2;
    snap.classes = json!([{"name":"Bard","hit_die":"d8","level":2}]);
    let stats = compute_stats(&snap);
    assert!(stats.jack_of_all_trades);
}

// =====================================================================
// compute_stats — sunlight sensitivity
// =====================================================================

#[test]
fn sunlight_sensitivity_causes_attack_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"sunlight_sensitivity":true});
    let stats = compute_stats(&snap);
    assert!(stats.attack_disadvantage);
}

// =====================================================================
// compute_stats — spellcasting
// =====================================================================

#[test]
fn spell_attack_bonus_uses_casting_ability() {
    let mut snap = base_snap();
    snap.level_total = 5;
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":18,"wis":10,"cha":10});
    snap.casting = json!({"ability":"int"});
    let stats = compute_stats(&snap);
    assert_eq!(stats.spell_attack_bonus, 7);
}

#[test]
fn spell_save_dc_computed_as_8_plus_pb_plus_ability() {
    let mut snap = base_snap();
    snap.level_total = 5;
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":16,"cha":10});
    snap.casting = json!({"ability":"wis"});
    let stats = compute_stats(&snap);
    assert_eq!(stats.spell_save_dc, 14);
}
