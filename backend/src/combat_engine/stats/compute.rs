use super::super::types::{CombatantSnapshot, ComputedStats};
use super::abilities::ability_mod;
use super::ac::parse_ac_base;
use serde_json::Value;

pub fn compute_stats(snap: &CombatantSnapshot) -> ComputedStats {
    let mut stats = ComputedStats {
        ac: super::ac::compute_ac_from_sheet(snap),
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
    let movement_denied = stats.paralyzed || stats.stunned || stats.restrained || stats.grappled
        || stats.petrified || stats.unconscious || stats.exhaustion >= 5;
    if !movement_denied && stats.flying_speed > stats.speed {
        stats.speed = stats.flying_speed;
    }
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
        super::abilities::proficiency_from_level(snap.level_total)
    };
    stats.initiative_bonus = ability_mod(snap, "dex")
        + snap.sheet_raw.get("initiative").and_then(|v| v.as_i64()).unwrap_or(0).clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    stats.spell_attack_bonus = pb + ability_mod(snap, &super::abilities::casting_ability(snap));
    stats.spell_save_dc = 8 + pb + ability_mod(snap, &super::abilities::casting_ability(snap));

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
            if super::abilities::save_proficient(snap, ab) {
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

pub fn apply_modifier(stats: &mut ComputedStats, key: &str, val: &Value) {
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
