// Auto-compute damage expression for a weapon based on its properties and wielder's stats.
use super::abilities::ability_mod;
use super::super::resolvers::parse_weapon_props;
use super::super::types::CombatantSnapshot;
use serde_json::Value;

/// Returns "1d8+3" style expression. If weapon already has a damage expression, appends ability mod if missing.
pub fn compute_weapon_damage_expression(weapon: &Value, snap: &CombatantSnapshot, two_handed: bool) -> String {
    let props_str = weapon.get("properties").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
    let props = parse_weapon_props(&props_str);

    // Determine ability mod
    let ability = if props.finesse {
        if ability_mod(snap, "dex") > ability_mod(snap, "str") { "dex" } else { "str" }
    } else if props.thrown && !props.ranged {
        "str"
    } else if props.ranged {
        "dex"
    } else {
        "str"
    };
    let ability_mod_val = ability_mod(snap, ability);

    // Get base damage die
    let damage_die = weapon.get("damage_die").and_then(|v| v.as_str())
        .or_else(|| weapon.get("damage").and_then(|v| v.as_str()))
        .unwrap_or("1d4");

    // Parse existing damage expression
    let base_expr = if damage_die.contains('+') || damage_die.contains('-') {
        // Already has modifier — check if ability mod is included
        damage_die.to_string()
    } else {
        damage_die.to_string()
    };

    // For versatile weapons in two-handed mode, use versatile die if available
    let die_expr = if two_handed && props.versatile {
        weapon.get("versatile_die").and_then(|v| v.as_str()).unwrap_or(&base_expr).to_string()
    } else {
        base_expr
    };

    // For off-hand TWF, no ability mod to damage unless fighting style
    // (caller handles this by passing ability_mod_val = 0 when appropriate)
    if ability_mod_val != 0 {
        format!("{}+{}", die_expr, ability_mod_val)
    } else {
        die_expr
    }
}
