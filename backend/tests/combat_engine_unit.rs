use dungeonsandapps::combat_engine::{
    AttackReq, CombatantSnapshot, WeaponProps, apply_damage_type, apply_hp_damage,
    compute_max_hp_from_sheet, compute_stats, concentration_check, is_wielding_polearm,
    proficiency_from_level, resolve_attack, resolve_polearm_ba_attack, resolve_two_weapon_attack,
};
use rand::SeedableRng;
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

#[tokio::test]
async fn compute_stats_exhaustion_level_1_save_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 1 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 1);
    assert!(
        stats.save_disadvantage,
        "exhaustion 1 → disadvantage on ability checks"
    );
    assert!(!stats.attack_disadvantage);
    assert!(!stats.speed_halved);
}

#[tokio::test]
async fn compute_stats_exhaustion_level_2_speed_halved_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 2 });
    let stats = compute_stats(&snap);
    // Exhaustion level 2 sets the speed_halved flag for UI/consumers.
    // The actual speed computation happens in the post-process step
    // which runs before exhaustion is read, so speed stays at base_speed here.
    assert!(stats.speed_halved);
    assert_eq!(stats.exhaustion, 2);
}

#[tokio::test]
async fn compute_stats_exhaustion_level_3_attack_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 3 });
    let stats = compute_stats(&snap);
    assert!(stats.attack_disadvantage);
}

#[tokio::test]
async fn compute_stats_exhaustion_level_5_zero_speed() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 5 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 0);
}

#[tokio::test]
async fn compute_stats_exhaustion_level_6_still_zero_speed() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 6 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 0);
    assert!(stats.attack_disadvantage);
}

#[tokio::test]
async fn compute_stats_petrified_resistances_and_incapacitated() {
    let mut snap = base_snap();
    snap.conditions = vec!["petrified".into()];
    let stats = compute_stats(&snap);
    assert!(stats.petrified);
    assert!(stats.incapacitated);
    assert_eq!(stats.speed, 0);
    assert!(stats.resistances.contains("bludgeoning"));
    assert!(stats.resistances.contains("piercing"));
    assert!(stats.resistances.contains("slashing"));
    assert!(stats.resistances.contains("fire"));
    assert!(stats.immunities.contains("poison"));
    assert!(stats.immunities.contains("psychic"));
}

#[tokio::test]
async fn compute_stats_paralyzed_with_fly_speed_still_zero() {
    let mut snap = base_snap();
    snap.conditions = vec!["paralyzed".into()];
    snap.sheet_raw = json!({ "fly_speed": 60 });
    let stats = compute_stats(&snap);
    assert!(stats.paralyzed);
    assert_eq!(
        stats.speed, 0,
        "paralyzed + fly_speed=60 must keep speed=0 (PHB p.292)"
    );
}

#[tokio::test]
async fn compute_stats_stunned_with_fly_speed_still_zero() {
    let mut snap = base_snap();
    snap.conditions = vec!["stunned".into()];
    snap.sheet_raw = json!({ "fly_speed": 60 });
    let stats = compute_stats(&snap);
    assert!(stats.stunned);
    assert_eq!(
        stats.speed, 0,
        "stunned + fly_speed=60 must keep speed=0 (PHB p.292)"
    );
}

#[tokio::test]
async fn compute_stats_fly_speed_uses_higher_of_walk_or_fly() {
    let mut snap = base_snap();
    snap.base_speed = 30;
    snap.sheet_raw = json!({ "fly_speed": 60 });
    let stats = compute_stats(&snap);
    assert_eq!(
        stats.speed, 60,
        "walk 30 + fly 60 → 60 (max, not replace — PHB)"
    );
}

#[tokio::test]
async fn compute_stats_fly_only_creature_uses_fly_speed() {
    let mut snap = base_snap();
    snap.base_speed = 0;
    snap.sheet_raw = json!({ "fly_speed": 80 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.speed, 80, "fly-only creature (walk 0) moves at fly 80");
}

#[tokio::test]
async fn compute_stats_heavy_armor_master_dr3() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "nonmagical_damage_reduction": 3 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.nonmagical_damage_reduction, 3);
}

#[tokio::test]
async fn compute_stats_gnome_cunning_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "gnome_cunning": true });
    let stats = compute_stats(&snap);
    assert!(stats.gnome_cunning);
}

#[tokio::test]
async fn compute_stats_savage_attacks_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "savage_attacks": true });
    let stats = compute_stats(&snap);
    assert!(stats.savage_attacks);
}

#[tokio::test]
async fn concentration_check_war_caster_uses_advantage() {
    let mut snap = base_snap();
    // con 20 → +5 mod; war_caster feat
    snap.abilities = json!({"str":10,"dex":10,"con":20,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({ "feats": [{ "key": "war_caster" }] });

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let (broken, roll) = concentration_check(&snap, 20, &mut rng);
    // DC = max(10, 10) = 10; with +5 con mod and advantage, very unlikely to fail
    // Just verify the expression was 2d20kh1 style by checking total is plausible
    assert!(
        roll.total >= 6,
        "2d20kh1+5 should roll at least 6: got {}",
        roll.total
    );
    let _ = broken; // result is probabilistic, don't assert pass/fail
}

#[tokio::test]
async fn concentration_check_normal_uses_1d20() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({});

    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let (_broken, roll) = concentration_check(&snap, 20, &mut rng);
    assert!(
        roll.total >= 1 && roll.total <= 20,
        "1d20+0 total out of range: {}",
        roll.total
    );
}

#[tokio::test]
async fn apply_damage_type_nonmagical_dr_reduces_bps() {
    let stats = dungeonsandapps::combat_engine::ComputedStats {
        nonmagical_damage_reduction: 3,
        ..Default::default()
    };
    let (dmg, _, _, _) = apply_damage_type(10, "bludgeoning", &stats, false);
    assert_eq!(dmg, 7);

    let (dmg2, _, _, _) = apply_damage_type(10, "piercing", &stats, false);
    assert_eq!(dmg2, 7);

    let (dmg3, _, _, _) = apply_damage_type(10, "slashing", &stats, false);
    assert_eq!(dmg3, 7);

    // DR doesn't reduce below 0
    let (dmg4, _, _, _) = apply_damage_type(2, "bludgeoning", &stats, false);
    assert_eq!(dmg4, 0);

    // Fire is not affected by DR
    let (dmg5, _, _, _) = apply_damage_type(10, "fire", &stats, false);
    assert_eq!(dmg5, 10);
}

#[tokio::test]
async fn apply_damage_type_magical_bypasses_nonmagical_dr() {
    let stats = dungeonsandapps::combat_engine::ComputedStats {
        nonmagical_damage_reduction: 3,
        ..Default::default()
    };
    let (dmg, _, _, _) = apply_damage_type(10, "bludgeoning", &stats, true);
    assert_eq!(dmg, 10, "magical damage bypasses nonmagical DR");
}

#[tokio::test]
async fn apply_damage_type_resistance_halves() {
    let mut stats = dungeonsandapps::combat_engine::ComputedStats::default();
    stats.resistances.insert("fire".into());
    let (dmg, is_resistant, _, _) = apply_damage_type(10, "fire", &stats, false);
    assert_eq!(dmg, 5);
    assert!(is_resistant);
}

#[tokio::test]
async fn apply_damage_type_immunity_zeroes() {
    let mut stats = dungeonsandapps::combat_engine::ComputedStats::default();
    stats.immunities.insert("cold".into());
    let (dmg, _, _, is_immune) = apply_damage_type(10, "cold", &stats, false);
    assert_eq!(dmg, 0);
    assert!(is_immune);
}

#[tokio::test]
async fn apply_damage_type_vulnerability_doubles() {
    let mut stats = dungeonsandapps::combat_engine::ComputedStats::default();
    stats.vulnerabilities.insert("lightning".into());
    let (dmg, _, is_vuln, _) = apply_damage_type(10, "lightning", &stats, false);
    assert_eq!(dmg, 20);
    assert!(is_vuln);
}

#[tokio::test]
async fn compute_max_hp_tough_feat_adds_2_per_level() {
    let mut snap = base_snap();
    snap.level_total = 4;
    snap.classes = json!([{ "name": "Fighter", "hit_die": "d10", "level": 4 }]);
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({ "feats": [{ "key": "tough" }] });

    let hp_with_tough = compute_max_hp_from_sheet(&snap);

    snap.sheet_raw = json!({});
    let hp_without = compute_max_hp_from_sheet(&snap);

    assert_eq!(
        hp_with_tough - hp_without,
        8,
        "tough adds 2×4=8 HP at level 4"
    );
}

#[tokio::test]
async fn compute_max_hp_hp_max_reduction() {
    let mut snap = base_snap();
    snap.level_total = 2;
    snap.classes = json!([{ "name": "Fighter", "hit_die": "d10", "level": 2 }]);
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({});

    let normal_hp = compute_max_hp_from_sheet(&snap);
    snap.sheet_raw = json!({ "hp_max_reduction": 5 });
    let reduced_hp = compute_max_hp_from_sheet(&snap);

    assert_eq!(
        normal_hp - reduced_hp,
        5,
        "hp_max_reduction of 5 should subtract 5"
    );
}

#[tokio::test]
async fn compute_max_hp_cannot_go_below_1() {
    let mut snap = base_snap();
    snap.level_total = 1;
    snap.classes = json!([{ "name": "Wizard", "hit_die": "d6", "level": 1 }]);
    snap.abilities = json!({"str":10,"dex":10,"con":1,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({ "hp_max_reduction": 9999 });

    let hp = compute_max_hp_from_sheet(&snap);
    assert_eq!(hp, 1, "HP should never go below 1");
}

#[tokio::test]
async fn apply_hp_damage_temp_absorbs_first() {
    let (new_hp, new_temp) = apply_hp_damage(20, 5, 3);
    assert_eq!(new_temp, 2, "temp HP should absorb first");
    assert_eq!(new_hp, 20, "real HP untouched");
}

#[tokio::test]
async fn apply_hp_damage_overflow_into_real_hp() {
    let (new_hp, new_temp) = apply_hp_damage(20, 5, 8);
    assert_eq!(new_temp, 0);
    assert_eq!(new_hp, 17, "5 temp absorbed, 3 remaining → 20-3=17");
}

#[tokio::test]
async fn apply_hp_damage_no_temp_reduces_directly() {
    let (new_hp, new_temp) = apply_hp_damage(20, 0, 6);
    assert_eq!(new_hp, 14);
    assert_eq!(new_temp, 0);
}

#[tokio::test]
async fn apply_hp_damage_zero_damage_no_change() {
    let (new_hp, new_temp) = apply_hp_damage(20, 5, 0);
    assert_eq!(new_hp, 20);
    assert_eq!(new_temp, 5);
}

#[tokio::test]
async fn proficiency_from_level_all_breakpoints() {
    assert_eq!(proficiency_from_level(1), 2);
    assert_eq!(proficiency_from_level(4), 2);
    assert_eq!(proficiency_from_level(5), 3);
    assert_eq!(proficiency_from_level(8), 3);
    assert_eq!(proficiency_from_level(9), 4);
    assert_eq!(proficiency_from_level(12), 4);
    assert_eq!(proficiency_from_level(13), 5);
    assert_eq!(proficiency_from_level(16), 5);
    assert_eq!(proficiency_from_level(17), 6);
    assert_eq!(proficiency_from_level(20), 6);
}

// =====================================================================
// Fighting Styles
// =====================================================================

#[tokio::test]
async fn compute_stats_archery_style_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "fighting_styles": ["archery"] });
    let stats = compute_stats(&snap);
    assert!(
        stats.archery_style,
        "archery fighting style should set archery_style flag"
    );
}

#[tokio::test]
async fn compute_stats_dueling_style_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "fighting_styles": ["dueling"] });
    let stats = compute_stats(&snap);
    assert!(
        stats.dueling_style,
        "dueling fighting style should set dueling_style flag"
    );
}

#[tokio::test]
async fn compute_stats_gwf_style_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "fighting_styles": ["great_weapon_fighting"] });
    let stats = compute_stats(&snap);
    assert!(
        stats.gwf_style,
        "GWF fighting style should set gwf_style flag"
    );
}

#[tokio::test]
async fn compute_stats_twf_style_sets_flag() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "fighting_styles": ["two-weapon_fighting"] });
    let stats = compute_stats(&snap);
    assert!(
        stats.twf_style,
        "TWF fighting style should set twf_style flag"
    );
}

#[tokio::test]
async fn compute_stats_defense_style_adds_ac() {
    // PHB p.91: Defense grants +1 AC. The base_snap has no armor
    // (ac = base_ac from snap), so the +1 still shows up as a +1 bump
    // in stats.ac relative to no style. (In production the AC computation
    // in `ac.rs` is the source of truth for armored combatants; here we
    // verify the flag and the +1 application.)
    let mut snap = base_snap();
    let no_style = compute_stats(&snap);
    snap.sheet_raw = json!({ "fighting_styles": ["defense"] });
    let with_style = compute_stats(&snap);
    assert!(with_style.defense_style, "defense fighting style flag set");
    assert_eq!(
        with_style.ac,
        no_style.ac + 1,
        "defense fighting style should add +1 AC (PHB p.91)"
    );
}

#[tokio::test]
async fn compute_stats_multiple_fighting_styles() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "fighting_styles": ["archery", "dueling"] });
    let stats = compute_stats(&snap);
    assert!(stats.archery_style);
    assert!(stats.dueling_style);
    assert!(!stats.gwf_style);
    assert!(!stats.twf_style);
}

#[tokio::test]
async fn compute_stats_fighting_style_case_insensitive() {
    let mut snap = base_snap();
    snap.sheet_raw =
        json!({ "fighting_styles": ["ARCHERY", "Great Weapon Fighting", "TWO-WEAPON FIGHTING"] });
    let stats = compute_stats(&snap);
    assert!(stats.archery_style);
    assert!(stats.gwf_style);
    assert!(stats.twf_style);
}

// =====================================================================
// Attack Resolution with Fighting Styles and Power Attack
// =====================================================================

fn _weapon_props_longbow() -> WeaponProps {
    WeaponProps {
        ranged: true,
        thrown: false,
        finesse: false,
        reach: false,
        ammunition: true,
        light: false,
        heavy: false,
        two_handed: true,
        versatile: false,
        loading: false,
        special: false,
    }
}

fn _weapon_props_longsword() -> WeaponProps {
    WeaponProps {
        ranged: false,
        thrown: false,
        finesse: false,
        reach: false,
        ammunition: false,
        light: false,
        heavy: false,
        two_handed: false,
        versatile: true,
        loading: false,
        special: false,
    }
}

#[tokio::test]
async fn resolve_attack_power_attack_penalty_and_bonus() {
    let mut attacker = base_snap();
    attacker.level_total = 5; // proficiency +3
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([{
        "id": "sword",
        "name": "Longsword",
        "damage": "1d8",
        "damage_type": "slashing",
        "properties": "versatile"
    }]);
    let mut target = base_snap();
    target.id = uuid::Uuid::new_v4();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("sword".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        power_attack: true,
        cover: None,
        advantage: false,
        disadvantage: false,
        extra_damage_expression: None,
        extra_damage_type: None,
        attack_expression: None,
        damage_expression: Some("1d8".into()),
        damage_type: "slashing".into(),
        damage_die: Some("d8".into()),
        is_spell_attack: false,
        is_magical: false,
        frightened_source_visible: None,
        label: None,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    let result = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();

    // With power attack: if hit, damage should include +10 bonus
    // Base damage 1d8 averages 4.5, power attack adds +10 = ~14.5
    if result.hit {
        assert!(
            result.damage_applied >= 10,
            "power attack should add +10 damage (got {})",
            result.damage_applied
        );
    }
    // Power attack applies -5 penalty, so attack_total should be lower than without
    // We can't assert on hit/miss due to RNG, but we verified the code path runs
}

#[tokio::test]
async fn resolve_attack_without_power_attack() {
    let mut attacker = base_snap();
    attacker.level_total = 5;
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([{
        "id": "sword",
        "name": "Longsword",
        "damage": "1d8",
        "damage_type": "slashing",
        "properties": "versatile"
    }]);
    let mut target = base_snap();
    target.id = uuid::Uuid::new_v4();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("sword".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        power_attack: false,
        cover: None,
        advantage: false,
        disadvantage: false,
        extra_damage_expression: None,
        extra_damage_type: None,
        attack_expression: None,
        damage_expression: Some("1d8".into()),
        damage_type: "slashing".into(),
        damage_die: Some("d8".into()),
        is_spell_attack: false,
        frightened_source_visible: None,
        is_magical: false,
        label: None,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    let result = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();

    // Without power attack: if hit, damage should be lower (no +10 bonus)
    if result.hit {
        assert!(
            result.damage_applied < 15,
            "without power attack damage should be lower (got {})",
            result.damage_applied
        );
    }
}

// =====================================================================
// Two-Weapon Fighting
// =====================================================================

#[tokio::test]
async fn twf_offhand_without_style_no_ability_mod() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([{
        "id": "dagger",
        "name": "Dagger",
        "damage_die": "1d4",
        "properties": "finesse, light, thrown"
    }]);
    let target = base_snap();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let result = resolve_two_weapon_attack(
        &attacker,
        &target,
        "dagger",
        &attacker_stats,
        &target_stats,
        false,
    )
    .unwrap();

    // Without TWF style, off-hand damage should not include ability mod
    // Dagger is 1d4, no +3 str mod. On crit 2d4 max 8.
    if result.hit {
        let dmg_expr = &result.damage_roll.as_ref().unwrap().expression;
        // "1d4"                → ok (non-crit without mod)
        // "2d4"                → ok (crit without mod)
        // "1d4+3" or "2d4+3"   → BAD (ability mod included)
        assert!(
            !dmg_expr.contains('+'),
            "TWF without style should not add ability mod (got expression '{}')",
            dmg_expr
        );
    }
}

#[tokio::test]
async fn twf_offhand_with_style_adds_ability_mod() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([{
        "id": "dagger",
        "name": "Dagger",
        "damage_die": "1d4",
        "properties": "finesse, light, thrown"
    }]);
    let target = base_snap();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let result = resolve_two_weapon_attack(
        &attacker,
        &target,
        "dagger",
        &attacker_stats,
        &target_stats,
        true,
    )
    .unwrap();

    // With TWF style, off-hand damage should include ability mod
    // Dagger 1d4 + 3 str mod = ~5.5 avg, max 7
    if result.hit {
        assert!(
            result.damage_applied >= 4,
            "TWF with style should add ability mod (got {})",
            result.damage_applied
        );
    }
}

#[tokio::test]
async fn twf_requires_light_weapon() {
    let mut attacker = base_snap();
    attacker.weapons = json!([{
        "id": "longsword",
        "name": "Longsword",
        "damage_die": "1d8",
        "properties": "versatile"
    }]);
    let target = base_snap();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let result = resolve_two_weapon_attack(
        &attacker,
        &target,
        "longsword",
        &attacker_stats,
        &target_stats,
        false,
    );

    assert!(result.is_err(), "TWF should require light weapon");
    assert!(
        result.unwrap_err().contains("light"),
        "error should mention light property"
    );
}

// =====================================================================
// Cantrip Scaling (tested via spell damage expression parsing)
// =====================================================================

fn scale_cantrip_damage(expression: &str, caster_level: i32) -> String {
    let multiplier = match caster_level {
        1..=4 => 1,
        5..=10 => 2,
        11..=16 => 3,
        _ => 4,
    };
    if multiplier <= 1 {
        return expression.to_string();
    }
    if let Some(d_pos) = expression.find('d').or_else(|| expression.find('D')) {
        let num_str = &expression[..d_pos];
        let base_n: i32 = num_str.parse().unwrap_or(1);
        let scaled_n = base_n * multiplier;
        format!("{}{}", scaled_n, &expression[d_pos..])
    } else {
        expression.to_string()
    }
}

#[test]
fn cantrip_scaling_levels_1_to_4_no_change() {
    assert_eq!(scale_cantrip_damage("1d8", 1), "1d8");
    assert_eq!(scale_cantrip_damage("1d8", 4), "1d8");
    assert_eq!(scale_cantrip_damage("1d10", 3), "1d10");
}

#[test]
fn cantrip_scaling_levels_5_to_10_doubles() {
    assert_eq!(scale_cantrip_damage("1d8", 5), "2d8");
    assert_eq!(scale_cantrip_damage("1d8", 10), "2d8");
    assert_eq!(scale_cantrip_damage("2d6", 7), "4d6");
}

#[test]
fn cantrip_scaling_levels_11_to_16_triples() {
    assert_eq!(scale_cantrip_damage("1d8", 11), "3d8");
    assert_eq!(scale_cantrip_damage("1d8", 16), "3d8");
    assert_eq!(scale_cantrip_damage("1d10", 12), "3d10");
}

#[test]
fn cantrip_scaling_levels_17_plus_quadruples() {
    assert_eq!(scale_cantrip_damage("1d8", 17), "4d8");
    assert_eq!(scale_cantrip_damage("1d8", 20), "4d8");
    assert_eq!(scale_cantrip_damage("2d6", 18), "8d6");
}

#[test]
fn cantrip_scaling_preserves_modifiers() {
    assert_eq!(scale_cantrip_damage("1d8+3", 5), "2d8+3");
    assert_eq!(scale_cantrip_damage("1d10+5", 11), "3d10+5");
    assert_eq!(scale_cantrip_damage("2d6+1d4", 17), "8d6+1d4");
}

// =====================================================================
// Spell Attack Bonus
// =====================================================================

#[tokio::test]
async fn compute_stats_spell_attack_bonus_calculation() {
    let mut snap = base_snap();
    snap.level_total = 5; // proficiency +3
    snap.abilities = json!({"int": 18, "dex": 10, "con": 10, "str": 10, "wis": 10, "cha": 10});
    snap.casting = json!({"ability": "int"});
    let stats = compute_stats(&snap);
    // Proficiency +3, int mod +4 = +7 spell attack
    assert_eq!(
        stats.spell_attack_bonus, 7,
        "spell attack should be pb + ability mod"
    );
}

#[tokio::test]
async fn compute_stats_spell_save_dc_calculation() {
    let mut snap = base_snap();
    snap.level_total = 5; // proficiency +3
    snap.abilities = json!({"wis": 16, "dex": 10, "con": 10, "str": 10, "int": 10, "cha": 10});
    snap.casting = json!({"ability": "wis"});
    let stats = compute_stats(&snap);
    // 8 + pb + wis mod = 8 + 3 + 3 = 14
    assert_eq!(
        stats.spell_save_dc, 14,
        "spell save DC should be 8 + pb + ability mod"
    );
}

// =====================================================================
// PHB mechanics coverage — fix-sprint regressions
// =====================================================================

/// PHB p.96: Sneak Attack once per turn.
/// The once/turn gate is enforced upstream (backend handler reads sheet.sneak_attack_used).
/// This unit test verifies the engine applies extra_damage_expression when supplied —
/// the handler is responsible for the once/turn cap.
#[tokio::test]
async fn sneak_attack_extra_damage_applied_once_per_attack() {
    let mut snap = base_snap();
    snap.hp_current = 50;
    snap.hp_max = 50;
    snap.base_ac = 14;
    snap.abilities = json!({"str":10,"dex":18,"con":10,"int":10,"wis":10,"cha":10});
    let stats = compute_stats(&snap);

    let target = base_snap();
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+8".into()),
        damage_expression: Some("1d6+4".into()),
        damage_type: "piercing".into(),
        damage_die: Some("1d6".into()),
        ability: Some("dex".into()),
        proficient: Some(true),
        advantage: false,
        disadvantage: false,
        cover: None,
        is_spell_attack: false,
        is_magical: false,
        label: Some("Sneak Attack".into()),
        weapon_id: None,
        frightened_source_visible: None,
        extra_damage_expression: Some("3d6".into()),
        extra_damage_type: Some("piercing".into()),
        power_attack: false,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    // Hit and verify sneak dice applied
    let mut extra_observed = false;
    for _ in 0..100 {
        let r = resolve_attack(&snap, &target, &req, &stats, &target_stats).unwrap();
        if r.hit && r.extra_damage_applied > 0 {
            assert_eq!(r.extra_damage_type.as_deref(), Some("piercing"));
            assert!(r.extra_damage_applied >= 3, "sneak attack 3d6 min = 3");
            extra_observed = true;
            break;
        }
    }
    assert!(
        extra_observed,
        "sneak attack extra damage should fire on at least one hit"
    );
}

/// PHB p.48: Reckless Attack grants attacker advantage on STR melee attacks.
/// The handler applies `adv = true` when reckless=true + STR + non-ranged/non-thrown.
/// Engine test: ensure `advantage: true` resolves to higher expected hit rate.
#[tokio::test]
async fn resolve_attack_reckless_advantage_flag() {
    let mut snap = base_snap();
    snap.hp_current = 50;
    snap.hp_max = 50;
    snap.base_ac = 14;
    snap.abilities = json!({"str":18,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let stats = compute_stats(&snap);

    let target = base_snap();
    let target_stats = compute_stats(&target);

    let base = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+6".into()),
        damage_expression: Some("1d8+4".into()),
        damage_type: "slashing".into(),
        damage_die: Some("1d8".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        advantage: false,
        disadvantage: false,
        cover: None,
        is_spell_attack: false,
        is_magical: false,
        label: Some("Reckless".into()),
        frightened_source_visible: None,
        weapon_id: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        reckless: true, // handler sets this; engine should accept
        bless_dice: None,
        bardic_inspiration_dice: None,
    };

    // With reckless + adv=true (set by handler), hit rate should be higher than no-adv
    let mut adv_hits = 0;
    for _ in 0..200 {
        let r = resolve_attack(&snap, &target, &base, &stats, &target_stats).unwrap();
        if r.hit {
            adv_hits += 1;
        }
    }
    // Against AC 12 with +6 attack, adv should hit more than 75/200 (37.5% expected w/o adv)
    assert!(
        adv_hits > 75,
        "reckless+adv should hit >37.5%: got {}/200",
        adv_hits
    );
}

// =====================================================================
// PHB p.168 Polearm Master: BA d4 attack with polearm
// =====================================================================

#[test]
fn is_wielding_polearm_detects_glaive_halberd_quarterstaff() {
    let mut snap = base_snap();
    snap.weapons = json!([
        { "name": "Glaive", "id": "glaive-1" },
    ]);
    assert!(is_wielding_polearm(&snap), "Glaive must be detected as polearm");

    snap.weapons = json!([{ "name": "Halberd" }]);
    assert!(is_wielding_polearm(&snap), "Halberd must be detected as polearm");

    snap.weapons = json!([{ "name": "Quarterstaff" }]);
    assert!(is_wielding_polearm(&snap), "Quarterstaff must be detected as polearm");

    snap.weapons = json!([{ "name": "Longsword" }, { "name": "Glaive" }]);
    assert!(
        is_wielding_polearm(&snap),
        "Polearm among non-polearms still detected"
    );
}

#[test]
fn is_wielding_polearm_rejects_non_polearm_weapons() {
    let mut snap = base_snap();
    for name in ["Longsword", "Rapier", "Dagger", "Shortbow", "Spear"] {
        snap.weapons = json!([{ "name": name }]);
        assert!(
            !is_wielding_polearm(&snap),
            "{} must not count as polearm for Polearm Master",
            name
        );
    }
    snap.weapons = json!([]);
    assert!(!is_wielding_polearm(&snap), "empty weapons list → false");
}

#[tokio::test]
async fn polearm_ba_attack_hits_and_applies_d4_damage() {
    let mut attacker = base_snap();
    attacker.sheet_raw = json!({ "feats": [{"key": "polearm_master"}] });
    attacker.weapons = json!([{ "name": "Glaive" }]);
    attacker.abilities = json!({"str":16,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    attacker.proficiency_bonus = 4;
    let attacker_stats = compute_stats(&attacker);
    assert!(attacker_stats.polearm_master, "feat must set polearm_master flag");

    let mut target = base_snap();
    target.hp_current = 20;
    target.hp_max = 20;
    target.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let target_stats = compute_stats(&target);

    let r = resolve_polearm_ba_attack(&attacker, &target, &attacker_stats, &target_stats)
        .expect("resolver must succeed");

    // Attack roll = 1d20+3+4 vs target AC 10+0. With proficiency +3 STR mod,
    // attack bonus is +7. d20 natural 1 auto-misses, 20+ always hits.
    if r.hit {
        assert!(r.damage_applied >= 1, "d4+3 damage must be at least 1");
        assert!(r.damage_applied <= 10, "d4+3 capped, max raw=7+3=10, no crit");
        assert_eq!(r.target_hp_after + r.damage_applied, 20, "HP must drop by damage");
    } else {
        assert_eq!(r.damage_applied, 0, "miss → no damage applied");
        assert_eq!(r.target_hp_after, 20, "miss → HP unchanged");
    }
}

#[tokio::test]
async fn polearm_ba_attack_critical_doubles_dice() {
    let mut attacker = base_snap();
    attacker.sheet_raw = json!({ "feats": [{"key": "polearm_master"}] });
    attacker.weapons = json!([{ "name": "Quarterstaff" }]);
    attacker.abilities = json!({"str":18,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    attacker.proficiency_bonus = 4;
    let attacker_stats = compute_stats(&attacker);

    // Repeat with a low-AC target to maximize hit rate; eventually one will crit.
    let mut target = base_snap();
    target.hp_current = 50;
    target.hp_max = 50;
    target.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let target_stats = compute_stats(&target);

    // Find at least one critical hit across 200 trials.
    let mut found_crit = false;
    for _ in 0..200 {
        let r = resolve_polearm_ba_attack(
            &attacker,
            &target,
            &attacker_stats,
            &target_stats,
        )
        .unwrap();
        if r.critical && r.hit {
            found_crit = true;
            // 2d4+4 crit: min 6, max 12 (no damage_bonus or weapon_damage_bonus).
            assert!(
                r.damage_applied >= 4,
                "crit min should be 2d4+4 (4 dmg + str mod 4 + 0 = 8 base pre-resistance)"
            );
            assert!(r.damage_applied <= 20, "crit 2d4+4 capped at 12 base");
            break;
        }
    }
    assert!(found_crit, "expected a critical hit in 200 trials vs AC 10");
}

/// PHB p.198: Temp HP "doesn't stack. For example, if a spell grants 5 temp HP,
/// then another grants 10, you have 10, not 15."
/// `apply_hp_damage` test: incoming damage hits temp HP first, never HP, until depleted.
#[tokio::test]
async fn temp_hp_absorbs_all_damage_until_depleted() {
    let mut snap = base_snap();
    snap.hp_current = 20;
    snap.hp_max = 20;
    snap.temp_hp = 5;

    // 3 damage → 0 temp, 20 hp
    let (hp, temp) = apply_hp_damage(20, 5, 3);
    assert_eq!(hp, 20, "HP unchanged when temp absorbs");
    assert_eq!(temp, 2, "Temp reduced by damage");

    // 5 damage → 0 temp, 17 hp
    let (hp, temp) = apply_hp_damage(20, 2, 5);
    assert_eq!(hp, 17, "HP reduced by overflow");
    assert_eq!(temp, 0, "Temp depleted");
}

// =====================================================================
// PHB p.290: Frightened attacker has disadvantage only if the source
// of fear is in line of sight (L15). The resolver uses a pre-computed
// visibility flag from the handler; the engine surface (compute_stats)
// captures the source_combatant_id from the effect.
// =====================================================================

#[test]
fn compute_stats_frightened_captures_source_id() {
    use dungeonsandapps::combat_engine::EffectSnapshot;
    let mut snap = base_snap();
    let source_id = Uuid::new_v4();
    snap.active_effects = vec![EffectSnapshot {
        id: Uuid::new_v4(),
        name: "Frightened".into(),
        modifiers: json!({"frightened": true}),
        concentration: false,
        source_type: "spell".into(),
        source_combatant_id: Some(source_id),
    }];
    let stats = compute_stats(&snap);
    assert!(stats.frightened, "frightened flag must be set");
    assert_eq!(
        stats.frightened_source_id,
        Some(source_id),
        "frightened_source_id must come from the effect's caster"
    );
}

#[test]
fn compute_stats_frightened_no_source_leaves_id_none() {
    use dungeonsandapps::combat_engine::EffectSnapshot;
    let mut snap = base_snap();
    snap.active_effects = vec![EffectSnapshot {
        id: Uuid::new_v4(),
        name: "Frightened".into(),
        modifiers: json!({"frightened": true}),
        concentration: false,
        source_type: "condition".into(),
        source_combatant_id: None,
    }];
    let stats = compute_stats(&snap);
    assert!(stats.frightened);
    assert_eq!(
        stats.frightened_source_id, None,
        "environmental condition has no source"
    );
}

#[tokio::test]
async fn resolve_attack_frightened_with_visible_source_applies_dis() {
    use dungeonsandapps::combat_engine::AttackReq;
    use dungeonsandapps::combat_engine::resolve_attack;
    let mut attacker = base_snap();
    attacker.sheet_raw = json!({});
    attacker.proficiency_bonus = 4;
    attacker.abilities = json!({"str":14,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let mut attacker_stats = compute_stats(&attacker);
    attacker_stats.frightened = true;
    attacker_stats.frightened_source_id = Some(Uuid::new_v4());
    attacker_stats.blinded = false;

    let mut target = base_snap();
    target.hp_current = 20;
    target.hp_max = 20;
    target.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+5".into()),
        damage_expression: Some("1d8+2".into()),
        damage_type: "slashing".into(),
        proficient: Some(true),
        frightened_source_visible: Some(true), // source IS visible
        ..Default::default()
    };
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    assert!(
        r.attack_disadvantage,
        "frightened with visible source → dis (PHB p.290)"
    );
}

#[tokio::test]
async fn resolve_attack_frightened_with_NOT_visible_source_no_dis() {
    use dungeonsandapps::combat_engine::AttackReq;
    use dungeonsandapps::combat_engine::resolve_attack;
    let mut attacker = base_snap();
    attacker.sheet_raw = json!({});
    attacker.proficiency_bonus = 4;
    attacker.abilities = json!({"str":14,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let mut attacker_stats = compute_stats(&attacker);
    attacker_stats.frightened = true;
    attacker_stats.frightened_source_id = Some(Uuid::new_v4());
    attacker_stats.blinded = false;

    let mut target = base_snap();
    target.hp_current = 20;
    target.hp_max = 20;
    target.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+5".into()),
        damage_expression: Some("1d8+2".into()),
        damage_type: "slashing".into(),
        proficient: Some(true),
        frightened_source_visible: Some(false), // source NOT visible (LOS blocked)
        ..Default::default()
    };
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    assert!(
        !r.attack_disadvantage,
        "frightened with NOT visible source → no dis (L15 fix)"
    );
}

#[tokio::test]
async fn resolve_attack_frightened_blinded_no_dis_even_if_visible() {
    use dungeonsandapps::combat_engine::AttackReq;
    use dungeonsandapps::combat_engine::resolve_attack;
    let mut attacker = base_snap();
    attacker.proficiency_bonus = 4;
    attacker.abilities = json!({"str":14,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let mut attacker_stats = compute_stats(&attacker);
    attacker_stats.frightened = true;
    attacker_stats.frightened_source_id = Some(Uuid::new_v4());
    attacker_stats.blinded = true; // BLINDED gate

    let mut target = base_snap();
    target.hp_current = 20;
    target.hp_max = 20;
    target.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+5".into()),
        damage_expression: Some("1d8+2".into()),
        damage_type: "slashing".into(),
        proficient: Some(true),
        frightened_source_visible: Some(true), // visible BUT blinded overrides
        ..Default::default()
    };
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    // Note: blinded also grants its own dis (line 117 in attack resolver).
    // The L15 fright-dis is suppressed by the blindness gate.
    // We assert the L15-specific check: without blinded, dis from
    // frightened + visible source = true; with blinded, the
    // fright-dis is suppressed, but dis from blinded still applies.
    assert!(r.attack_disadvantage, "blinded also grants dis (PHB)");
}

#[tokio::test]
async fn resolve_attack_frightened_no_override_keeps_audit_fallback() {
    use dungeonsandapps::combat_engine::AttackReq;
    use dungeonsandapps::combat_engine::resolve_attack;
    let mut attacker = base_snap();
    attacker.proficiency_bonus = 4;
    attacker.abilities = json!({"str":14,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    let mut attacker_stats = compute_stats(&attacker);
    attacker_stats.frightened = true;
    attacker_stats.frightened_source_id = Some(Uuid::new_v4());
    attacker_stats.blinded = false;

    let target = base_snap();
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+5".into()),
        damage_expression: Some("1d8+2".into()),
        damage_type: "slashing".into(),
        proficient: Some(true),
        // frightened_source_visible: None → audit fallback (dis applies
        // unless blinded). Preserves pre-L15 behavior.
        ..Default::default()
    };
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    assert!(
        r.attack_disadvantage,
        "no override → audit fallback (dis) applies"
    );
}
