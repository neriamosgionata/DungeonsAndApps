// Ability scores, proficiency, racial bonuses, save proficiency, casting ability.
use super::super::types::CombatantSnapshot;
use std::collections::HashMap;

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

pub fn save_proficient(snap: &CombatantSnapshot, ability: &str) -> bool {
    snap.saves.get(ability).and_then(|v| v.as_bool()).unwrap_or(false)
}

pub fn casting_ability(snap: &CombatantSnapshot) -> String {
    casting_ability_from_classes(snap)
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

pub fn proficiency_from_level(level: i32) -> i32 {
    2 + ((level.max(1) - 1) / 4)
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
        "goblin" => { bonuses.insert("str".into(), 2); bonuses.insert("dex".into(), 1); }
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
        bonuses.insert("dex".into(), 1);
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
        bonuses.insert("dex".into(), 1);
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
