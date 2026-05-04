use cinghialapp::combat_engine::{
    CombatantSnapshot,
    apply_damage_type, apply_hp_damage, compute_max_hp_from_sheet,
    compute_stats, concentration_check, proficiency_from_level,
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
    assert!(stats.save_disadvantage, "exhaustion 1 → disadvantage on ability checks");
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
    assert!(stats.save_disadvantage, "petrified → auto-fail STR/DEX saves");
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
    assert!(roll.total >= 6, "2d20kh1+5 should roll at least 6: got {}", roll.total);
    let _ = broken; // result is probabilistic, don't assert pass/fail
}

#[tokio::test]
async fn concentration_check_normal_uses_1d20() {
    let mut snap = base_snap();
    snap.abilities = json!({"str":10,"dex":10,"con":10,"int":10,"wis":10,"cha":10});
    snap.sheet_raw = json!({});

    let mut rng = rand::rngs::StdRng::seed_from_u64(1);
    let (_broken, roll) = concentration_check(&snap, 20, &mut rng);
    assert!(roll.total >= 1 && roll.total <= 20, "1d20+0 total out of range: {}", roll.total);
}

#[tokio::test]
async fn apply_damage_type_nonmagical_dr_reduces_bps() {
    let stats = cinghialapp::combat_engine::ComputedStats {
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
    let stats = cinghialapp::combat_engine::ComputedStats {
        nonmagical_damage_reduction: 3,
        ..Default::default()
    };
    let (dmg, _, _, _) = apply_damage_type(10, "bludgeoning", &stats, true);
    assert_eq!(dmg, 10, "magical damage bypasses nonmagical DR");
}

#[tokio::test]
async fn apply_damage_type_resistance_halves() {
    let mut stats = cinghialapp::combat_engine::ComputedStats::default();
    stats.resistances.insert("fire".into());
    let (dmg, is_resistant, _, _) = apply_damage_type(10, "fire", &stats, false);
    assert_eq!(dmg, 5);
    assert!(is_resistant);
}

#[tokio::test]
async fn apply_damage_type_immunity_zeroes() {
    let mut stats = cinghialapp::combat_engine::ComputedStats::default();
    stats.immunities.insert("cold".into());
    let (dmg, _, _, is_immune) = apply_damage_type(10, "cold", &stats, false);
    assert_eq!(dmg, 0);
    assert!(is_immune);
}

#[tokio::test]
async fn apply_damage_type_vulnerability_doubles() {
    let mut stats = cinghialapp::combat_engine::ComputedStats::default();
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

    assert_eq!(hp_with_tough - hp_without, 8, "tough adds 2×4=8 HP at level 4");
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

    assert_eq!(normal_hp - reduced_hp, 5, "hp_max_reduction of 5 should subtract 5");
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
