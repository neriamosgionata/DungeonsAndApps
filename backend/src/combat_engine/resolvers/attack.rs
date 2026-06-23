use super::super::stats::{
    ability_mod, compute_weapon_damage_expression, proficiency_from_level,
};
use super::super::types::{CombatantSnapshot, ComputedStats};
use super::damage_type::{
    apply_damage_type, apply_hp_damage, concentration_check, crit_double_dice,
};
use super::types::{AttackReq, AttackResult, find_weapon};
use crate::dice::roll;
use rand::{SeedableRng, rngs::StdRng};

pub fn resolve_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    req: &AttackReq,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    // Determine cover bonus. HIGH-3: "full" cover = PHB p.150 total cover
    // (target can't be targeted directly) — previously fell through to 0
    // bonus, allowing attacks to hit normally. Reject the attack instead.
    let cover_bonus = match req.cover.as_deref() {
        Some("full") => {
            return Err("target has total cover and cannot be targeted directly".into());
        }
        Some("half") => 2,
        Some("three_quarters") => 5,
        _ => 0,
    };

    // Determine advantage/disadvantage
    let mut adv = req.advantage || attacker_stats.attack_advantage;
    let mut dis = req.disadvantage || attacker_stats.attack_disadvantage;

    // Resolve weapon properties early so prone/ranged checks can use them
    let weapon = req
        .weapon_id
        .as_deref()
        .and_then(|wid| find_weapon(attacker, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
    let is_ranged_attack = weapon_props.ranged || weapon_props.thrown || req.is_spell_attack;

    // Target conditions affect attacker
    if target_stats.prone {
        // HIGH-2: 1 cell on a default 50-px grid = 20% of the map (5ft).
        // PHB p.292: melee attack on prone target = adv, ranged = dis.
        let within_5ft = if let (Some(ax), Some(ay), Some(tx), Some(ty)) = (
            attacker.token_x,
            attacker.token_y,
            target.token_x,
            target.token_y,
        ) {
            let d_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
            d_pct < 20.0
        } else {
            true
        };
        if within_5ft {
            adv = true;
        } else {
            dis = true;
        }
    }
    if target_stats.invisible {
        dis = true;
    }
    // Sprint 38 HIGH-3: PHB p.195 — "When a creature can't see you, you
    // have advantage on attack rolls against it." A blinded target can't
    // see the attacker, so the attacker rolls with advantage. Also
    // covers heavily-obscured cases where the target has the blinded
    // condition from any source (magical darkness, fog, gaze).
    if target_stats.blinded {
        adv = true;
    }
    if target_stats.paralyzed
        || target_stats.unconscious
        || target_stats.restrained
        || target_stats.stunned
    {
        // MED-3: PHB p.292 — attacks against stunned also have advantage
        // (was: only paralyzed/unconscious/restrained).
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
    // L15: Frightened attacker has disadvantage ONLY if the source is in line
    // of sight (PHB p.290). Blindness breaks LOS — a blinded attacker can't
    // see the source of fear, so no disadvantage. Full source-of-fear tracking
    // (per-effect source_combatant_id) is a follow-up; for now blindness is
    // the strongest LOS check we can do without schema changes.
    if attacker_stats.frightened && !attacker_stats.blinded {
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
    let archery_bonus =
        if attacker_stats.archery_style && (weapon_props.ranged || weapon_props.thrown) {
            2
        } else {
            0
        };
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
            if weapon_props.thrown && !weapon_props.ranged {
                "str"
            } else if weapon_props.ranged {
                "dex"
            } else {
                "str"
            }
        });
        let ability_mod = if weapon_props.finesse {
            ability_mod(attacker, "str").max(ability_mod(attacker, "dex"))
        } else {
            ability_mod(attacker, ability)
        };
        let prof = if req.proficient.unwrap_or(true) {
            pb
        } else {
            0
        };
        let bonus = attacker_stats.attack_bonus + archery_bonus + power_attack_penalty;

        // Bless: +1d4 (or +Nd4 if multiple bless sources)
        let bless_str = if let Some(n) = req.bless_dice.filter(|&n| n > 0) {
            if n == 1 {
                "+1d4".to_string()
            } else {
                format!("+{}d4", n)
            }
        } else {
            String::new()
        };

        // Bardic Inspiration: +1dX
        let bardic_str = if let Some(die) = req.bardic_inspiration_dice {
            format!("+1d{}", die)
        } else {
            String::new()
        };

        if effective_adv {
            format!(
                "2d20kh1+{}+{}+{}{}{}",
                ability_mod, prof, bonus, bless_str, bardic_str
            )
        } else if effective_dis {
            format!(
                "2d20kl1+{}+{}+{}{}{}",
                ability_mod, prof, bonus, bless_str, bardic_str
            )
        } else {
            format!(
                "1d20+{}+{}+{}{}{}",
                ability_mod, prof, bonus, bless_str, bardic_str
            )
        }
    };

    let attack_roll =
        roll(&attack_expr, &mut rng).map_err(|e| format!("attack roll error: {}", e))?;

    // Determine natural roll (kept die for adv/dis, first roll for straight rolls)
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
    let mut critical = natural_roll >= crit_range;
    // PHB p.292: an attack against a paralyzed or unconscious target within
    // 5ft is automatically a critical hit.
    if !critical {
        // HIGH-2: 1 cell = 20% of map = 5ft (default 50-px grid).
        let within_5ft = if let (Some(ax), Some(ay), Some(tx), Some(ty)) = (
            attacker.token_x,
            attacker.token_y,
            target.token_x,
            target.token_y,
        ) {
            let d_pct = ((ax - tx).powi(2) + (ay - ty).powi(2)).sqrt();
            d_pct < 20.0
        } else {
            // No positions set: assume melee range.
            true
        };
        if within_5ft && (target_stats.paralyzed || target_stats.unconscious) {
            critical = true;
        }
    }
    let auto_miss = natural_roll == 1;

    let target_ac = target_stats.ac + cover_bonus;
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
        } else if let Some((weapon, _)) = req
            .weapon_id
            .as_deref()
            .and_then(|wid| find_weapon(attacker, wid))
        {
            compute_weapon_damage_expression(weapon, attacker, false)
        } else {
            // Default: unarmed strike = 1 + str mod (min 1 total)
            let str_mod = ability_mod(attacker, "str");
            let base = (1 + str_mod).max(1);
            format!("{}", base)
        };

        let mut dmg_roll =
            roll(&dmg_expr, &mut rng).map_err(|e| format!("damage roll error: {}", e))?;

        // GWF: reroll weapon damage once if any die landed 1 or 2
        // Only applies to melee weapons; take the better of two rolls
        if attacker_stats.gwf_style && !weapon_props.ranged && !weapon_props.thrown {
            let has_low = dmg_roll
                .terms
                .iter()
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
            dmg_roll =
                roll(&crit_expr, &mut rng).map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        // Savage Attacks (Half-orc): extra weapon die on crit
        let savage_bonus = if critical && attacker_stats.savage_attacks {
            let die = req.damage_die.as_deref().unwrap_or("d6");
            roll(&format!("1{}", die), &mut rng)
                .map(|r| r.total)
                .unwrap_or(0)
        } else {
            0
        };

        // Dueling style: +2 damage when wielding a one-handed weapon and no off-hand weapon
        // (simplified: +2 if not two-handed and not ranged)
        let dueling_bonus = if attacker_stats.dueling_style
            && !weapon_props.two_handed
            && !weapon_props.ranged
            && !weapon_props.thrown
        {
            2
        } else {
            0
        };

        // Power attack (Sharpshooter / GWM): +10 damage on hit
        let power_attack_damage = if req.power_attack { 10 } else { 0 };

        let raw_dmg = dmg_roll.total
            + attacker_stats.damage_bonus
            + attacker_stats.weapon_damage_bonus
            + savage_bonus
            + dueling_bonus
            + power_attack_damage;
        let dtype = req.damage_type.to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) =
            apply_damage_type(raw_dmg, &dtype, target_stats, req.is_magical);

        // Extra damage (Sneak Attack, Divine Smite, Rage, etc.)
        // PHB p.196: all attack damage dice are doubled on a critical hit.
        let (extra_applied, extra_dtype) = if let Some(ref extra_expr) = req.extra_damage_expression
        {
            let expr = if critical {
                crit_double_dice(extra_expr)
            } else {
                extra_expr.clone()
            };
            let extra_roll =
                roll(&expr, &mut rng).map_err(|e| format!("extra damage roll error: {}", e))?;
            let extra_raw = extra_roll.total;
            let extra_type = req.extra_damage_type.as_deref().unwrap_or("piercing");
            let (extra_eff, _, _, _) =
                apply_damage_type(extra_raw, extra_type, target_stats, req.is_magical);
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
