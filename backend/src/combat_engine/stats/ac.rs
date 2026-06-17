// AC computation from armor + shield + dex mod (with armor max dex cap).
use super::abilities::ability_mod;
use super::super::types::CombatantSnapshot;

pub fn compute_ac_from_sheet(snap: &CombatantSnapshot) -> i32 {
    // Check for structured armor config in raw sheet
    if let Some(armor) = snap.sheet_raw.get("armor").and_then(|v| v.as_object()) {
        let armor_type = armor.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let dex_mod = ability_mod(snap, "dex");
        let shield_bonus = if snap.sheet_raw.get("shield").and_then(|v| v.as_bool()).unwrap_or(false) { 2 } else { 0 };

        let base_ac = match armor_type {
            "unarmored_barbarian" => 10 + dex_mod + ability_mod(snap, "con"),
            "unarmored_monk" => 10 + dex_mod + ability_mod(snap, "wis"),
            "mage_armor" | "draconic" => 13 + dex_mod,
            "natural" => {
                let ac_base = armor.get("ac_base").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(10);
                let max_dex = armor.get("max_dex").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
                ac_base + dex_mod.min(max_dex)
            }
            _ => {
                // Regular armor: ac_base + min(dex_mod, max_dex) + shield
                let ac_base = armor.get("ac_base").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(10);
                let armor_max_dex = armor.get("max_dex").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(99);
                let max_dex = if armor_type == "medium" {
                    snap.sheet_raw.get("medium_armor_max_dex_override")
                        .and_then(|v| v.as_i64())
                        .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                        .unwrap_or(armor_max_dex)
                } else {
                    armor_max_dex
                };
                ac_base + dex_mod.min(max_dex)
            }
        };
        return (base_ac + shield_bonus).max(1);
    }

    // Fallback to flat AC from sheet
    snap.base_ac.max(1)
}

/// Parse ac_base strings like "13+dex", "15+con", "10+dex+shield"
pub fn parse_ac_base(expr: &str, snap: &CombatantSnapshot) -> Option<i32> {
    let mut total: i32 = 0;
    for part in expr.split('+') {
        let p = part.trim().to_lowercase();
        if let Ok(n) = p.parse::<i32>() {
            total += n;
        } else if ["str", "dex", "con", "int", "wis", "cha"].contains(&p.as_str()) {
            total += ability_mod(snap, &p);
        } else if p == "shield" {
            total += 2;
        }
    }
    Some(total.max(1))
}
