// Combat resolution: req/result structs + resolve_attack, resolve_damage,
// resolve_save, resolve_heal, resolve_death_save, resolve_skill_check,
// concentration_check, crit_double_dice, apply_damage_type.
// Extracted from combat_engine.rs to keep the file under the 500-line
// guideline (per AGENTS.md §1.4).

use super::stats::{ability_mod, compute_weapon_damage_expression, proficiency_from_level};
use super::types::{CombatantSnapshot, ComputedStats};
use crate::dice::{RollResult, roll};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};

pub struct AttackReq {
    pub target_id: uuid::Uuid,
    /// If provided, roll this expression. Otherwise auto-compute.
    pub attack_expression: Option<String>,
    pub damage_expression: Option<String>,
    pub damage_type: String,
    /// Base weapon die (e.g. "d8") — used for Savage Attacks extra crit die.
    pub damage_die: Option<String>,
    pub ability: Option<String>, // str/dex/etc for computing mod if expressions not given
    pub proficient: Option<bool>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub cover: Option<String>, // none | half | three_quarters
    pub is_spell_attack: bool,
    pub is_magical: bool,
    pub label: Option<String>,
    /// Weapon ID from sheet.weapons[].id; used to apply weapon properties.
    pub weapon_id: Option<String>,
    /// Extra damage on hit (Sneak Attack, Divine Smite, Rage, etc.) — expression like "2d6"
    pub extra_damage_expression: Option<String>,
    pub extra_damage_type: Option<String>,
    /// Sharpshooter / Great Weapon Master: −5 attack, +10 damage on hit
    pub power_attack: bool,
    /// Reckless Attack (Barbarian): advantage on attack, enemies have advantage against you
    pub reckless: bool,
    /// Bless spell: extra d4(s) added to attack roll
    pub bless_dice: Option<i32>,
    /// Bardic Inspiration: extra d6/d8/d10/d12 added to attack roll (die size)
    pub bardic_inspiration_dice: Option<i32>,
}

/// Parsed weapon properties from sheet JSON
#[derive(Debug, Default, Clone)]
pub struct WeaponProps {
    pub finesse: bool,
    pub reach: bool,
    pub ranged: bool,
    pub thrown: bool,
    pub two_handed: bool,
    pub versatile: bool,
    pub light: bool,
    pub heavy: bool,
    pub ammunition: bool,
    pub loading: bool,
    pub special: bool,
}

pub fn parse_weapon_props(props_str: &str) -> WeaponProps {
    let mut p = WeaponProps::default();
    for part in props_str.split(',') {
        match part.trim().to_lowercase().as_str() {
            "finesse" => p.finesse = true,
            "reach" => p.reach = true,
            "ranged" => p.ranged = true,
            "thrown" => p.thrown = true,
            "two-handed" | "twohanded" | "two_handed" => p.two_handed = true,
            "versatile" => p.versatile = true,
            "light" => p.light = true,
            "heavy" => p.heavy = true,
            "ammunition" | "ammo" => p.ammunition = true,
            "loading" => p.loading = true,
            "special" => p.special = true,
            _ => {}
        }
    }
    p
}

pub fn find_weapon<'a>(snapshot: &'a CombatantSnapshot, weapon_id: &str) -> Option<(&'a serde_json::Value, WeaponProps)> {
    if let Some(arr) = snapshot.weapons.as_array() {
        for w in arr {
            if w.get("id").and_then(|v| v.as_str()) == Some(weapon_id)
                || w.get("name").and_then(|v| v.as_str()) == Some(weapon_id)
            {
                let props = w.get("properties")
                    .and_then(|v| v.as_str())
                    .map(parse_weapon_props)
                    .unwrap_or_default();
                return Some((w, props));
            }
        }
    }
    None
}

#[derive(Debug, Serialize)]
pub struct AttackResult {
    pub hit: bool,
    pub critical: bool,
    pub natural_roll: i32,
    pub attack_total: i32,
    pub target_ac: i32,
    pub attack_roll: RollResult,
    pub damage_roll: Option<RollResult>,
    pub damage_base: i32,
    pub damage_applied: i32,
    pub extra_damage_applied: i32,
    pub extra_damage_type: Option<String>,
    pub target_hp_before: i32,
    pub target_hp_after: i32,
    pub target_temp_hp_after: i32,
    pub concentration_broken: bool,
    pub concentration_roll: Option<RollResult>,
    pub combat_event_id: Option<uuid::Uuid>,
    pub cover_bonus: i32,
    pub attack_advantage: bool,
    pub attack_disadvantage: bool,
    pub damage_resisted: bool,
    pub damage_vulnerable: bool,
    pub damage_immune: bool,
    pub reach_weapon: bool,
    pub needs_ammo: bool,
    pub instant_death: bool,
}

#[derive(Debug, Deserialize)]
pub struct DamageReq {
    pub amount: i32,
    pub damage_type: String,
    pub source_combatant_id: Option<uuid::Uuid>,
    pub label: Option<String>,
    pub is_magical: bool,
}

#[derive(Debug, Serialize)]
pub struct DamageResult {
    pub damage_raw: i32,
    pub damage_applied: i32,
    pub hp_before: i32,
    pub hp_after: i32,
    pub temp_hp_after: i32,
    pub concentration_broken: bool,
    pub concentration_roll: Option<RollResult>,
    pub combat_event_id: Option<uuid::Uuid>,
    pub damage_resisted: bool,
    pub damage_vulnerable: bool,
    pub damage_immune: bool,
    pub instant_death: bool,
}

#[derive(Debug, Deserialize)]
pub struct SaveReq {
    pub ability: String, // str/dex/con/int/wis/cha
    pub dc: i32,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
    pub is_magical: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct SaveResult {
    pub passed: bool,
    pub natural_roll: i32,
    pub save_total: i32,
    pub dc: i32,
    pub save_roll: RollResult,
    pub save_advantage: bool,
    pub save_disadvantage: bool,
}

#[derive(Debug, Deserialize)]
pub struct HealReq {
    pub amount: i32,
    pub source_combatant_id: Option<uuid::Uuid>,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct HealResult {
    pub amount: i32,
    pub hp_before: i32,
    pub hp_after: i32,
    pub temp_hp_after: i32,
    pub stabilized: bool,
    pub revived: bool,
}

#[derive(Debug, Deserialize)]
pub struct DeathSaveReq {
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DeathSaveResult {
    pub natural_roll: i32,
    pub passed: bool,
    pub successes_before: i32,
    pub failures_before: i32,
    pub successes_after: i32,
    pub failures_after: i32,
    pub stabilized: bool,
    pub died: bool,
    pub nat20: bool,
    pub nat1: bool,
    pub hp_after: i32,
    pub alive: bool,
}

#[derive(Debug, Deserialize)]
pub struct SkillCheckReq {
    pub skill: String, // e.g. "perception", "athletics"
    pub dc: Option<i32>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SkillCheckResult {
    pub skill: String,
    pub natural_roll: i32,
    pub total: i32,
    pub dc: Option<i32>,
    pub passed: Option<bool>,
    pub advantage: bool,
    pub disadvantage: bool,
}

#[derive(Debug, Deserialize)]
pub struct CastSpellReq {
    pub spell_slug: String,
    pub target_ids: Vec<uuid::Uuid>,
    pub upcast_level: Option<i32>,
    pub advantage: bool,
    pub disadvantage: bool,
    pub save_advantage: bool,
    pub save_disadvantage: bool,
}

#[derive(Debug, Serialize)]
pub struct CastSpellResult {
    pub spell_slug: String,
    pub target_results: Vec<SpellTargetResult>,
    pub slot_consumed: bool,
    pub concentration_broken_existing: bool,
}

#[derive(Debug, Serialize)]
pub struct SpellTargetResult {
    pub target_id: uuid::Uuid,
    pub hit: Option<bool>, // None for save-based spells
    pub critical: bool,
    pub save_passed: Option<bool>,
    pub damage_applied: i32,
    pub hp_after: i32,
    pub temp_hp_after: i32,
    pub effect_applied: bool,
}

// =====================================================================
// Pure Resolution Functions
// =====================================================================

pub fn resolve_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    req: &AttackReq,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    // Determine cover bonus
    let cover_bonus = match req.cover.as_deref() {
        Some("half") => 2,
        Some("three_quarters") => 5,
        _ => 0,
    };

    // Determine advantage/disadvantage
    let mut adv = req.advantage || attacker_stats.attack_advantage;
    let mut dis = req.disadvantage || attacker_stats.attack_disadvantage;

    // Resolve weapon properties early so prone/ranged checks can use them
    let weapon = req.weapon_id.as_deref().and_then(|wid| find_weapon(attacker, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
    let is_ranged_attack = weapon_props.ranged || weapon_props.thrown || req.is_spell_attack;

    // Target conditions affect attacker
    if target_stats.prone {
        let within_5ft = if let (Some(ax), Some(ay), Some(tx), Some(ty)) = (attacker.token_x, attacker.token_y, target.token_x, target.token_y) {
            let d_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
            d_pct < 5.0
        } else { true };
        if within_5ft {
            adv = true;
        } else {
            dis = true;
        }
    }
    if target_stats.invisible {
        dis = true;
    }
    if target_stats.paralyzed || target_stats.unconscious || target_stats.restrained {
        adv = true;
    }
    // Target's effects that affect attacker's rolls (Dodge, Help, Reckless)
    if target_stats.attack_disadvantage_against {
        dis = true;
    }
    if target_stats.attack_advantage_against {
        adv = true;
    }

    // Invisible attacker has advantage
    if attacker_stats.invisible {
        adv = true;
    }
    // Frightened attacker has disadvantage if source is visible
    if attacker_stats.frightened {
        dis = true;
    }
    // Poisoned attacker has disadvantage
    if attacker_stats.poisoned {
        dis = true;
    }
    // Blinded attacker has disadvantage
    if attacker_stats.blinded {
        dis = true;
    }

    // Charmed: no blanket disadvantage. PHB p.290: can't attack the charmer (enforced per-target, not here).

    // Prone ranged disadvantage: being prone + using ranged/thrown weapon = disadvantage
    if attacker_stats.prone_ranged_disadvantage && is_ranged_attack {
        dis = true;
    }

    // Final cancel out
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    // Archery fighting style: +2 to ranged attack rolls
    let archery_bonus = if attacker_stats.archery_style && (weapon_props.ranged || weapon_props.thrown) { 2 } else { 0 };
    // power_attack (Sharpshooter / Great Weapon Master): -5 attack roll
    let power_attack_penalty = if req.power_attack { -5 } else { 0 };

    // Roll attack
    let attack_expr = if let Some(ref expr) = req.attack_expression {
        expr.clone()
    } else {
        // Auto-compute: 1d20 + pb + ability_mod + attack_bonus from effects
        let pb = if attacker.proficiency_bonus > 0 {
            attacker.proficiency_bonus
        } else {
            proficiency_from_level(attacker.level_total)
        };
        let ability = req.ability.as_deref().unwrap_or_else(|| {
            if weapon_props.thrown && !weapon_props.ranged { "str" }
            else if weapon_props.ranged { "dex" }
            else { "str" }
        });
        let ability_mod = if weapon_props.finesse {
            ability_mod(attacker, "str").max(ability_mod(attacker, "dex"))
        } else {
            ability_mod(attacker, ability)
        };
        let prof = if req.proficient.unwrap_or(true) { pb } else { 0 };
        let bonus = attacker_stats.attack_bonus + archery_bonus + power_attack_penalty;

        // Bless: +1d4 (or +Nd4 if multiple bless sources)
        let bless_str = if let Some(n) = req.bless_dice.filter(|&n| n > 0) {
            if n == 1 { "+1d4".to_string() }
            else { format!("+{}d4", n) }
        } else { String::new() };

        // Bardic Inspiration: +1dX
        let bardic_str = if let Some(die) = req.bardic_inspiration_dice {
            format!("+1d{}", die)
        } else { String::new() };

        if effective_adv {
            format!("2d20kh1+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        } else if effective_dis {
            format!("2d20kl1+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        } else {
            format!("1d20+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        }
    };

    let attack_roll = roll(&attack_expr, &mut rng)
        .map_err(|e| format!("attack roll error: {}", e))?;

    // Determine natural roll (kept die for adv/dis, first roll for straight rolls)
    let natural_roll = attack_roll.terms.first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let crit_range = attacker.sheet_raw.get("crit_range")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(20);
    let critical = natural_roll >= crit_range;
    let auto_miss = natural_roll == 1;

    let target_ac = target_stats.ac + cover_bonus;
    let hit = if critical { true } else if auto_miss { false } else { attack_roll.total >= target_ac };

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
        target_hp_before: target.hp_current,
        target_hp_after: target.hp_current,
        target_temp_hp_after: target.temp_hp,
        concentration_broken: false,
        concentration_roll: None,
        combat_event_id: None,
        cover_bonus,
        attack_advantage: effective_adv,
        attack_disadvantage: effective_dis,
        damage_resisted: false,
        damage_vulnerable: false,
        damage_immune: false,
        reach_weapon: weapon_props.reach,
        needs_ammo: weapon_props.ammunition,
        instant_death: false,
    };

    if hit {
        let dmg_expr = if let Some(ref expr) = req.damage_expression {
            expr.clone()
        } else if let Some((weapon, _)) = req.weapon_id.as_deref().and_then(|wid| find_weapon(attacker, wid)) {
            compute_weapon_damage_expression(weapon, attacker, false)
        } else {
            // Default: unarmed strike = 1 + str mod (min 1 total)
            let str_mod = ability_mod(attacker, "str");
            let base = (1 + str_mod).max(1);
            format!("{}", base)
        };

        let mut dmg_roll = roll(&dmg_expr, &mut rng)
            .map_err(|e| format!("damage roll error: {}", e))?;

        // GWF: reroll weapon damage once if any die landed 1 or 2
        // Only applies to melee weapons; take the better of two rolls
        if attacker_stats.gwf_style && !weapon_props.ranged && !weapon_props.thrown {
            let has_low = dmg_roll.terms.iter()
                .flat_map(|t| t.rolls.iter())
                .any(|&r| r <= 2);
            if has_low {
                if let Ok(rerolled) = roll(&dmg_expr, &mut rng) {
                    if rerolled.total > dmg_roll.total {
                        dmg_roll = rerolled;
                    }
                }
            }
        }

        // Critical = double dice
        if critical {
            let crit_expr = crit_double_dice(&dmg_expr);
            dmg_roll = roll(&crit_expr, &mut rng)
                .map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        // Savage Attacks (Half-orc): extra weapon die on crit
        let savage_bonus = if critical && attacker_stats.savage_attacks {
            let die = req.damage_die.as_deref().unwrap_or("d6");
            roll(&format!("1{}", die), &mut rng).map(|r| r.total).unwrap_or(0)
        } else { 0 };

        // Dueling style: +2 damage when wielding a one-handed weapon and no off-hand weapon
        // (simplified: +2 if not two-handed and not ranged)
        let dueling_bonus = if attacker_stats.dueling_style
            && !weapon_props.two_handed
            && !weapon_props.ranged
            && !weapon_props.thrown
        { 2 } else { 0 };

        // Power attack (Sharpshooter / GWM): +10 damage on hit
        let power_attack_damage = if req.power_attack { 10 } else { 0 };

        let raw_dmg = dmg_roll.total + attacker_stats.damage_bonus + attacker_stats.weapon_damage_bonus + savage_bonus + dueling_bonus + power_attack_damage;
        let dtype = req.damage_type.to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) = apply_damage_type(raw_dmg, &dtype, target_stats, req.is_magical);

        // Extra damage (Sneak Attack, Divine Smite, Rage, etc.)
        // PHB p.196: all attack damage dice are doubled on a critical hit.
        let (extra_applied, extra_dtype) = if let Some(ref extra_expr) = req.extra_damage_expression {
            let expr = if critical { crit_double_dice(extra_expr) } else { extra_expr.clone() };
            let extra_roll = roll(&expr, &mut rng).map_err(|e| format!("extra damage roll error: {}", e))?;
            let extra_raw = extra_roll.total;
            let extra_type = req.extra_damage_type.as_deref().unwrap_or("piercing");
            let (extra_eff, _, _, _) = apply_damage_type(extra_raw, extra_type, target_stats, req.is_magical);
            (extra_eff, Some(extra_type.to_string()))
        } else {
            (0, None)
        };

        result.damage_roll = Some(dmg_roll);
        result.damage_base = raw_dmg;
        result.damage_applied = effective_dmg;
        result.extra_damage_applied = extra_applied;
        result.extra_damage_type = extra_dtype;
        result.damage_resisted = resisted;
        result.damage_vulnerable = vulnerable;
        result.damage_immune = immune;

        let total_damage = effective_dmg + extra_applied;

        // PHB p.197: massive damage = remaining damage after reducing to 0 ≥ hp_max
        let remaining_after_zero = (total_damage - target.hp_current - target.temp_hp).max(0);
        result.instant_death = target.hp_current > 0 && remaining_after_zero >= target.hp_max;

        // Apply HP damage
        let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, total_damage);
        result.target_hp_after = new_hp;
        result.target_temp_hp_after = new_temp;

        // Concentration check if target has concentration
        if target.active_effects.iter().any(|e| e.concentration) {
            let (broken, roll_res) = concentration_check(target, total_damage, &mut rng);
            result.concentration_broken = broken;
            result.concentration_roll = Some(roll_res);
        }
    }

    Ok(result)
}

/// Resolve a two-weapon fighting bonus-action off-hand attack.
/// Off-hand weapon must have the "light" property.
/// Damage does NOT include ability modifier unless the attacker has the TWF fighting style.
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
        if ability_mod(attacker, "dex") > ability_mod(attacker, "str") { "dex" } else { "str" }
    } else if weapon_props.thrown && !weapon_props.ranged {
        "str"
    } else if weapon_props.ranged {
        "dex"
    } else {
        "str"
    };
    let ability_mod_val = ability_mod(attacker, ability);

    let attack_expr = format!("1d20+{}+{}", ability_mod_val, pb);
    let attack_roll = roll(&attack_expr, &mut rng)
        .map_err(|e| format!("attack roll error: {}", e))?;

    let natural_roll = attack_roll.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    let crit_range = attacker.sheet_raw.get("crit_range")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(20);
    let critical = natural_roll >= crit_range;
    let auto_miss = natural_roll == 1;
    let target_ac = target_stats.ac;
    let hit = if critical { true } else if auto_miss { false } else { attack_roll.total >= target_ac };

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
            let base_die = weapon.0.get("damage_die").and_then(|v| v.as_str())
                .or_else(|| weapon.0.get("damage").and_then(|v| v.as_str()))
                .unwrap_or("1d4");
            base_die.to_string()
        };

        let mut dmg_roll = roll(&dmg_expr_no_mod, &mut rng)
            .map_err(|e| format!("damage roll error: {}", e))?;

        if critical {
            let crit_expr = crit_double_dice(&dmg_expr_no_mod);
            dmg_roll = roll(&crit_expr, &mut rng)
                .map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        let raw_dmg = dmg_roll.total + attacker_stats.damage_bonus + attacker_stats.weapon_damage_bonus;
        let dtype = weapon.0.get("damage_type").and_then(|v| v.as_str()).unwrap_or("slashing").to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) = apply_damage_type(raw_dmg, &dtype, target_stats, false);

        result.damage_roll = Some(dmg_roll);
        result.damage_base = raw_dmg;
        result.damage_applied = effective_dmg;
        result.damage_resisted = resisted;
        result.damage_vulnerable = vulnerable;
        result.damage_immune = immune;
        result.instant_death = target.hp_current > 0 && (effective_dmg - target.hp_current - target.temp_hp).max(0) >= target.hp_max;

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

pub fn resolve_damage(
    target: &CombatantSnapshot,
    req: &DamageReq,
    target_stats: &ComputedStats,
) -> Result<DamageResult, String> {
    let mut rng = StdRng::from_os_rng();
    let dtype = req.damage_type.to_lowercase();
    let (effective_dmg, damage_resisted, damage_vulnerable, damage_immune) = apply_damage_type(req.amount, &dtype, target_stats, req.is_magical);

    let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, effective_dmg);

    let mut concentration_broken = false;
    let mut concentration_roll = None;
    if target.active_effects.iter().any(|e| e.concentration) {
        let (broken, roll_res) = concentration_check(target, effective_dmg, &mut rng);
        concentration_broken = broken;
        concentration_roll = Some(roll_res);
    }

    Ok(DamageResult {
        damage_raw: req.amount,
        damage_applied: effective_dmg,
        hp_before: target.hp_current,
        hp_after: new_hp,
        temp_hp_after: new_temp,
        concentration_broken,
        concentration_roll,
        combat_event_id: None,
        damage_resisted,
        damage_vulnerable,
        damage_immune,
        instant_death: target.hp_current > 0 && (effective_dmg - target.hp_current - target.temp_hp).max(0) >= target.hp_max,
    })
}

pub fn resolve_save(
    snap: &CombatantSnapshot,
    req: &SaveReq,
    stats: &ComputedStats,
) -> Result<SaveResult, String> {
    let mut rng = StdRng::from_os_rng();
    let ability = req.ability.to_lowercase();

    let mut adv = req.advantage || stats.save_advantage;
    let dis = req.disadvantage || stats.save_disadvantage;
    // Gnome Cunning: advantage on INT/WIS/CHA saves vs magic
    if stats.gnome_cunning && req.is_magical.unwrap_or(false)
        && matches!(ability.as_str(), "int" | "wis" | "cha")
    {
        adv = true;
    }
    // Magic Resistance: advantage on saves vs spells/magical effects (Yuan-Ti, Satyr)
    if snap.sheet_raw.get("magic_resistance").and_then(|v| v.as_bool()).unwrap_or(false)
        && req.is_magical.unwrap_or(false)
    {
        adv = true;
    }
    // Paralyzed/Petrified = auto-fail STR and DEX saves
    if (stats.paralyzed || stats.petrified) && (ability == "str" || ability == "dex") {
        let save_roll = roll("1d20", &mut rng).unwrap_or_else(|e| {
            tracing::error!("auto-fail 1d20 roll failed: {e}; using 0");
            crate::dice::RollResult { expression: "1d20".into(), terms: vec![], total: 0 }
        });
        return Ok(SaveResult {
            passed: false,
            natural_roll: 1,
            save_total: 1,
            dc: req.dc,
            save_roll,
            save_advantage: false,
            save_disadvantage: true,
        });
    }

    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let save_mod = stats.save_mods.iter()
        .find(|(a, _)| a == &ability)
        .map(|(_, m)| *m)
        .unwrap_or(ability_mod(snap, &ability));

    let expr = if effective_adv {
        format!("2d20kh1+{}", save_mod)
    } else if effective_dis {
        format!("2d20kl1+{}", save_mod)
    } else {
        format!("1d20+{}", save_mod)
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("save roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let passed = roll_res.total >= req.dc;

    Ok(SaveResult {
        passed,
        natural_roll: natural,
        save_total: roll_res.total,
        dc: req.dc,
        save_roll: roll_res,
        save_advantage: effective_adv,
        save_disadvantage: effective_dis,
    })
}

pub fn resolve_heal(target: &CombatantSnapshot, req: &HealReq) -> HealResult {
    let hp_before = target.hp_current;
    let hp_after = (target.hp_current + req.amount).min(target.hp_max);
    let stabilized = hp_before <= 0 && hp_after > 0;
    let revived = stabilized; // same concept here
    HealResult {
        amount: req.amount,
        hp_before,
        hp_after,
        temp_hp_after: target.temp_hp,
        stabilized,
        revived,
    }
}

pub fn resolve_death_save(
    snap: &CombatantSnapshot,
    req: &DeathSaveReq,
) -> Result<DeathSaveResult, String> {
    let mut rng = StdRng::from_os_rng();

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        "2d20kh1".to_string()
    } else if effective_dis {
        "2d20kl1".to_string()
    } else {
        "1d20".to_string()
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("death save roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    let nat20 = natural == 20;
    let nat1 = natural == 1;

    // Read current death saves from sheet
    let ds = snap.sheet_raw.get("death_saves");
    let successes_before = ds.and_then(|d| d.get("successes"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(0, 3) as i32;
    let failures_before = ds.and_then(|d| d.get("failures"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(0, 3) as i32;

    let mut successes_after = successes_before;
    let mut failures_after = failures_before;
    let mut hp_after = snap.hp_current;
    let mut alive = true;
    let mut stabilized = false;
    let mut died = false;

    if nat20 {
        // Regain 1 HP, stable
        hp_after = 1;
        alive = true;
        stabilized = true;
        successes_after = 0;
        failures_after = 0;
    } else if nat1 {
        failures_after = (failures_before + 2).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    } else if natural >= 10 {
        successes_after = (successes_before + 1).min(3);
        if successes_after >= 3 {
            stabilized = true;
            alive = true;
            successes_after = 0;
            failures_after = 0;
        }
    } else {
        failures_after = (failures_before + 1).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    }

    Ok(DeathSaveResult {
        natural_roll: natural,
        passed: natural >= 10 && !nat1,
        successes_before,
        failures_before,
        successes_after,
        failures_after,
        stabilized,
        died,
        nat20,
        nat1,
        hp_after,
        alive,
    })
}

pub fn resolve_skill_check(
    snap: &CombatantSnapshot,
    req: &SkillCheckReq,
    stats: &ComputedStats,
) -> Result<SkillCheckResult, String> {
    let mut rng = StdRng::from_os_rng();
    let skill = req.skill.to_lowercase().replace(' ', "_");

    let pb = if snap.proficiency_bonus > 0 {
        snap.proficiency_bonus
    } else {
        proficiency_from_level(snap.level_total)
    };

    let skill_prof_for_jack = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient_for_jack = matches!(skill_prof_for_jack, Some("prof") | Some("proficient") | Some("expert"));

    let modv = stats.skill_mods.iter()
        .find(|(s, _)| s == &skill)
        .map(|(_, m)| if !is_proficient_for_jack && stats.jack_of_all_trades {
            *m + (pb / 2)
        } else {
            *m
        })
        .unwrap_or_else(|| {
            // fallback: try ability mod based on skill name
            let ability = skill_ability(&skill);
            let base = ability_mod(snap, ability);
            if stats.jack_of_all_trades { base + pb / 2 } else { base }
        });

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        format!("2d20kh1+{}", modv)
    } else if effective_dis {
        format!("2d20kl1+{}", modv)
    } else {
        format!("1d20+{}", modv)
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("skill check roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    // Reliable Talent (Rogue 11+): treat any d20 ≤9 as 10 for proficient/expert skills
    let has_reliable_talent = snap.classes.as_array().map(|arr| {
        arr.iter().any(|c| {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let level = c.get("level").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
            name == "rogue" && level >= 11
        })
    }).unwrap_or(false);
    let skill_prof = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient = matches!(skill_prof, Some("prof") | Some("proficient") | Some("expert"));
    let total = if has_reliable_talent && is_proficient && natural < 10 {
        roll_res.total - natural + 10
    } else {
        roll_res.total
    };

    let passed = req.dc.map(|dc| total >= dc);

    Ok(SkillCheckResult {
        skill: req.skill.clone(),
        natural_roll: natural,
        total,
        dc: req.dc,
        passed,
        advantage: effective_adv,
        disadvantage: effective_dis,
    })
}

fn skill_ability(skill: &str) -> &str {
    match skill {
        "athletics" => "str",
        "acrobatics" | "sleight_of_hand" | "stealth" => "dex",
        "arcana" | "history" | "investigation" | "nature" | "religion" => "int",
        "animal_handling" | "insight" | "medicine" | "perception" | "survival" => "wis",
        "deception" | "intimidation" | "performance" | "persuasion" => "cha",
        _ => "wis",
    }
}

// =====================================================================
// Helpers
// =====================================================================

pub fn apply_damage_type(raw: i32, dtype: &str, stats: &ComputedStats, is_magical: bool) -> (i32, bool, bool, bool) {
    if stats.immunities.contains(dtype) || stats.immunities.contains("all") {
        return (0, false, false, true);
    }
    if stats.immunities.contains("nonmagical") && !is_magical {
        return (0, false, false, true);
    }
    // PHB p.197: resistance and vulnerability cancel each other out.
    let has_resistance = stats.resistances.contains(dtype) || stats.resistances.contains(&"all".to_string());
    let has_vulnerability = stats.vulnerabilities.contains(dtype) || stats.vulnerabilities.contains(&"all".to_string());
    if has_vulnerability && has_resistance {
        return (raw, false, false, false);
    }
    if has_vulnerability {
        return (raw * 2, false, true, false);
    }
    if has_resistance {
        return ((raw as f32 / 2.0).floor() as i32, true, false, false);
    }
    if stats.resistances.contains("nonmagical") && !is_magical {
        return ((raw as f32 / 2.0).floor() as i32, true, false, false);
    }
    // Heavy Armor Master: -3 to nonmagical B/P/S
    if stats.nonmagical_damage_reduction > 0 && !is_magical
        && matches!(dtype, "bludgeoning" | "piercing" | "slashing")
    {
        let reduced = (raw - stats.nonmagical_damage_reduction).max(0);
        return (reduced, false, false, false);
    }
    (raw, false, false, false)
}

/// PHB p.197: if damage from a single hit equals or exceeds HP max, instant death (no death saves).
pub fn is_massive_damage(hp_max: i32, damage_applied: i32) -> bool {
    hp_max > 0 && damage_applied >= hp_max
}

pub fn apply_hp_damage(hp: i32, temp: i32, dmg: i32) -> (i32, i32) {
    if dmg <= 0 { return (hp, temp); }
    let remaining = dmg - temp;
    if remaining <= 0 {
        (hp, temp - dmg)
    } else {
        (hp - remaining, 0)
    }
}

pub fn concentration_check(target: &CombatantSnapshot, damage: i32, rng: &mut StdRng) -> (bool, RollResult) {
    // DC = max(10, floor(damage / 2))
    let dc = (damage / 2).max(10);
    let con_mod = ability_mod(target, "con");
    let has_war_caster = target.sheet_raw.get("feats")
        .and_then(|v| v.as_array())
        .map(|feats| feats.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("war_caster")))
        .unwrap_or(false);
    let expr = if has_war_caster {
        format!("2d20kh1+{}", con_mod)
    } else {
        format!("1d20+{}", con_mod)
    };
    let roll_res = match roll(&expr, rng) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("concentration_check roll failed: {e}; defaulting to broken");
            return (true, crate::dice::RollResult { expression: expr, terms: vec![], total: 0 });
        }
    };
    let broken = roll_res.total < dc;
    (broken, roll_res)
}

/// Double the number of dice in an expression for critical hits.
/// "1d8+3" → "2d8+3", "2d6+1d4+5" → "4d6+2d4+5"
pub fn crit_double_dice(expr: &str) -> String {
    let cleaned: String = expr.chars().filter(|c| !c.is_whitespace()).collect();
    let mut result = String::new();
    let mut i = 0;
    let chars: Vec<char> = cleaned.chars().collect();
    while i < chars.len() {
        // Look for NdS pattern
        if let Some(d_idx) = chars[i..].iter().position(|&c| c == 'd' || c == 'D') {
            let d_abs = i + d_idx;
            // Try to parse number before d
            let mut num_start = d_abs;
            while num_start > i && chars[num_start - 1].is_ascii_digit() {
                num_start -= 1;
            }
            if num_start < d_abs {
                let num_str: String = chars[num_start..d_abs].iter().collect();
                if let Ok(n) = num_str.parse::<u32>() {
                    // Append everything before num_start
                    result.extend(chars[i..num_start].iter());
                    result.push_str(&(n * 2).to_string());
                    result.push('d');
                    i = d_abs + 1;
                    continue;
                }
            } else {
                // Implicit dN (e.g. "d8"): treat as 1dN → 2dN
                result.push('2');
                result.push(chars[d_abs]);
                i = d_abs + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    if result.is_empty() { result = expr.to_string(); }
    result
}

