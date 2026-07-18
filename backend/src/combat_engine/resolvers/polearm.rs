// Polearm Master bonus-action attack (PHB p.168).
//
// "When you take the Attack action and attack with only a glaive, halberd,
//  quarterstaff, or spear, you can use a bonus action to make a melee attack
//  with the opposite end of the weapon; the damage die is a d4 and the attack
//  deals bludgeoning damage."
//
// Mechanic enforced here: must be wielding a polearm weapon (glaive/halberd/
// quarterstaff) and have the Polearm Master feat (gated by the route handler).
// The attack uses the attacker's STR mod + prof; damage is 1d4 + STR mod
// (no fighting-style bonus since it's the off-end of the weapon).

use super::damage_type::{apply_damage_type, apply_hp_damage, concentration_check, crit_double_dice};
use super::super::stats::{ability_mod, proficiency_from_level};
use super::super::types::{CombatantSnapshot, ComputedStats};
use super::types::AttackResult;
use crate::dice::roll;
use rand::SeedableRng;
use rand::rngs::StdRng;

/// Names of weapons that count as polearms for Polearm Master.
pub const POLEARM_NAMES: &[&str] = &["glaive", "halberd", "quarterstaff"];

/// Returns true if the attacker is currently wielding a weapon whose
/// name (case-insensitive substring match) is in POLEARM_NAMES.
pub fn is_wielding_polearm(attacker: &CombatantSnapshot) -> bool {
    let weapons = match attacker.weapons.as_array() {
        Some(a) => a,
        None => return false,
    };
    weapons.iter().any(|w| {
        let name = w
            .get("name")
            .and_then(|v| v.as_str())
            .or_else(|| w.get("id").and_then(|v| v.as_str()))
            .unwrap_or("")
            .to_lowercase();
        POLEARM_NAMES.iter().any(|p| name == *p || name.contains(p))
    })
}

pub fn resolve_polearm_ba_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    let pb = if attacker.proficiency_bonus > 0 {
        attacker.proficiency_bonus
    } else {
        proficiency_from_level(attacker.level_total)
    };
    let str_mod = ability_mod(attacker, "str");
    let attack_expr = format!("1d20+{}+{}", str_mod, pb);
    let attack_roll =
        roll(&attack_expr, &mut rng).map_err(|e| format!("polearm BA attack roll error: {}", e))?;
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
        reach_weapon: false,
        needs_ammo: false,
        instant_death: false,
    };

    if hit {
        // PHB: "the damage die is a d4 and the attack deals bludgeoning damage."
        // No ability mod is added to the damage in some readings (the butt end
        // is a small strike); PHB is silent. Common interpretation: 1d4 + STR
        // mod, matching the unarmed-strike rule. We follow that.
        let dmg_expr = "1d4".to_string();
        let mut dmg_roll = roll(&dmg_expr, &mut rng)
            .map_err(|e| format!("polearm BA damage roll error: {}", e))?;
        if critical {
            let crit_expr = crit_double_dice(&dmg_expr);
            dmg_roll = roll(&crit_expr, &mut rng)
                .map_err(|e| format!("polearm BA crit damage roll error: {}", e))?;
        }
        // Apply STR mod + damage bonuses from effects.
        let raw_dmg = dmg_roll.total
            + str_mod
            + attacker_stats.damage_bonus
            + attacker_stats.weapon_damage_bonus;
        let (effective_dmg, resisted, vulnerable, immune) =
            apply_damage_type(raw_dmg, "bludgeoning", target_stats, false);
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
