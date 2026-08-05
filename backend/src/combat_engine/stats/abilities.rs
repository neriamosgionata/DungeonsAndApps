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

/// Extra Attack (PHB p.72/76): attacks allowed with one Attack action.
/// Fighter 5→2, 11→3, 20→4; Barbarian/Paladin/Ranger/Monk 5→2. Extra
/// Attack does NOT stack across classes (PHB p.164 — a Fighter 5 / Monk 5
/// attacks twice, not three times); the code takes the max. 1 = no extra.
pub fn extra_attack_count(snap: &CombatantSnapshot) -> i32 {
    let mut count = 1i32;
    if let Some(arr) = snap.classes.as_array() {
        for cls in arr {
            let name = cls
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_lowercase();
            let level = cls
                .get("level")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(0, 20) as i32)
                .unwrap_or(0);
            let own = match name.as_str() {
                "fighter" if level >= 20 => 4,
                "fighter" if level >= 11 => 3,
                "fighter" if level >= 5 => 2,
                "barbarian" | "paladin" | "ranger" | "monk" if level >= 5 => 2,
                _ => 1,
            };
            count = count.max(own);
        }
    }
    count
}

/// A17: creature size rank (1 tiny → 6 gargantuan). NPC `size` stat wins;
/// characters fall back to race (halfling/gnome/kobold/goblin/fairy = small).
pub fn creature_size(snap: &CombatantSnapshot) -> i32 {
    if let Some(sz) = snap.sheet_raw.get("size").and_then(|v| v.as_str()) {
        match sz.to_lowercase().as_str() {
            "tiny" => 1,
            "small" => 2,
            "medium" => 3,
            "large" => 4,
            "huge" => 5,
            "gargantuan" => 6,
            _ => 3,
        }
    } else if let Some(race) = &snap.race {
        let r = race.to_lowercase();
        if r.contains("halfling")
            || r.contains("gnome")
            || r.contains("kobold")
            || r.contains("goblin")
            || r.contains("fairy")
        {
            2
        } else {
            3
        }
    } else {
        3
    }
}

/// Apply racial ability score bonuses.
/// Returns a map of ability → bonus amount.
pub fn apply_racial_bonuses(snap: &CombatantSnapshot) -> HashMap<String, i32> {
    let mut bonuses = HashMap::new();
    let race = match snap.race {
        Some(ref r) => r.to_lowercase(),
        None => return bonuses,
    };

    // R6: base bonuses — exact match first, then composite-race fallback
    // ("High Elf (Sun Elf)" → the "high elf" entry). Longest key first so
    // substring conflicts resolve correctly ("variant human" beats "human",
    // "hobgoblin" beats "goblin", "half-orc" beats "orc"). Mirrors the
    // frontend racialAbilityBonus lookup.
    fn contains_both(race: &str, a: &str, b: &str) -> bool {
        race.contains(a) && race.contains(b)
    }
    let race_trim = race.trim();
    let base_races: &[(&str, &[(&str, i32)])] = &[
        ("yuan-ti pureblood", &[("cha", 2), ("int", 1)]),
        ("protector aasimar", &[("cha", 2)]),
        ("scourge aasimar", &[("cha", 2)]),
        ("fallen aasimar", &[("cha", 2)]),
        ("mountain dwarf", &[("con", 2)]),
        // L-4: generic labels — "Dwarf" must still grant CON +2, "Elf" DEX
        // +2, etc. (subrace rows above stay authoritative when present).
        ("dwarf", &[("con", 2)]),
        ("elf", &[("dex", 2)]),
        ("halfling", &[("dex", 2)]),
        ("gnome", &[("int", 2)]),
        ("half-orc", &[("str", 2), ("con", 1)]),
        ("half-elf", &[("cha", 2)]),
        ("dragonborn", &[("str", 2), ("cha", 1)]),
        ("tiefling", &[("cha", 2), ("int", 1)]),
        ("variant human", &[]), // +1 to two of choice; user sets manually (frontend same)
        ("lightfoot halfling", &[("dex", 2), ("cha", 1)]),
        ("forest gnome", &[("int", 2)]),
        ("rock gnome", &[("int", 2)]),
        ("deep gnome", &[("int", 2)]),
        ("water genasi", &[("wis", 2)]),
        ("earth genasi", &[("con", 2)]),
        ("hobgoblin", &[("con", 2), ("int", 1)]),
        ("half-orc", &[("str", 2), ("con", 1)]),
        ("half-elf", &[("cha", 2)]),
        ("stout halfling", &[("dex", 2), ("con", 1)]),
        ("lizardfolk", &[("con", 2), ("wis", 1)]),
        ("githyanki", &[("str", 2), ("int", 1)]),
        ("githzerai", &[("wis", 2), ("int", 1)]),
        ("shadar-kai", &[("dex", 2), ("con", 1)]),
        ("changeling", &[("cha", 2), ("dex", 1)]),
        ("air genasi", &[("dex", 2)]),
        ("fire genasi", &[("int", 2)]),
        ("dragonborn", &[("str", 2), ("cha", 1)]),
        ("aarakocra", &[("dex", 2), ("wis", 1)]),
        ("aasimar", &[("cha", 2)]),
        ("warforged", &[("con", 2), ("str", 1)]),
        ("lightfoot", &[("dex", 2), ("cha", 1)]),
        ("bugbear", &[("str", 2), ("dex", 1)]),
        ("centaur", &[("str", 2), ("wis", 1)]),
        ("minotaur", &[("str", 2), ("con", 1)]),
        ("tabaxi", &[("dex", 2), ("cha", 1)]),
        ("tortle", &[("str", 2), ("wis", 1)]),
        ("fairy", &[("dex", 2), ("cha", 1)]),
        ("satyr", &[("cha", 2), ("dex", 1)]),
        ("triton", &[("str", 1), ("con", 1), ("cha", 1)]),
        ("high elf", &[("dex", 2)]),
        ("wood elf", &[("dex", 2)]),
        ("firbolg", &[("wis", 2), ("str", 1)]),
        ("goblin", &[("dex", 2), ("con", 1)]),
        ("kenku", &[("dex", 2), ("wis", 1)]),
        ("kobold", &[("dex", 2), ("str", -2)]),
        ("human", &[("str", 1), ("dex", 1), ("con", 1), ("int", 1), ("wis", 1), ("cha", 1)]),
        ("drow", &[("dex", 2)]),
        ("eladrin", &[("dex", 2)]),
        ("tiefling", &[("cha", 2), ("int", 1)]),
        ("orc", &[("str", 2), ("con", 1), ("int", -2)]),
        ("hill dwarf", &[("con", 2)]),
    ];
    let mut base_matched = false;
    for &(key, bns) in base_races {
        // LOW-4: "half-elf" contains "elf" — the exact row wins instead of
        // inheriting the base race's bonuses (half-elf: CHA+2, not DEX+2).
        let hits = if race_trim.starts_with("half-") {
            race_trim == key
        } else {
            race_trim == key || (race_trim.contains(key) && !base_matched)
        };
        if hits {
            for &(ab, b) in bns {
                bonuses.insert(ab.into(), b);
            }
            base_matched = true;
        }
    }

    // Subrace bonuses
    // MED-1: composites like "Dwarf (Mountain)" / "Elf (High)" are stored
    // subrace-first — the contiguous substring checks below never matched.
    // Match on BOTH words regardless of order.
    if contains_both(&race, "hill", "dwarf") {
        bonuses.insert("wis".into(), 1);
    } else if contains_both(&race, "mountain", "dwarf") {
        bonuses.insert("str".into(), 2);
    } else if contains_both(&race, "high", "elf") {
        bonuses.insert("int".into(), 1);
    } else if contains_both(&race, "wood", "elf") {
        bonuses.insert("wis".into(), 1);
    } else if race.contains("drow") {
        bonuses.insert("cha".into(), 1);
    } else if race.contains("eladrin") {
        bonuses.insert("int".into(), 1);
    } else if contains_both(&race, "forest", "gnome") {
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
