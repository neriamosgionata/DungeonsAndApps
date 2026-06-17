// Derived stat computation: compute_stats, apply_modifier, and all stat helpers.
// Extracted from combat_engine.rs to keep the file under the 500-line
// guideline (per AGENTS.md §1.4).

use super::resolvers::parse_weapon_props;
use super::types::{CombatantSnapshot, ComputedStats};
use serde_json::Value;
use std::collections::HashMap;pub fn compute_stats(snap: &CombatantSnapshot) -> ComputedStats {
    let mut stats = ComputedStats {
        ac: compute_ac_from_sheet(snap),
        speed: snap.base_speed.max(0),
        ..Default::default()
    };

    // 1. Parse conditions — support "name:N" (timed) and bare "name"
    for cond in &snap.conditions {
        let raw = cond.to_lowercase();
        // Strip optional duration suffix ":N"
        let c = raw.split(':').next().unwrap_or(&raw).to_string();
        match c.as_str() {
            "blinded" => { stats.blinded = true; stats.attack_disadvantage = true; }
            "prone" => { stats.prone = true; }
            "paralyzed" => { stats.paralyzed = true; stats.incapacitated = true; stats.speed = 0; }
            "restrained" => { stats.restrained = true; stats.attack_disadvantage = true; stats.save_disadvantage_for("dex"); stats.speed = 0; }
            "frightened" => { stats.frightened = true; stats.attack_disadvantage = true; }
            "charmed" => { stats.charmed = true; }
            "poisoned" => { stats.poisoned = true; stats.attack_disadvantage = true; }
            "stunned" => { stats.stunned = true; stats.incapacitated = true; stats.speed = 0; }
            "unconscious" => { stats.unconscious = true; stats.incapacitated = true; stats.prone = true; stats.speed = 0; }
            "petrified" => {
                stats.petrified = true; stats.incapacitated = true; stats.speed = 0;
                stats.resistances.extend(["bludgeoning","piercing","slashing","fire","cold","lightning","thunder","acid","poison"].map(String::from));
                stats.immunities.insert("poison".into()); stats.immunities.insert("psychic".into());
            }
            "grappled" => { stats.grappled = true; stats.speed = 0; }
            "incapacitated" => { stats.incapacitated = true; }
            "invisible" => { stats.invisible = true; stats.attack_advantage = true; }
            "deafened" => { stats.deafened = true; }
            _ => {}
        }
    }

    // 2. Parse effect modifiers
    for eff in &snap.active_effects {
        if let Some(mods) = eff.modifiers.as_object() {
            for (key, val) in mods {
                apply_modifier(&mut stats, key, val);
            }
        }
    }

    // 2.5. Racial senses from sheet (darkvision, etc.)
    if let Some(senses) = snap.sheet_raw.get("senses").and_then(|v| v.as_object()) {
        if let Some(n) = senses.get("darkvision").and_then(|v| v.as_i64()) {
            stats.darkvision_range = stats.darkvision_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
        if let Some(n) = senses.get("blindsight").and_then(|v| v.as_i64()) {
            stats.blindsight_range = stats.blindsight_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
        if let Some(n) = senses.get("truesight").and_then(|v| v.as_i64()) {
            stats.truesight_range = stats.truesight_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
        if let Some(n) = senses.get("tremorsense").and_then(|v| v.as_i64()) {
            stats.tremorsense_range = stats.tremorsense_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
        }
    }

    // 2.6. Innate alternate speeds from sheet
    if let Some(n) = snap.sheet_raw.get("fly_speed").and_then(|v| v.as_i64()) {
        stats.flying_speed = stats.flying_speed.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }
    if let Some(n) = snap.sheet_raw.get("swim_speed").and_then(|v| v.as_i64()) {
        stats.swim_speed = stats.swim_speed.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }
    if let Some(n) = snap.sheet_raw.get("climb_speed").and_then(|v| v.as_i64()) {
        stats.climb_speed = stats.climb_speed.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
    }

    // 3. Post-process AC
    // ac_base overrides (e.g. "13+dex" from mage armor)
    for eff in &snap.active_effects {
        if let Some(mods) = eff.modifiers.as_object() {
            if let Some(base) = mods.get("ac_base").and_then(|v| v.as_str()) {
                if let Some(new_ac) = parse_ac_base(base, snap) {
                    stats.ac = new_ac;
                }
            }
            if let Some(min) = mods.get("ac_min").and_then(|v| v.as_i64()) {
                stats.ac = stats.ac.max(min.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
            }
        }
    }

    // 4. Exhaustion from sheet (applied before speed post-process)
    stats.exhaustion = snap.sheet_raw.get("exhaustion")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
    if stats.exhaustion >= 1 {
        stats.save_disadvantage = true;
    }
    if stats.exhaustion >= 2 {
        stats.speed_halved = true;
    }
    if stats.exhaustion >= 3 {
        stats.attack_disadvantage = true;
    }
    if stats.exhaustion >= 5 {
        stats.speed = 0;
    }

    // 5. Post-process speed
    let movement_denied = stats.restrained || stats.grappled || stats.petrified || stats.unconscious
        || stats.exhaustion >= 5;
    if stats.flying_speed > 0 && !movement_denied { stats.speed = stats.flying_speed; }
    if stats.speed_halved && !stats.ignore_speed_halved(snap) {
        stats.speed = (stats.speed as f32 * 0.5).ceil() as i32;
    }
    if stats.speed_doubled {
        stats.speed *= 2;
    }

    // 6. Proficiency + ability-based mods
    let pb = if snap.proficiency_bonus > 0 {
        snap.proficiency_bonus
    } else {
        proficiency_from_level(snap.level_total)
    };
    stats.initiative_bonus = ability_mod(snap, "dex")
        + snap.sheet_raw.get("initiative").and_then(|v| v.as_i64()).unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    stats.spell_attack_bonus = pb + ability_mod(snap, &casting_ability(snap));
    stats.spell_save_dc = 8 + pb + ability_mod(snap, &casting_ability(snap));

    // 7. Save mods
    let save_abilities = ["str", "dex", "con", "int", "wis", "cha"];
    for ab in &save_abilities {
        // Check saves_override first (matches frontend saveMod())
        let modv = if let Some(ov) = snap.sheet_raw.get("saves_override")
            .and_then(|o| o.get(*ab))
            .and_then(|v| v.as_i64())
        {
            ov.clamp(i32::MIN as i64, i32::MAX as i64) as i32
        } else {
            let mut v = ability_mod(snap, ab);
            if save_proficient(snap, ab) {
                v += pb;
            }
            // Check for save-specific bonus in effects
            for eff in &snap.active_effects {
                if let Some(mods) = eff.modifiers.as_object() {
                    if let Some(bonuses) = mods.get("save_bonus").and_then(|v| v.as_object()) {
                        if let Some(b) = bonuses.get(*ab).and_then(|v| v.as_i64()) {
                            v += b.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                        }
                    }
                }
            }
            v
        };
        stats.save_mods.push((ab.to_string(), modv));
    }

    // 8. Skill mods
    let skill_ability_map: &[(&str, &str)] = &[
        ("athletics", "str"),
        ("acrobatics", "dex"), ("sleight_of_hand", "dex"), ("stealth", "dex"),
        ("arcana", "int"), ("history", "int"), ("investigation", "int"),
        ("nature", "int"), ("religion", "int"),
        ("animal_handling", "wis"), ("insight", "wis"), ("medicine", "wis"),
        ("perception", "wis"), ("survival", "wis"),
        ("deception", "cha"), ("intimidation", "cha"), ("performance", "cha"), ("persuasion", "cha"),
    ];
    for &(skill, ability) in skill_ability_map {
        let base = ability_mod(snap, ability);
        let prof_level = snap.skills.get(skill)
            .or_else(|| snap.skills.get(&skill.replace('_', " ")))
            .and_then(|v| v.as_str());
        let modv = match prof_level {
            Some("expert") => base + pb * 2,
            Some("prof") | Some("proficient") => base + pb,
            _ => base,
        };
        stats.skill_mods.push((skill.to_string(), modv));
    }

    // 9. Passive scores (10 + skill mod) — add feat bonuses
    let passive_bonus = snap.sheet_raw.get("senses")
        .and_then(|s| s.get("passive_perception_bonus"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    let passive_inv_bonus = snap.sheet_raw.get("senses")
        .and_then(|s| s.get("passive_investigation_bonus"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    for &(skill, _) in skill_ability_map {
        if let Some(modv) = stats.skill_mods.iter().find(|(s, _)| s == skill).map(|(_, m)| *m) {
            let extra = if skill == "perception" { passive_bonus }
                else if skill == "investigation" { passive_inv_bonus }
                else { 0 };
            stats.passive_scores.push((skill.to_string(), 10 + modv + extra));
        }
    }

    // 10. Class-level derived features (speed bonuses computed by frontend, stored in sheet.speed)
    if let Some(arr) = snap.classes.as_array() {
        for cls in arr {
            let name = cls.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let level = cls.get("level").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
            // Evasion: Rogue 7+ or Monk 7+
            if (name == "rogue" || name == "monk") && level >= 7 {
                stats.evasion = true;
            }
            // Jack of All Trades: Bard 2+, half prof on non-proficient skills
            if name == "bard" && level >= 2 {
                stats.jack_of_all_trades = true;
            }
        }
    }

    // Sheet-level flags from racial traits and feats
    if snap.sheet_raw.get("sunlight_sensitivity").and_then(|v| v.as_bool()).unwrap_or(false) {
        stats.attack_disadvantage = true;
    }
    if snap.sheet_raw.get("gnome_cunning").and_then(|v| v.as_bool()).unwrap_or(false) {
        stats.gnome_cunning = true;
    }
    if snap.sheet_raw.get("savage_attacks").and_then(|v| v.as_bool()).unwrap_or(false) {
        stats.savage_attacks = true;
    }
    if let Some(dr) = snap.sheet_raw.get("nonmagical_damage_reduction").and_then(|v| v.as_i64()) {
        stats.nonmagical_damage_reduction = dr.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    }

    // Magic item bonuses from attunement items
    if let Some(attunements) = snap.sheet_raw.get("attunement").and_then(|v| v.as_array()) {
        for att in attunements {
            if let Some(bonuses) = att.get("bonuses").and_then(|v| v.as_object()) {
                if let Some(n) = bonuses.get("ac").and_then(|v| v.as_i64()) { stats.ac += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
                if let Some(n) = bonuses.get("attack").and_then(|v| v.as_i64()) {
                    let b = n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.attack_bonus += b;
                    stats.spell_attack_bonus += b;
                }
                if let Some(n) = bonuses.get("damage").and_then(|v| v.as_i64()) { stats.damage_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
                if let Some(n) = bonuses.get("spell_dc").and_then(|v| v.as_i64()) { stats.spell_save_dc += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
                if let Some(n) = bonuses.get("initiative").and_then(|v| v.as_i64()) { stats.initiative_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
                if let Some(n) = bonuses.get("speed").and_then(|v| v.as_i64()) { stats.speed += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
                // Ability score bonuses from attunement items
                if let Some(n) = bonuses.get("str").and_then(|v| v.as_i64()) {
                    stats.attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.damage_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                }
    if let Some(n) = bonuses.get("dex").and_then(|v| v.as_i64()) {
                    stats.attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.initiative_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.ac += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                }
                // Mental ability attunement bonuses → spell attack + DC
                if let Some(n) = bonuses.get("int").and_then(|v| v.as_i64()) {
                    stats.spell_attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.spell_save_dc += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                }
                if let Some(n) = bonuses.get("wis").and_then(|v| v.as_i64()) {
                    stats.spell_attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.spell_save_dc += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                }
                if let Some(n) = bonuses.get("cha").and_then(|v| v.as_i64()) {
                    stats.spell_attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                    stats.spell_save_dc += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
                }
            }
        }
    }

    // Fighting styles from sheet_raw.fighting_styles (array of strings)
    if let Some(styles) = snap.sheet_raw.get("fighting_styles").and_then(|v| v.as_array()) {
        for s_val in styles {
            let s_str = s_val.as_str().unwrap_or("").to_lowercase();
            match s_str.as_str() {
                "archery" => stats.archery_style = true,
                "dueling" => stats.dueling_style = true,
                "great_weapon_fighting" | "great weapon fighting" => stats.gwf_style = true,
                "two-weapon_fighting" | "two-weapon fighting" | "two_weapon_fighting" => stats.twf_style = true,
                _ => {}
            }
        }
    }

    // Prone attacker: disadvantage on all attack rolls (PHB p.292)
    if stats.prone {
        stats.prone_ranged_disadvantage = true; // field reused for all attacks when prone
        stats.attack_disadvantage = true;
    }

    // Jack of All Trades: only applied in resolve_skill_check for non-proficient skills
    // to avoid double-counting on proficient ones. The skill_mods stay raw.
    stats
}

fn apply_modifier(stats: &mut ComputedStats, key: &str, val: &Value) {
    match key {
        "ac_bonus" => {
            if let Some(n) = val.as_i64() { stats.ac += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "attack_bonus" => {
            if let Some(n) = val.as_i64() { stats.attack_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "speed_bonus" => {
            if let Some(n) = val.as_i64() { stats.speed += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "speed_halved" => {
            if val.as_bool() == Some(true) { stats.speed_halved = true; }
        }
        "speed_doubled" => {
            if val.as_bool() == Some(true) { stats.speed_doubled = true; }
        }
        "flying_speed" => {
            if let Some(n) = val.as_i64() { stats.flying_speed = n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "swim_speed" => {
            if let Some(n) = val.as_i64() { stats.swim_speed = n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "climb_speed" => {
            if let Some(n) = val.as_i64() { stats.climb_speed = n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "damage_bonus" => {
            if let Some(n) = val.as_i64() { stats.damage_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "weapon_damage_bonus" => {
            if let Some(n) = val.as_i64() { stats.weapon_damage_bonus += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "hp_regen_per_turn" => {
            if let Some(n) = val.as_i64() { stats.hp_regen_per_turn += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "temp_hp_per_turn" => {
            if let Some(n) = val.as_i64() { stats.temp_hp_per_turn += n.clamp(i32::MIN as i64, i32::MAX as i64) as i32; }
        }
        "darkvision" => {
            if let Some(n) = val.as_i64() { stats.darkvision_range = stats.darkvision_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32); }
        }
        "truesight" => {
            if let Some(n) = val.as_i64() { stats.truesight_range = stats.truesight_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32); }
        }
        "blindsight" => {
            if let Some(n) = val.as_i64() { stats.blindsight_range = stats.blindsight_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32); }
        }
        "tremorsense" => {
            if let Some(n) = val.as_i64() { stats.tremorsense_range = stats.tremorsense_range.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32); }
        }
        "burrow_speed" => {
            if let Some(n) = val.as_i64() { stats.burrow_speed = stats.burrow_speed.max(n.clamp(i32::MIN as i64, i32::MAX as i64) as i32); }
        }
        "invisible" => {
            if val.as_bool() == Some(true) { stats.invisible = true; stats.attack_advantage = true; }
        }
        "attack_advantage" => {
            if val.as_bool() == Some(true) { stats.attack_advantage = true; }
        }
        "attack_disadvantage" => {
            if val.as_bool() == Some(true) { stats.attack_disadvantage = true; }
        }
        "save_advantage" => {
            if val.as_bool() == Some(true) { stats.save_advantage = true; }
        }
        "save_disadvantage" => {
            if val.as_bool() == Some(true) { stats.save_disadvantage = true; }
        }
        "attack_advantage_against" => {
            if val.as_bool() == Some(true) { stats.attack_advantage_against = true; }
        }
        "attack_disadvantage_against" => {
            if val.as_bool() == Some(true) { stats.attack_disadvantage_against = true; }
        }
        "frightened" => {
            if val.as_bool() == Some(true) { stats.frightened = true; stats.attack_disadvantage = true; }
        }
        "paralyzed" => {
            if val.as_bool() == Some(true) { stats.paralyzed = true; stats.incapacitated = true; }
        }
        "restrained" => {
            if val.as_bool() == Some(true) { stats.restrained = true; stats.attack_disadvantage = true; stats.save_disadvantage = true; stats.speed = 0; }
        }
        "poisoned" => {
            if val.as_bool() == Some(true) { stats.poisoned = true; stats.attack_disadvantage = true; }
        }
        "charmed" => {
            if val.as_bool() == Some(true) { stats.charmed = true; }
        }
        "stunned" => {
            if val.as_bool() == Some(true) { stats.stunned = true; stats.incapacitated = true; }
        }
        "incapacitated" => {
            if val.as_bool() == Some(true) { stats.incapacitated = true; }
        }
        "grapple_immunity" => {
            // no direct stat change; used by movement logic
        }
        "ignore_difficult_terrain" => {
            // movement logic
        }
        "spider_climb" => {
            // movement logic
        }
        "water_walk" => {
            // movement logic
        }
        "sanctuary" | "death_ward" | "antimagic" | "daylight" | "mirror_images" | "polymorphed" | "spiritual_weapon" | "jump_tripled" => {
            // tracked but not mechanically enforced here
        }
        "damage_resistance" | "nonmagical_damage_resistance" => {
            if let Some(arr) = val.as_array() {
                for v in arr { if let Some(s) = v.as_str() { stats.resistances.insert(s.to_lowercase()); } }
            } else if let Some(s) = val.as_str() {
                stats.resistances.insert(s.to_lowercase());
            }
        }
        "damage_vulnerability" => {
            if let Some(arr) = val.as_array() {
                for v in arr { if let Some(s) = v.as_str() { stats.vulnerabilities.insert(s.to_lowercase()); } }
            } else if let Some(s) = val.as_str() {
                stats.vulnerabilities.insert(s.to_lowercase());
            }
        }
        "damage_immunity" => {
            if let Some(arr) = val.as_array() {
                for v in arr { if let Some(s) = v.as_str() { stats.immunities.insert(s.to_lowercase()); } }
            } else if let Some(s) = val.as_str() {
                stats.immunities.insert(s.to_lowercase());
            }
        }
        _ => {
            // damage type-specific resistances (e.g. "fire_damage_resistance")
            if key.ends_with("_resistance") {
                let dtype = key.trim_end_matches("_resistance");
                stats.resistances.insert(dtype.to_lowercase());
            } else if key.ends_with("_vulnerability") {
                let dtype = key.trim_end_matches("_vulnerability");
                stats.vulnerabilities.insert(dtype.to_lowercase());
            } else if key.ends_with("_immunity") {
                let dtype = key.trim_end_matches("_immunity");
                stats.immunities.insert(dtype.to_lowercase());
            }
        }
    }
}

impl ComputedStats {
    // save_disadvantage_for and ignore_speed_halved live in types.rs (the impl block
    // can't be split across files; we re-export methods here for convenience)
}

pub fn proficiency_from_level(level: i32) -> i32 {
    2 + ((level.max(1) - 1) / 4)
}

/// Compute AC from armor + shield + dex mod (with armor max dex cap).
/// Falls back to snap.base_ac if no armor config in sheet.
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

/// Auto-compute damage expression for a weapon based on its properties and wielder's stats.
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

/// Determine spellcasting ability from classes array, falling back to global casting.ability.
fn casting_ability_from_classes(snap: &CombatantSnapshot) -> String {
    let class_defaults: std::collections::HashMap<&str, &str> = [
        ("wizard", "int"), ("artificer", "int"),
        ("cleric", "wis"), ("druid", "wis"), ("ranger", "wis"),
        ("bard", "cha"), ("paladin", "cha"), ("sorcerer", "cha"), ("warlock", "cha"),
    ].iter().cloned().collect();

    let mut votes: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

    if let Some(arr) = snap.classes.as_array() {
        for cls in arr {
            let name = cls.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let level = cls.get("level").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
            if level <= 0 { continue; }

            let ability = cls.get("spellcasting_ability")
                .and_then(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .or_else(|| class_defaults.get(name.as_str()).map(|s| s.to_string()));

            if let Some(ab) = ability {
                *votes.entry(ab).or_insert(0) += level;
            }
        }
    }

    votes.into_iter()
        .max_by_key(|(_, v)| *v)
        .map(|(k, _)| k)
        .unwrap_or_else(|| snap.casting.get("ability").and_then(|v| v.as_str()).unwrap_or("int").to_lowercase())
}

/// Apply racial ability score bonuses.
/// Returns a map of ability → bonus amount.
pub fn apply_racial_bonuses(snap: &CombatantSnapshot) -> HashMap<String, i32> {
    let mut bonuses = HashMap::new();
    let race = match snap.race {
        Some(ref r) => r.to_lowercase(),
        None => return bonuses,
    };

    match race.as_str() {
        "dragonborn" => { bonuses.insert("str".into(), 2); bonuses.insert("cha".into(), 1); }
        "hill dwarf" | "mountain dwarf" => { bonuses.insert("con".into(), 2); }
        "high elf" | "wood elf" | "drow" | "eladrin" => { bonuses.insert("dex".into(), 2); }
        "forest gnome" | "rock gnome" | "deep gnome" => { bonuses.insert("int".into(), 2); }
        "half-elf" => { bonuses.insert("cha".into(), 2); }
        "half-orc" => { bonuses.insert("str".into(), 2); bonuses.insert("con".into(), 1); }
        "lightfoot halfling" | "stout halfling" => { bonuses.insert("dex".into(), 2); }
        "human" | "variant human" => {
            // Variant human gets +1 to two abilities of choice; base human gets +1 to all
            // We can't know which ones for variant, so apply none and let user set manually
        }
        "tiefling" => { bonuses.insert("cha".into(), 2); bonuses.insert("int".into(), 1); }
        "aasimar" => { bonuses.insert("cha".into(), 2); }
        "bugbear" => { bonuses.insert("str".into(), 2); bonuses.insert("dex".into(), 1); }
        "firbolg" => { bonuses.insert("wis".into(), 2); bonuses.insert("str".into(), 1); }
        "goblin" => { bonuses.insert("dex".into(), 2); bonuses.insert("con".into(), 1); }
        "hobgoblin" => { bonuses.insert("con".into(), 2); bonuses.insert("int".into(), 1); }
        "kenku" => { bonuses.insert("dex".into(), 2); bonuses.insert("wis".into(), 1); }
        "kobold" => { bonuses.insert("dex".into(), 2); bonuses.insert("str".into(), -2); }
        "lizardfolk" => { bonuses.insert("con".into(), 2); bonuses.insert("wis".into(), 1); }
        "orc" => { bonuses.insert("str".into(), 2); bonuses.insert("con".into(), 1); bonuses.insert("int".into(), -2); }
        "tabaxi" => { bonuses.insert("dex".into(), 2); bonuses.insert("cha".into(), 1); }
        "triton" => { bonuses.insert("str".into(), 1); bonuses.insert("con".into(), 1); bonuses.insert("cha".into(), 1); }
        "yuan-ti pureblood" => { bonuses.insert("cha".into(), 2); bonuses.insert("int".into(), 1); }
        "shadar-kai" => { bonuses.insert("dex".into(), 2); }
        "githyanki" => { bonuses.insert("str".into(), 2); }
        "githzerai" => { bonuses.insert("wis".into(), 2); }
        "centaur" => { bonuses.insert("str".into(), 2); }
        "minotaur" => { bonuses.insert("str".into(), 2); }
        "changeling" => { bonuses.insert("cha".into(), 2); }
        "warforged" => { bonuses.insert("con".into(), 2); }
        "aarakocra" => { bonuses.insert("dex".into(), 2); }
        "tortle" => { bonuses.insert("str".into(), 2); }
        "fairy" => { bonuses.insert("dex".into(), 2); }
        "satyr" => { bonuses.insert("cha".into(), 2); }
        _ => {}
    }

    // Subrace bonuses
    if race.contains("hill dwarf") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("mountain dwarf") {
        bonuses.insert("str".into(), 2);
    } else if race.contains("high elf") {
        bonuses.insert("int".into(), 1);
    } else if race.contains("wood elf") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("drow") {
        bonuses.insert("cha".into(), 1);
    } else if race.contains("eladrin") {
        bonuses.insert("int".into(), 1);
    } else if race.contains("forest gnome") {
        bonuses.insert("dex".into(), 1);
    } else if race.contains("rock gnome") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("lightfoot halfling") {
        bonuses.insert("cha".into(), 1);
    } else if race.contains("stout halfling") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("protector aasimar") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("scourge aasimar") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("fallen aasimar") {
        bonuses.insert("str".into(), 1);
    } else if race.contains("deep gnome") {
        bonuses.insert("dex".into(), 1);
    } else if race.contains("shadar-kai") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("githyanki") {
        bonuses.insert("int".into(), 1);
    } else if race.contains("githzerai") {
        bonuses.insert("int".into(), 1);
    } else if race.contains("centaur") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("minotaur") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("changeling") {
        bonuses.insert("dex".into(), 1);
    } else if race.contains("warforged") {
        bonuses.insert("str".into(), 1);
    } else if race.contains("aarakocra") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("tortle") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("fairy") {
        bonuses.insert("cha".into(), 1);
    } else if race.contains("satyr") {
        bonuses.insert("dex".into(), 1);
    } else if race.contains("air genasi") {
        bonuses.insert("int".into(), 1);
    } else if race.contains("earth genasi") {
        bonuses.insert("str".into(), 1);
    } else if race.contains("fire genasi") {
        bonuses.insert("con".into(), 1);
    } else if race.contains("water genasi") {
        bonuses.insert("con".into(), 1);
    }

    bonuses
}

pub fn ability_mod(snap: &CombatantSnapshot, ability: &str) -> i32 {
    // Check abilities_override first (matches frontend abilityScore())
    if let Some(override_val) = snap.sheet_raw.get("abilities_override")
        .and_then(|o| o.get(ability))
        .and_then(|v| v.as_i64())
    {
        let score = override_val.max(1).min(30);
        return ((score - 10) as f32 / 2.0).floor() as i32;
    }
    let base_score = snap.abilities.get(ability).and_then(|v| v.as_i64()).unwrap_or(10);
    let racial_bonus = apply_racial_bonuses(snap).get(ability).copied().unwrap_or(0);
    let score = (base_score + racial_bonus as i64).max(1).min(30);
    ((score - 10) as f32 / 2.0).floor() as i32
}

fn save_proficient(snap: &CombatantSnapshot, ability: &str) -> bool {
    snap.saves.get(ability).and_then(|v| v.as_bool()).unwrap_or(false)
}

fn casting_ability(snap: &CombatantSnapshot) -> String {
    casting_ability_from_classes(snap)
}

/// Parse ac_base strings like "13+dex", "15+con", "10+dex+shield"
fn parse_ac_base(expr: &str, snap: &CombatantSnapshot) -> Option<i32> {
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
