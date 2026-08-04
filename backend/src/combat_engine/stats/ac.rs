// AC computation from armor + shield + dex mod (with armor max dex cap).
use super::abilities::ability_mod;
use super::super::types::CombatantSnapshot;

pub fn compute_ac_from_sheet(snap: &CombatantSnapshot) -> i32 {
    let shield_bonus = if snap.sheet_raw.get("shield").and_then(|v| v.as_bool()).unwrap_or(false) { 2 } else { 0 };
    let base = if snap.sheet_raw.get("ac_manual").and_then(|v| v.as_bool()).unwrap_or(false) {
        // L1: manual AC edit overrides armor-based computation (matches
        // frontend computeAC). base_ac is the synced sheet.ac.
        (snap.base_ac + shield_bonus).max(1)
    } else if let Some(armor) = snap.sheet_raw.get("armor").and_then(|v| v.as_object()) {
        let armor_type = armor.get("type").and_then(|v| v.as_str()).unwrap_or("");
        let dex_mod = ability_mod(snap, "dex");

        let base_ac = match armor_type {
            "unarmored_barbarian" => 10 + dex_mod + ability_mod(snap, "con"),
            "unarmored_monk" => 10 + dex_mod + ability_mod(snap, "wis"),
            "mage_armor" | "draconic" => 13 + dex_mod,
            "natural" => {
                let ac_base = armor.get("ac_base").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(10);
                // R6: absent max_dex = uncapped (was 0, silently dropping
                // the DEX mod for homebrew "15+DEX" natural armor).
                let max_dex = armor.get("max_dex").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(99);
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
        (base_ac + shield_bonus).max(1)
    } else {
        // L2: no armor config → flat AC still gains the shield bonus (PHB).
        (snap.base_ac + shield_bonus).max(1)
    };

    // H10: sheet-level modifiers that also apply to the flat fallback —
    // matches frontend computedAC().
    let mut ac = base;
    if let Some(n) = snap.sheet_raw.get("ac_bonus").and_then(|v| v.as_i64()) {
        ac += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    }
    // Dual Wielder (PHB p.166): +1 AC while wielding two melee weapons.
    let has_dual_wielder = snap
        .sheet_raw
        .get("feats")
        .and_then(|f| f.as_array())
        .map(|arr| {
            arr.iter()
                .any(|f| f.get("key").and_then(|k| k.as_str()) == Some("dual_wielder"))
        })
        .unwrap_or(false);
    if has_dual_wielder {
        // R6: melee = not ranged (PHB) — the old range-string heuristic
        // missed thrown melee weapons (handaxe "20/60").
        let melee_count = snap.weapons.as_array().map(|ws| {
            ws.iter()
                .filter(|w| {
                    let equipped = w.get("equipped").and_then(|v| v.as_bool()).unwrap_or(true);
                    let props = w.get("properties").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                    equipped && !props.contains("ranged")
                })
                .count()
        }).unwrap_or(0);
        if melee_count >= 2 {
            ac += 1;
        }
    }
    ac
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
