// Max HP computation from class hit dice + CON mod per level.
use super::abilities::ability_mod;
use super::super::types::CombatantSnapshot;

/// Compute max HP from class hit dice + CON mod per level.
/// Uses average HP per die (d6=4, d8=5, d10=6, d12=7) for deterministic calc.
pub fn compute_max_hp_from_sheet(snap: &CombatantSnapshot) -> i32 {
    let con_mod = ability_mod(snap, "con");
    let mut total = 0;
    let mut first_class = true;

    if let Some(arr) = snap.classes.as_array() {
        for cls in arr {
            let level = cls.get("level").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(1);
            let die = cls.get("hit_die").and_then(|v| v.as_str()).unwrap_or("d8");
            let avg = match die {
                "d6" => 4,
                "d8" => 5,
                "d10" => 6,
                "d12" => 7,
                _ => 5,
            };
            let die_max = die.trim_start_matches('d').parse::<i32>().unwrap_or(8);
            if first_class {
                total += die_max + con_mod + (level - 1).max(0) * (avg + con_mod);
                first_class = false;
            } else {
                total += level * (avg + con_mod);
            }
        }
    }

    // Apply racial bonus (hill dwarf gets +1 HP per level)
    if let Some(ref race) = snap.race {
        if race.to_lowercase().contains("hill dwarf") {
            let level = snap.level_total.max(1);
            total += level;
        }
    }

    // Tough feat: +2 HP per level
    if let Some(feats) = snap.sheet_raw.get("feats").and_then(|v| v.as_array()) {
        if feats.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("tough")) {
            total += 2 * snap.level_total.max(1);
        }
    }

    // HP max reduction (wraith touch, etc.)
    let reduction = snap.sheet_raw.get("hp_max_reduction")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
    (total - reduction).max(1)
}
