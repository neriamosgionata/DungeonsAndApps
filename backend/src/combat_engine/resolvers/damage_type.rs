use super::super::stats::ability_mod;
use super::super::types::{CombatantSnapshot, ComputedStats};
use crate::dice::{RollResult, roll};
use rand::rngs::StdRng;

pub fn apply_damage_type(
    raw: i32,
    dtype: &str,
    stats: &ComputedStats,
    is_magical: bool,
) -> (i32, bool, bool, bool) {
    if stats.immunities.contains(dtype) || stats.immunities.contains(&"all".to_string()) {
        return (0, false, false, true);
    }
    if stats.immunities.contains("nonmagical") && !is_magical {
        return (0, false, false, true);
    }
    // PHB p.197: resistance and vulnerability cancel each other out.
    let has_resistance =
        stats.resistances.contains(dtype) || stats.resistances.contains(&"all".to_string());
    let has_vulnerability =
        stats.vulnerabilities.contains(dtype) || stats.vulnerabilities.contains(&"all".to_string());
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
    if stats.nonmagical_damage_reduction > 0
        && !is_magical
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
    if dmg <= 0 {
        return (hp, temp);
    }
    let remaining = dmg - temp;
    if remaining <= 0 {
        (hp, temp - dmg)
    } else {
        // HIGH-5: HP cannot go below 0. PHB p.197: 0-HP targets taking damage
        // drop a death-save failure (handler-side), but HP itself stays at 0.
        // saturating_sub avoids i32 underflow if hp == 0 already.
        (hp.saturating_sub(remaining).max(0), 0)
    }
}

pub fn concentration_check(
    target: &CombatantSnapshot,
    damage: i32,
    rng: &mut StdRng,
) -> (bool, RollResult) {
    // DC = max(10, floor(damage / 2))
    let dc = (damage / 2).max(10);
    let con_mod = ability_mod(target, "con");
    let has_war_caster = target
        .sheet_raw
        .get("feats")
        .and_then(|v| v.as_array())
        .map(|feats| {
            feats
                .iter()
                .any(|f| f.get("key").and_then(|k| k.as_str()) == Some("war_caster"))
        })
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
            return (
                true,
                crate::dice::RollResult {
                    expression: expr,
                    terms: vec![],
                    total: 0,
                },
            );
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
    if result.is_empty() {
        result = expr.to_string();
    }
    result
}
