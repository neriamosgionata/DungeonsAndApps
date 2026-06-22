// Request/result structs for combat resolution + weapon property helpers.

use super::super::types::CombatantSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize)]
pub struct AttackReq {
    pub target_id: uuid::Uuid,
    // -- rest of fields below --
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

pub fn find_weapon<'a>(
    snapshot: &'a CombatantSnapshot,
    weapon_id: &str,
) -> Option<(&'a serde_json::Value, WeaponProps)> {
    if let Some(arr) = snapshot.weapons.as_array() {
        for w in arr {
            if w.get("id").and_then(|v| v.as_str()) == Some(weapon_id)
                || w.get("name").and_then(|v| v.as_str()) == Some(weapon_id)
            {
                let props = w
                    .get("properties")
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

// Re-export RollResult for the structs that use it
use crate::dice::RollResult;
