use super::super::stats::{ability_mod, compute_weapon_damage_expression, proficiency_from_level};
use super::super::types::{CombatantSnapshot, ComputedStats};
use super::damage_type::{
    apply_damage_type, apply_hp_damage, concentration_check, crit_double_dice,
};
use super::types::{AttackResult, find_weapon};
use crate::dice::roll;
use rand::{SeedableRng, rngs::StdRng};

pub fn resolve_two_weapon_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    offhand_weapon_id: &str,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
    twf_fighting_style: bool,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    let weapon = find_weapon(attacker, offhand_weapon_id)
        .ok_or_else(|| "off-hand weapon not found".to_string())?;
    let weapon_props = weapon.1;

    if !weapon_props.light {
        return Err("off-hand weapon must have the light property".to_string());
    }

    let pb = if attacker.proficiency_bonus > 0 {
        attacker.proficiency_bonus
    } else {
        proficiency_from_level(attacker.level_total)
    };

    let ability = if weapon_props.finesse {
        if ability_mod(attacker, "dex") > ability_mod(attacker, "str") {
            "dex"
        } else {
            "str"
        }
    } else if weapon_props.thrown && !weapon_props.ranged {
        "str"
    } else if weapon_props.ranged {
        "dex"
    } else {
        "str"
    };
    let ability_mod_val = ability_mod(attacker, ability);

    let attack_expr = format!("1d20+{}+{}", ability_mod_val, pb);
    let attack_roll =
        roll(&attack_expr, &mut rng).map_err(|e| format!("attack roll error: {}", e))?;

    let natural_roll = attack_roll
        .terms
        .first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let crit_range = attacker
        .sheet_raw
        .get("crit_range")
        .and_then(|v| v.as_i64())
        .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
        .unwrap_or(20);
    let critical = natural_roll >= crit_range;
    let auto_miss = natural_roll == 1;
    let target_ac = target_stats.ac;
    let hit = if critical {
        true
    } else if auto_miss {
        false
    } else {
        attack_roll.total >= target_ac
    };

    let mut result = AttackResult {
        hit,
        critical,
        natural_roll,
        attack_total: attack_roll.total,
        target_ac,
        attack_roll,
        damage_roll: None,
        damage_base: 0,
        damage_applied: 0,
        extra_damage_applied: 0,
        extra_damage_type: None,
            sneak_attack_applied: false,
            sneak_attack_damage: 0,
            stunning_strike_applied: false,
            stunning_strike_save_passed: None,
            smite_applied: false,
            smite_damage: 0,
            smite_slot_consumed: None,
        target_hp_before: target.hp_current,
        target_hp_after: target.hp_current,
        target_temp_hp_after: target.temp_hp,
        concentration_broken: false,
        concentration_roll: None,
        combat_event_id: None,
        cover_bonus: 0,
        attack_advantage: false,
        attack_disadvantage: false,
        damage_resisted: false,
        damage_vulnerable: false,
        damage_immune: false,
        reach_weapon: weapon_props.reach,
        needs_ammo: weapon_props.ammunition,
        instant_death: false,
    };

    if hit {
        let dmg_expr = compute_weapon_damage_expression(weapon.0, attacker, false);
        // For TWF: remove ability mod from damage unless fighting style
        let dmg_expr_no_mod = if twf_fighting_style {
            dmg_expr
        } else {
            // Strip the +ability_mod suffix if present
            let base_die = weapon
                .0
                .get("damage_die")
                .and_then(|v| v.as_str())
                .or_else(|| weapon.0.get("damage").and_then(|v| v.as_str()))
                .unwrap_or("1d4");
            base_die.to_string()
        };

        let mut dmg_roll =
            roll(&dmg_expr_no_mod, &mut rng).map_err(|e| format!("damage roll error: {}", e))?;

        if critical {
            let crit_expr = crit_double_dice(&dmg_expr_no_mod);
            dmg_roll =
                roll(&crit_expr, &mut rng).map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        let raw_dmg =
            dmg_roll.total + attacker_stats.damage_bonus + attacker_stats.weapon_damage_bonus;
        let dtype = weapon
            .0
            .get("damage_type")
            .and_then(|v| v.as_str())
            .unwrap_or("slashing")
            .to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) =
            apply_damage_type(raw_dmg, &dtype, target_stats, false);

        result.damage_roll = Some(dmg_roll);
        result.damage_base = raw_dmg;
        result.damage_applied = effective_dmg;
        result.damage_resisted = resisted;
        result.damage_vulnerable = vulnerable;
        result.damage_immune = immune;
        result.instant_death = target.hp_current > 0
            && (effective_dmg - target.hp_current - target.temp_hp).max(0) >= target.hp_max;

        let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, effective_dmg);
        result.target_hp_after = new_hp;
        result.target_temp_hp_after = new_temp;

        if target.active_effects.iter().any(|e| e.concentration) {
            let (broken, roll_res) = concentration_check(target, effective_dmg, &mut rng);
            result.concentration_broken = broken;
            result.concentration_roll = Some(roll_res);
        }
    }

    Ok(result)
}
