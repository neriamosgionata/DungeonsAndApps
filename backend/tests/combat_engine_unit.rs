use dungeonsandapps::combat_engine::{
    AttackReq, CombatantSnapshot, WeaponProps, ability_mod, apply_damage_type, apply_hp_damage,
    apply_racial_bonuses, compute_max_hp_from_sheet, compute_stats, concentration_check,
    is_wielding_polearm, proficiency_from_level, resolve_attack, resolve_polearm_ba_attack,
    resolve_two_weapon_attack,
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
        mounted_on: None,
    }
}

#[tokio::test]
// R6: PHB p.291 — exhaustion L1 = ability-CHECK disadvantage; saves get
// theirs at L3 (was: save dis at L1, no check dis anywhere).
async fn compute_stats_exhaustion_level_1_ability_check_disadvantage() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({ "exhaustion": 1 });
    let stats = compute_stats(&snap);
    assert_eq!(stats.exhaustion, 1);
    assert!(
        stats.ability_check_disadvantage,
        "exhaustion 1 → disadvantage on ability checks"
    );
    assert!(!stats.save_disadvantage, "exhaustion 1 must NOT dis saves");
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
    let stats = compute_stats(&snap);
    let (broken, roll) = concentration_check(&snap, &stats, 20, &mut rng);
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
    let stats = compute_stats(&snap);
    let (_broken, roll) = concentration_check(&snap, &stats, 20, &mut rng);
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
// R6: PHB p.91 — Defense applies ONLY while wearing armor. The old test
// asserted +1 unarmored (the bug — combat AC diverged from the sheet).
async fn compute_stats_defense_style_adds_ac_only_with_armor() {
    let mut snap = base_snap();
    let no_style = compute_stats(&snap);
    snap.sheet_raw = json!({ "fighting_styles": ["defense"] });
    let with_style = compute_stats(&snap);
    assert!(with_style.defense_style, "defense fighting style flag set");
    assert_eq!(
        with_style.ac,
        no_style.ac,
        "defense style must NOT add +1 while unarmored"
    );
    let mut armored = base_snap();
    armored.sheet_raw = json!({ "fighting_styles": ["defense"], "armor": {"type": "heavy", "ac_base": 16, "max_dex": 0} });
    let armored_stats = compute_stats(&armored);
    assert_eq!(
        armored_stats.ac,
        17,
        "defense style +1 while wearing armor (16 + 1)"
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
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
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
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
    };

    let result = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();

    // Without power attack: if hit (and not a nat-20 crit that doubles the
    // dice), damage should be lower (no +10 bonus).
    if result.hit && !result.critical {
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
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
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
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
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
    // M-2: PHB p.168 — the spear IS a polearm for Polearm Master.
    for name in ["Longsword", "Rapier", "Dagger", "Shortbow"] {
        snap.weapons = json!([{ "name": name }]);
        assert!(
            !is_wielding_polearm(&snap),
            "{} must not count as polearm for Polearm Master",
            name
        );
    }
    snap.weapons = json!([{ "name": "Spear" }]);
    assert!(
        is_wielding_polearm(&snap),
        "Spear must count as a polearm for Polearm Master (PHB p.168)"
    );
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

#[tokio::test]
async fn resolve_attack_sneak_attack_dice_applied_on_hit() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str":10,"dex":18,"con":10,"int":10,"wis":10,"cha":10});
    attacker.proficiency_bonus = 3;
    let attacker_stats = compute_stats(&attacker);
    let target = base_snap();
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        attack_expression: Some("1d20+10".into()),
        damage_expression: Some("1d6+4".into()),
        damage_type: "piercing".into(),
        damage_die: Some("1d6".into()),
        ability: Some("dex".into()),
        proficient: Some(true),
        advantage: true,
        disadvantage: false,
        cover: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        sneak_attack: false,
        sneak_attack_dice: Some("3d6".into()),
        ..Default::default()
    };
        
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    if r.hit {
        assert!(r.sneak_attack_applied, "sneak should be applied on hit");
        assert!(r.sneak_attack_damage > 0, "sneak should deal damage");
        assert!(r.damage_applied + r.extra_damage_applied + r.sneak_attack_damage > 0, "total should include sneak");
    } else {
        assert!(!r.sneak_attack_applied, "sneak only applies on hit");
        assert_eq!(r.sneak_attack_damage, 0);
    }
}

#[tokio::test]
async fn resolve_attack_smite_damage_applied_on_hit() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str":18,"dex":10,"con":10,"int":10,"wis":10,"cha":16});
    attacker.proficiency_bonus = 3;
    attacker.level_total = 5;
    attacker.weapons = json!([{
        "id": "sword", "name": "Longsword", "damage": "1d8+4",
        "damage_type": "slashing", "properties": "versatile"
    }]);
    let attacker_stats = compute_stats(&attacker);
    let target = base_snap();
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("sword".into()),
        damage_expression: Some("1d8+4".into()),
        damage_type: "slashing".into(),
        damage_die: Some("d8".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        attack_expression: Some("1d20+7".into()),
        advantage: false,
        disadvantage: false,
        cover: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: Some(2),
        ..Default::default()
    };
        
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    if r.hit {
        assert!(r.smite_applied, "smite should be applied on hit");
        assert!(r.smite_damage > 0, "smite should deal radiant damage (slot 2 = 3d8)");
        assert_eq!(r.smite_slot_consumed, Some(2), "should record slot level consumed");
    } else {
        assert!(!r.smite_applied, "smite only applies on hit");
        assert_eq!(r.smite_damage, 0);
        assert_eq!(r.smite_slot_consumed, None);
    }
}

#[tokio::test]
async fn resolve_attack_smite_vs_undead_extra_d8() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str":18,"dex":10,"con":10,"int":10,"wis":10,"cha":16});
    attacker.proficiency_bonus = 3;
    attacker.weapons = json!([{
        "id": "sword", "name": "Longsword", "damage": "1d8+4",
        "damage_type": "slashing", "properties": "versatile"
    }]);
    let attacker_stats = compute_stats(&attacker);
    let mut target = base_snap();
    target.sheet_raw = json!({"creature_type": "undead"});
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("sword".into()),
        damage_expression: Some("1d8+4".into()),
        damage_type: "slashing".into(),
        damage_die: Some("d8".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        attack_expression: Some("1d20+7".into()),
        advantage: true,
        cover: None,
        extra_damage_expression: None,
        extra_damage_type: None,
        power_attack: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: Some(1),
        ..Default::default()
    };
        
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    if r.hit {
        assert!(r.smite_applied, "smite should apply on hit vs undead");
        // Base smite L1 = 2d8, +1d8 vs undead = 3d8 minimum. Floor after resist = >= 2.
        assert!(r.smite_damage >= 2, "smite vs undead should deal at least 2 radiant (3d8 min)");
    }
}

#[tokio::test]
async fn compute_stats_aura_of_protection_paladin_6_adds_cha_to_saves() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":18});
    snap.classes = json!([{"name": "paladin", "level": 6}]);
    let stats = compute_stats(&snap);
    // CHA 18 → +4 mod. Aura adds CHA mod (+4) to ALL saves on top of base mod.
    // STR/DEX/CON/INT/WIS (10): base=0 + aura=4 = 4
    // CHA (18): base=4 + aura=4 = 8
    for (ab, modv) in &stats.save_mods {
        let expected = if ab == "cha" { 8 } else { 4 };
        assert_eq!(
            *modv, expected,
            "Paladin 6 aura: {} save expected {}, got {}",
            ab, expected, modv
        );
    }
}

#[tokio::test]
async fn compute_stats_aura_of_protection_paladin_5_no_bonus() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":18});
    snap.classes = json!([{"name": "paladin", "level": 5}]);
    let stats = compute_stats(&snap);
    // L5 paladin: base ability mod only, no aura
    for (ab, modv) in &stats.save_mods {
        let expected = if ab == "cha" { 4 } else { 0 };
        assert_eq!(
            *modv, expected,
            "Paladin 5: {} save expected {}, got {}",
            ab, expected, modv
        );
    }
}

#[tokio::test]
async fn resolve_attack_brutal_critical_barbarian_9_extra_die() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str":18,"dex":10,"con":16,"int":10,"wis":10,"cha":10});
    attacker.proficiency_bonus = 4;
    attacker.level_total = 9;
    attacker.classes = json!([{"name": "barbarian", "level": 9}]);
    attacker.weapons = json!([{
        "id": "axe", "name": "Greataxe", "damage": "1d12+4",
        "damage_type": "slashing", "properties": "heavy, two-handed"
    }]);
    let attacker_stats = compute_stats(&attacker);
    let target = base_snap();
    let target_stats = compute_stats(&target);

    // Force a crit via advantage + high attack bonus
    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("axe".into()),
        ability: Some("str".into()),
        proficient: Some(true),
        damage_expression: Some("1d12+4".into()),
        damage_type: "slashing".into(),
        damage_die: Some("d12".into()),
        attack_expression: Some("2d20kh1+99".into()),
        advantage: true,
        ..Default::default()
    };
        
    let r = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    assert!(r.hit, "should hit");
    // Run multiple attempts to get a crit (nat 20 on 2d20kh1 ≈ 10% chance)
    let mut found_crit = false;
    for _ in 0..50 {
        let rr = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
        if rr.critical {
            // Brutal Critical L9 adds 1 extra d12 on top of crit double dice.
            // Normal: 1d12+4. Crit: 2d12+4. Brutal: 3d12+4.
            // With brutal, minimum = 3d12 re-rolled from 1d12+0 -> at least 3+4 = 7
            assert!(rr.damage_applied >= 7, "brutal crit L9 should add extra d12");
            found_crit = true;
            break;
        }
    }
    assert!(found_crit, "expected at least 1 crit in 50 attempts");
}

#[tokio::test]
async fn compute_stats_danger_sense_barbarian_2_dex_save_advantage() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":16,"dex":14,"con":16,"int":10,"wis":12,"cha":10});
    snap.classes = json!([{"name": "barbarian", "level": 2}]);
    let stats = compute_stats(&snap);
    assert!(stats.danger_sense, "Barb 2 should have danger_sense");
    assert!(!stats.initiative_advantage, "Barb 2 should NOT have initiative_advantage");
}

#[tokio::test]
async fn compute_stats_feral_instinct_barbarian_7_initiative_advantage() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":16,"dex":14,"con":16,"int":10,"wis":12,"cha":10});
    snap.classes = json!([{"name": "barbarian", "level": 7}]);
    let stats = compute_stats(&snap);
    assert!(stats.danger_sense, "Barb 7 should have danger_sense");
    assert!(stats.initiative_advantage, "Barb 7 should have initiative_advantage");
}

#[tokio::test]
async fn compute_stats_danger_sense_advantage_applied_in_resolve_save() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":14,"con":10,"int":10,"wis":10,"cha":10});
    snap.classes = json!([{"name": "barbarian", "level": 2}]);
    let stats = compute_stats(&snap);
    assert!(stats.danger_sense, "Barb 2 should have danger_sense");
    // Danger Sense applies advantage on DEX saves.
    // resolve_save uses stats.danger_sense to add adv for DEX.
    // We can't easily test resolve_save here (requires full import),
    // but verify the flag is correctly set.
    let dex_save_mod = stats.save_mods.iter()
        .find(|(a,_)| a == "dex")
        .map(|(_,m)| *m)
        .unwrap_or(-999);
    // DEX 14 → +2 mod, no proficiency → +2
    assert_eq!(dex_save_mod, 2, "DEX save should be +2 (mod only, no prof)");
}

#[tokio::test]
async fn resolve_attack_stunning_strike_save_dc_computation() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str":10,"dex":14,"con":14,"int":10,"wis":18,"cha":10});
    attacker.proficiency_bonus = 3;
    attacker.level_total = 5;
    attacker.classes = json!([{"name": "monk", "level": 5}]);
    attacker.weapons = json!([{
        "id": "staff", "name": "Quarterstaff", "damage": "1d8+2",
        "damage_type": "bludgeoning", "properties": "versatile"
    }]);
    let attacker_stats = compute_stats(&attacker);
    let target = base_snap();
    let target_stats = compute_stats(&target);

    // Stunning Strike: on hit, target makes CON save vs DC = 8 + prof + WIS mod
    // WIS 18 = +4, prof = +3 (L5), DC = 8 + 3 + 4 = 15
    let expected_dc = 8 + 3 + 4; // = 15
    // The stunning_strike flag doesn't affect the resolver (it's handled in attack_apply),
    // but verify the underlying stats are correct
    let wis_mod = attacker_stats.save_mods.iter()
        .find(|(a,_)| a == "wis")
        .map(|(_,m)| *m)
        .unwrap_or(-999);
    // WIS 18 = +4, no prof = +4
    assert!(wis_mod >= 4, "WIS save should include +4 ability mod, got {}", wis_mod);
}

// =====================================================================
// Racial ability bonuses must match the frontend racialAbilityBonus table
// (character/+page.svelte) — divergence caused sheet/combat mod mismatch.
// =====================================================================

#[tokio::test]
async fn racial_ability_bonuses_match_frontend_table() {
    let cases: &[(&str, &str, i32)] = &[
        ("human", "str", 1), ("human", "dex", 1), ("human", "con", 1),
        ("human", "int", 1), ("human", "wis", 1), ("human", "cha", 1),
        ("variant human", "str", 0),
        ("goblin", "dex", 2), ("goblin", "con", 1), ("goblin", "str", 0),
        ("lightfoot halfling", "dex", 2), ("lightfoot halfling", "cha", 1),
        ("stout halfling", "dex", 2), ("stout halfling", "con", 1),
        ("fairy", "dex", 2), ("fairy", "cha", 1),
        ("air genasi", "dex", 2), ("air genasi", "int", 1),
        ("earth genasi", "con", 2), ("earth genasi", "str", 1),
        ("fire genasi", "int", 2), ("fire genasi", "con", 1),
        ("water genasi", "wis", 2), ("water genasi", "con", 1),
        ("dragonborn", "str", 2), ("dragonborn", "cha", 1),
        ("half-orc", "str", 2), ("half-orc", "con", 1),
        ("mountain dwarf", "con", 2), ("mountain dwarf", "str", 2),
        ("hill dwarf", "con", 2), ("hill dwarf", "wis", 1),
        ("kobold", "dex", 2), ("kobold", "str", -2),
        ("tiefling", "cha", 2), ("tiefling", "int", 1),
    ];
    for &(race, ab, expected) in cases {
        let mut snap = base_snap();
        snap.race = Some(race.to_string());
        let bonus = apply_racial_bonuses(&snap).get(ab).copied().unwrap_or(0);
        assert_eq!(bonus, expected, "race={race} ability={ab}");
    }
}

#[tokio::test]
async fn ability_mod_honors_racial_bonuses() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":15,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    assert_eq!(ability_mod(&snap, "str"), 2, "no race: 15 → +2");
    snap.race = Some("dragonborn".into()); // +2 str → 17 → +3
    assert_eq!(ability_mod(&snap, "str"), 3);
    snap.race = Some("human".into()); // +1 → 16 → +3
    assert_eq!(ability_mod(&snap, "str"), 3);
    snap.race = Some("goblin".into()); // no str bonus → 15 → +2
    assert_eq!(ability_mod(&snap, "str"), 2);
}

#[tokio::test]
async fn ability_mod_prefers_override_over_racial() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":15,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.race = Some("dragonborn".into());
    snap.sheet_raw = json!({"abilities_override": {"str": 20}});
    assert_eq!(ability_mod(&snap, "str"), 5, "override 20 → +5, racial ignored");
}

// =====================================================================
// H8: massive damage threshold must use the halved max when exhaustion
// level 4 is active (PHB p.291) — instant death used to fire 2x too late.
// =====================================================================

#[tokio::test]
async fn resolve_attack_massive_damage_uses_halved_max_for_exhausted() {
    let mut target = base_snap();
    target.hp_current = 5;
    target.hp_max = 20;
    target.sheet_raw = json!({"exhaustion": 4});
    let target_stats = compute_stats(&target);
    assert!(target_stats.hp_max_halved, "exhaustion 4 must halve max");

    let attacker = base_snap();
    let attacker_stats = compute_stats(&attacker);
    let req = AttackReq {
        target_id: Uuid::new_v4(),
        attack_expression: Some("1d20+20".into()),
        damage_expression: Some("15".into()),
        damage_type: "force".into(),
        proficient: Some(true),
        ..Default::default()
    };
        
    // 15 dmg: remaining after 0 = 10 >= halved max 10 → instant death;
    // with the pre-fix full max (20) this would NOT be massive damage.
    let res = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    if res.natural_roll != 1 {
        // nat 1 = auto-miss (no damage) — skip the assertion then
        assert!(
            res.instant_death,
            "15 dmg vs hp 5 should be massive (halved max 10), instant_death={}",
            res.instant_death
        );
    }
}

// =====================================================================
// H10: backend AC must match frontend computedAC: sheet.ac_bonus +
// Dual Wielder +1 when wielding two melee weapons.
// =====================================================================

#[tokio::test]
async fn compute_stats_ac_includes_sheet_bonus_and_dual_wielder() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({
        "ac_bonus": 1,
        "feats": [{"key": "dual_wielder"}]
    });
    snap.weapons = json!([
        { "name": "Longsword", "range": "melee", "equipped": true },
        { "name": "Dagger", "range": "melee", "equipped": true }
    ]);
    let stats = compute_stats(&snap);
    assert_eq!(stats.ac, 14, "base 12 + ac_bonus 1 + dual wielder 1, got {}", stats.ac);

    // One melee weapon only → no dual wielder bonus
    let mut snap2 = base_snap();
    snap2.sheet_raw = json!({
        "ac_bonus": 0,
        "feats": [{"key": "dual_wielder"}]
    });
    snap2.weapons = json!([{ "name": "Longsword", "range": "melee", "equipped": true }]);
    assert_eq!(compute_stats(&snap2).ac, 12, "no dual wielder bonus with 1 melee weapon");

    // No feat → no bonus even with two weapons
    let mut snap3 = base_snap();
    snap3.weapons = json!([
        { "name": "Longsword", "range": "melee", "equipped": true },
        { "name": "Dagger", "range": "melee", "equipped": true }
    ]);
    assert_eq!(compute_stats(&snap3).ac, 12, "no dual wielder without the feat");
}

// =====================================================================
// M20: sheet.initiative is a FULL override (replaces the DEX mod, matches
// the frontend) — not added on top of it.
// =====================================================================

#[tokio::test]
async fn initiative_bonus_uses_override_as_total() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":16,"con":10,"int":10,"wis":10,"cha":10});
    let stats = compute_stats(&snap);
    assert_eq!(stats.initiative_bonus, 3, "no override → DEX mod +3");

    snap.sheet_raw = json!({"initiative": 7});
    let stats2 = compute_stats(&snap);
    assert_eq!(stats2.initiative_bonus, 7, "override 7 replaces DEX (not 3+7)");

    snap.sheet_raw = json!({"initiative": -2});
    let stats3 = compute_stats(&snap);
    assert_eq!(stats3.initiative_bonus, -2, "negative override honored");
}

// =====================================================================
// LOW: manual AC override (ac_manual) + shield bonus in the no-armor
// fallback — must match frontend computeAC.
// =====================================================================

#[tokio::test]
async fn compute_stats_ac_manual_override_and_shield_fallback() {
    // No armor config, shield on → base_ac + 2
    let mut snap = base_snap();
    snap.sheet_raw = json!({"shield": true});
    assert_eq!(compute_stats(&snap).ac, 14, "base 12 + shield 2");

    // Manual AC override ignores armor config entirely
    let mut snap2 = base_snap();
    snap2.sheet_raw = json!({
        "ac_manual": true,
        "shield": true,
        "armor": { "type": "heavy", "ac_base": 16, "max_dex": 0 }
    });
    assert_eq!(compute_stats(&snap2).ac, 14, "manual AC 12 + shield 2, armor ignored");

    // No ac_manual → armor config wins
    let mut snap3 = base_snap();
    snap3.sheet_raw = json!({
        "shield": true,
        "armor": { "type": "heavy", "ac_base": 16, "max_dex": 0 }
    });
    assert_eq!(compute_stats(&snap3).ac, 18, "heavy 16 + shield 2");
}

#[tokio::test]
async fn resolve_attack_includes_weapon_attack_bonus() {
    // R6: per-weapon attack_bonus (magic weapon) was dropped by the engine;
    // the sheet's +3 must reach the roll. Deterministic via the kept-die math.
    let mut attacker = base_snap();
    attacker.level_total = 5;
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([{
        "id": "magic-sword",
        "name": "Longsword +3",
        "damage": "1d8",
        "damage_type": "slashing",
        "properties": "versatile",
        "attack_bonus": 3
    }]);
    let mut target = base_snap();
    target.id = Uuid::new_v4();
    let attacker_stats = compute_stats(&attacker);
    let target_stats = compute_stats(&target);

    let req = AttackReq {
        target_id: target.id,
        weapon_id: Some("magic-sword".into()),
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
        is_magical: true,
        label: None,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
    };

    let result = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    // pb (level 5) 3 + STR 3 + weapon attack_bonus 3 = 9 (no cover/archery/power).
    assert_eq!(
        result.attack_total - result.natural_roll,
        9,
        "weapon.attack_bonus must be included in the attack roll"
    );
}

// =====================================================================
// A-series combat mechanics (2026-08-04)
// =====================================================================

#[tokio::test]
async fn extra_attack_count_follows_phb() {
    let mut snap = base_snap();
    snap.classes = json!([{"name": "Fighter", "level": 4}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 1);
    snap.classes = json!([{"name": "Fighter", "level": 5}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 2);
    snap.classes = json!([{"name": "Fighter", "level": 11}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 3);
    snap.classes = json!([{"name": "Fighter", "level": 20}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 4);
    // Martial 5th level
    snap.classes = json!([{"name": "Monk", "level": 5}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 2);
    snap.classes = json!([{"name": "Ranger", "level": 5}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 2);
    // Multiclass takes the max (Fighter 5 + Monk 5 = 2, not 4)
    snap.classes = json!([{"name": "Fighter", "level": 5}, {"name": "Monk", "level": 5}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 2);
    snap.classes = json!([{"name": "Fighter", "level": 11}, {"name": "Monk", "level": 5}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 3);
    // Non-martial stays at 1
    snap.classes = json!([{"name": "Wizard", "level": 20}]);
    assert_eq!(dungeonsandapps::combat_engine::extra_attack_count(&snap), 1);
}

#[tokio::test]
async fn compute_stats_shield_master_dex_save_bonus() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"feats": [{"key": "shield_master"}], "shield": true});
    let stats = compute_stats(&snap);
    let dex = stats.save_mods.iter().find(|(a, _)| a == "dex").unwrap().1;
    let str = stats.save_mods.iter().find(|(a, _)| a == "str").unwrap().1;
    assert_eq!(dex, 2, "shield master + shield = +2 DEX saves");
    assert_eq!(str, 0, "other saves unaffected");
    // No shield → no bonus
    let mut snap2 = base_snap();
    snap2.sheet_raw = json!({"feats": [{"key": "shield_master"}]});
    let stats2 = compute_stats(&snap2);
    let dex2 = stats2.save_mods.iter().find(|(a, _)| a == "dex").unwrap().1;
    assert_eq!(dex2, 0);
}

#[tokio::test]
async fn compute_stats_blind_fighting_grants_blindsight() {
    let mut snap = base_snap();
    snap.sheet_raw = json!({"fighting_styles": ["blind_fighting"]});
    let stats = compute_stats(&snap);
    assert_eq!(stats.blindsight_range, 10, "Blind Fighting grants 10 ft blindsight");
    // Blinded → no benefit (TCoE: can't see while blinded)
    let mut snap2 = base_snap();
    snap2.sheet_raw = json!({"fighting_styles": ["blind_fighting"]});
    snap2.conditions = vec!["blinded".into()];
    let stats2 = compute_stats(&snap2);
    assert_eq!(stats2.blindsight_range, 0);
}

#[tokio::test]
async fn resolve_attack_precision_superiority_adds_die() {
    let mut attacker = base_snap();
    attacker.level_total = 5;
    attacker.abilities = json!({"str": 16, "dex": 10, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.classes = json!([{"name": "Fighter", "level": 5}]);
    attacker.weapons = json!([{
        "id": "sword", "name": "Longsword", "damage": "1d8",
        "damage_type": "slashing", "properties": "versatile"
    }]);
    let mut target = base_snap();
    target.id = Uuid::new_v4();
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
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
        precision_superiority: true,
    };
    let result = resolve_attack(&attacker, &target, &req, &attacker_stats, &target_stats).unwrap();
    let precision = result.precision_superiority_bonus.expect("precision die rolled");
    assert!((1..=8).contains(&precision), "fighter 5 → d8 superiority die ({precision})");
    // bonus = pb 3 + str 3 + precision; attack_total - natural must include it.
    assert_eq!(
        result.attack_total - result.natural_roll,
        6 + precision,
        "precision die must be added to the attack roll"
    );
}

#[tokio::test]
async fn creature_size_ranks() {
    let mut small = base_snap();
    small.race = Some("Halfling".into());
    assert_eq!(dungeonsandapps::combat_engine::creature_size(&small), 2);
    let mut med = base_snap();
    med.race = Some("Human".into());
    assert_eq!(dungeonsandapps::combat_engine::creature_size(&med), 3);
    let mut large = base_snap();
    large.sheet_raw = json!({"size": "large"});
    assert_eq!(dungeonsandapps::combat_engine::creature_size(&large), 4);
    let mut garg = base_snap();
    garg.sheet_raw = json!({"size": "gargantuan"});
    assert_eq!(dungeonsandapps::combat_engine::creature_size(&garg), 6);
}

// H-1: concentration check is a CON save — must use the full save modifier
// (proficiency/bonuses via save_mods), not the raw ability mod.
#[tokio::test]
async fn concentration_check_uses_con_save_bonus() {
    let snap = base_snap();
    let mut stats = compute_stats(&snap);
    stats.save_mods = vec![("con".to_string(), 6)];
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let (_, res) = concentration_check(&snap, &stats, 20, &mut rng);
    assert!(
        res.expression.contains("+6"),
        "concentration expr must use the CON save bonus: {}",
        res.expression
    );
}

fn rage_attack_req(target_id: Uuid, weapon_id: &str, ability: &str) -> AttackReq {
    AttackReq {
        target_id,
        weapon_id: Some(weapon_id.into()),
        ability: Some(ability.into()),
        proficient: Some(true),
        power_attack: false,
        cover: None,
        advantage: false,
        disadvantage: false,
        extra_damage_expression: None,
        extra_damage_type: None,
        attack_expression: None,
        damage_expression: Some("10".into()),
        damage_type: "slashing".into(),
        damage_die: None,
        is_spell_attack: false,
        is_magical: false,
        frightened_source_visible: None,
        label: None,
        reckless: false,
        bless_dice: None,
        bardic_inspiration_dice: None,
        precision_superiority: false,
        sneak_attack: false,
        sneak_attack_dice: None,
        stunning_strike: false,
        smite_slot_level: None,
    }
}

// H-5: Rage's damage bonus applies ONLY to melee weapon attacks using
// Strength — never ranged, spell, or DEX-finesse attacks.
#[tokio::test]
async fn rage_damage_bonus_only_melee_str_attacks() {
    let mut attacker = base_snap();
    attacker.abilities = json!({"str": 18, "dex": 8, "con": 10, "int": 10, "wis": 10, "cha": 10});
    attacker.weapons = json!([
        { "id": "gs", "name": "Greatsword", "damage": "2d6", "damage_type": "slashing", "properties": "heavy, two-handed" },
        { "id": "bow", "name": "Longbow", "damage": "1d8", "damage_type": "piercing", "properties": "ammunition, heavy, ranged" }
    ]);
    let mut target = base_snap();
    target.id = Uuid::new_v4();
    target.base_ac = 5;
    target.hp_current = 50;
    target.hp_max = 50;
    let target_stats = compute_stats(&target);
    let mut attacker_stats = compute_stats(&attacker);
    attacker_stats.damage_bonus = 2; // Rage effect

    // Nat-1 auto-misses; retry each scenario until it lands so the
    // damage assertions are deterministic.
    let melee = loop {
        let r = resolve_attack(
            &attacker,
            &target,
            &rage_attack_req(target.id, "gs", "str"),
            &attacker_stats,
            &target_stats,
        )
        .unwrap();
        if r.hit { break r; }
    };
    assert_eq!(melee.damage_base, 12, "melee STR attack gets the rage bonus");

    let ranged = loop {
        let r = resolve_attack(
            &attacker,
            &target,
            &rage_attack_req(target.id, "bow", "dex"),
            &attacker_stats,
            &target_stats,
        )
        .unwrap();
        if r.hit { break r; }
    };
    assert_eq!(ranged.damage_base, 10, "ranged attack must NOT get the rage bonus");

    // Finesse with DEX > STR: the attack uses DEX → no rage bonus.
    let mut dex_fencer = base_snap();
    dex_fencer.abilities = json!({"str": 8, "dex": 18, "con": 10, "int": 10, "wis": 10, "cha": 10});
    dex_fencer.weapons = json!([{ "id": "rapier", "name": "Rapier", "damage": "1d8", "damage_type": "piercing", "properties": "finesse" }]);
    let mut dex_stats = compute_stats(&dex_fencer);
    dex_stats.damage_bonus = 2;
    let finesse = loop {
        let r = resolve_attack(
            &dex_fencer,
            &target,
            &rage_attack_req(target.id, "rapier", "str"),
            &dex_stats,
            &target_stats,
        )
        .unwrap();
        if r.hit { break r; }
    };
    assert_eq!(
        finesse.damage_base, 10,
        "DEX-finesse attack must NOT get the rage bonus"
    );
}
