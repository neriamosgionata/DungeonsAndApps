// D&D 5e Combat Resolution Engine
// Pure functions for attack / damage / save / derived-stat computation.
// DB interaction wrappers live in routes/combat.rs.

use crate::dice::{RollResult, roll};
use rand::{SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};

// =====================================================================
// Structured NPC Stat Block
// =====================================================================

/// A structured 5e monster/NPC stat block stored as JSONB in `npcs.stats`.
/// When serialized, fields match the character-sheet keys so `load_snapshot`
/// can feed them directly into `CombatantSnapshot`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcStats {
    #[serde(default)]
    pub abilities: NpcAbilities,
    pub ac: Option<i32>,
    #[serde(default)]
    pub hp: NpcHp,
    #[serde(default)]
    pub speed: i32,
    #[serde(default)]
    pub saves: HashMap<String, bool>,
    #[serde(default)]
    pub skills: HashMap<String, String>,
    #[serde(default)]
    pub weapons: Vec<NpcWeapon>,
    #[serde(default)]
    pub casting: NpcCasting,
    #[serde(default)]
    pub equipment: Vec<NpcEquipment>,

    // NPC-specific display fields
    pub cr: Option<String>,
    pub xp: Option<i32>,
    #[serde(default)]
    pub pb: i32,

    #[serde(default)]
    pub resistances: Vec<String>,
    #[serde(default)]
    pub vulnerabilities: Vec<String>,
    #[serde(default)]
    pub immunities: Vec<String>,
    #[serde(default, rename = "condition_immunities")]
    pub condition_immunities: Vec<String>,
    #[serde(default)]
    pub senses: NpcSenses,
    #[serde(default)]
    pub languages: Vec<String>,

    #[serde(default)]
    pub actions: Vec<NpcAction>,
    #[serde(default)]
    pub legendary_actions: Vec<NpcAction>,
    #[serde(default)]
    pub reactions: Vec<NpcAction>,
    #[serde(default)]
    pub traits: Vec<NpcAction>,

    // Legacy free-form fields (backward compat)
    pub attitude: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcAbilities {
    #[serde(default)]
    pub str: i32,
    #[serde(default)]
    pub dex: i32,
    #[serde(default)]
    pub con: i32,
    #[serde(default)]
    pub int: i32,
    #[serde(default)]
    pub wis: i32,
    #[serde(default)]
    pub cha: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcHp {
    pub max: Option<i32>,
    pub current: Option<i32>,
    pub hit_dice: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcWeapon {
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub properties: String,
    #[serde(default)]
    pub damage: String,
    #[serde(default)]
    pub damage_type: String,
    #[serde(default)]
    pub attack_bonus: i32,
    pub range: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcCasting {
    pub ability: Option<String>,
    pub spell_attack_bonus: Option<i32>,
    pub spell_save_dc: Option<i32>,
    pub slots: Option<Value>,
    pub spells: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcEquipment {
    pub name: String,
    pub quantity: Option<i32>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcSenses {
    pub darkvision: Option<i32>,
    pub blindsight: Option<i32>,
    pub truesight: Option<i32>,
    pub tremorsense: Option<i32>,
    pub passive_perception: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcAction {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub attack_bonus: Option<i32>,
    pub damage: Option<String>,
    pub damage_type: Option<String>,
    pub range: Option<String>,
    pub recharge: Option<String>,
    pub limited_use: Option<NpcLimitedUse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct NpcLimitedUse {
    pub count: i32,
    pub per: String,
}

impl NpcStats {
    /// Parse from raw JSONB value. Returns `None` if the value is null/empty.
    pub fn from_value(v: &Value) -> Option<Self> {
        if v.is_null() || (v.is_object() && v.as_object().map(|o| o.is_empty()).unwrap_or(true)) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

    /// Convert abilities to the JSON shape `ability_mod()` expects.
    pub fn abilities_value(&self) -> Value {
        serde_json::json!({
            "str": self.abilities.str,
            "dex": self.abilities.dex,
            "con": self.abilities.con,
            "int": self.abilities.int,
            "wis": self.abilities.wis,
            "cha": self.abilities.cha,
        })
    }

    /// Convert saves to the JSON shape `save_proficient()` expects.
    pub fn saves_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.saves {
            map.insert(k.clone(), Value::Bool(*v));
        }
        Value::Object(map)
    }

    /// Convert skills to the JSON shape `compute_stats()` expects.
    pub fn skills_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.skills {
            map.insert(k.clone(), Value::String(v.clone()));
        }
        Value::Object(map)
    }

    pub fn weapons_value(&self) -> Value {
        serde_json::to_value(&self.weapons).unwrap_or_else(|_| Value::Array(vec![]))
    }

    pub fn casting_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        if let Some(ref a) = self.casting.ability { map.insert("ability".into(), Value::String(a.clone())); }
        if let Some(v) = self.casting.spell_attack_bonus { map.insert("spell_attack_bonus".into(), Value::Number(v.into())); }
        if let Some(v) = self.casting.spell_save_dc { map.insert("spell_save_dc".into(), Value::Number(v.into())); }
        if let Some(ref s) = self.casting.slots { map.insert("slots".into(), s.clone()); }
        if let Some(ref s) = self.casting.spells { map.insert("spells".into(), s.clone()); }
        Value::Object(map)
    }

    pub fn equipment_value(&self) -> Value {
        serde_json::to_value(&self.equipment).unwrap_or_else(|_| Value::Array(vec![]))
    }
}

// =====================================================================
// Data Structures
// =====================================================================

#[derive(Debug, Clone, Default, Serialize)]
pub struct ComputedStats {
    pub ac: i32,
    pub speed: i32,
    pub initiative_bonus: i32,
    pub attack_bonus: i32,
    pub spell_attack_bonus: i32,
    pub spell_save_dc: i32,
    /// (ability, total modifier)
    pub save_mods: Vec<(String, i32)>,
    pub skill_mods: Vec<(String, i32)>,
    pub passive_scores: Vec<(String, i32)>,
    pub exhaustion: i32,
    pub resistances: HashSet<String>,
    pub vulnerabilities: HashSet<String>,
    pub immunities: HashSet<String>,
    pub attack_advantage: bool,
    pub attack_disadvantage: bool,
    pub save_advantage: bool,
    pub save_disadvantage: bool,
    pub speed_halved: bool,
    pub speed_doubled: bool,
    pub incapacitated: bool,
    pub invisible: bool,
    pub frightened: bool,
    pub paralyzed: bool,
    pub restrained: bool,
    pub prone: bool,
    pub blinded: bool,
    pub deafened: bool,
    pub charmed: bool,
    pub poisoned: bool,
    pub stunned: bool,
    pub unconscious: bool,
    pub petrified: bool,
    pub grappling: bool,
    pub grappled: bool,
    pub concentration: bool,
    pub evasion: bool,
    pub hover: bool,
    pub flying_speed: i32,
    pub swim_speed: i32,
    pub climb_speed: i32,
    pub burrow_speed: i32,
    pub damage_bonus: i32,
    pub weapon_damage_bonus: i32,
    pub nonmagical_damage_reduction: i32,
    pub gnome_cunning: bool,
    pub savage_attacks: bool,
    pub hp_regen_per_turn: i32,
    pub temp_hp_per_turn: i32,
    pub darkvision_range: i32,
    pub truesight_range: i32,
    pub blindsight_range: i32,
    pub tremorsense_range: i32,
    pub archery_style: bool,
    pub dueling_style: bool,
    pub gwf_style: bool,
    pub twf_style: bool,
    pub prone_ranged_disadvantage: bool,
    pub jack_of_all_trades: bool,
    /// Target has effect that grants attackers advantage (Help, Reckless Attack)
    pub attack_advantage_against: bool,
    /// Target has effect that grants attackers disadvantage (Dodge)
    pub attack_disadvantage_against: bool,
}

/// Snapshot of everything needed to resolve combat for one combatant.
#[derive(Debug, Clone)]
pub struct CombatantSnapshot {
    pub id: uuid::Uuid,
    pub encounter_id: uuid::Uuid,
    pub display_name: String,
    pub character_id: Option<uuid::Uuid>,
    pub npc_id: Option<uuid::Uuid>,
    pub hp_current: i32,
    pub hp_max: i32,
    pub temp_hp: i32,
    /// Base AC from sheet (frontend-computed or auto-derived)
    pub base_ac: i32,
    /// Base speed from sheet
    pub base_speed: i32,
    pub level_total: i32,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    /// Raw ability scores from sheet
    pub abilities: Value,
    /// Save proficiencies { "str": true, ... }
    pub saves: Value,
    /// Skill proficiencies { "perception": "prof", ... }
    pub skills: Value,
    /// Proficiency bonus override; 0 means compute from level
    pub proficiency_bonus: i32,
    pub conditions: Vec<String>,
    pub active_effects: Vec<EffectSnapshot>,
    /// Spellcasting ability mod, attack, dc from sheet
    pub casting: Value,
    /// Weapon list from sheet
    pub weapons: Value,
    /// Equipment list from sheet
    pub equipment: Value,
    /// Race name (for racial trait bonuses)
    pub race: Option<String>,
    /// Classes array from sheet (for per-class casting ability, hit dice, etc.)
    pub classes: Value,
    /// Raw sheet JSON (for armor, shield, and other fields not yet extracted)
    pub sheet_raw: Value,
}

#[derive(Debug, Clone)]
pub struct EffectSnapshot {
    pub id: uuid::Uuid,
    pub name: String,
    pub modifiers: Value,
    pub concentration: bool,
    pub source_type: String,
}

// =====================================================================
// Derived Stat Computation
// =====================================================================

pub fn compute_stats(snap: &CombatantSnapshot) -> ComputedStats {
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
            "restrained" => { stats.restrained = true; stats.attack_disadvantage = true; stats.save_advantage_for("dex"); stats.speed = 0; }
            "frightened" => { stats.frightened = true; stats.attack_disadvantage = true; }
            "charmed" => { stats.charmed = true; }
            "poisoned" => { stats.poisoned = true; stats.attack_disadvantage = true; stats.save_disadvantage_for("con"); }
            "stunned" => { stats.stunned = true; stats.incapacitated = true; stats.speed = 0; }
            "unconscious" => { stats.unconscious = true; stats.incapacitated = true; stats.prone = true; stats.speed = 0; }
            "petrified" => {
                stats.petrified = true; stats.incapacitated = true; stats.speed = 0;
                stats.save_disadvantage = true; // auto-fail STR and DEX saves (handled in resolve_save)
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
            if val.as_bool() == Some(true) { stats.restrained = true; stats.attack_disadvantage = true; stats.speed = 0; }
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
    fn save_advantage_for(&mut self, _ability: &str) {
        // Currently global; could be ability-specific later
        self.save_advantage = true;
    }
    fn save_disadvantage_for(&mut self, _ability: &str) {
        self.save_disadvantage = true;
    }
    fn ignore_speed_halved(&self, snap: &CombatantSnapshot) -> bool {
        snap.active_effects.iter().any(|e| {
            e.modifiers.as_object()
                .map(|m| m.get("ignore_speed_reduction").is_some())
                .unwrap_or(false)
        })
    }
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
    } else if props.ranged || props.thrown {
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

// =====================================================================
// Combat Resolution
// =====================================================================

#[derive(Debug, Deserialize)]
pub struct AttackReq {
    pub target_id: uuid::Uuid,
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

fn parse_weapon_props(props_str: &str) -> WeaponProps {
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

pub fn find_weapon<'a>(snapshot: &'a CombatantSnapshot, weapon_id: &str) -> Option<(&'a serde_json::Value, WeaponProps)> {
    if let Some(arr) = snapshot.weapons.as_array() {
        for w in arr {
            if w.get("id").and_then(|v| v.as_str()) == Some(weapon_id)
                || w.get("name").and_then(|v| v.as_str()) == Some(weapon_id)
            {
                let props = w.get("properties")
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

// =====================================================================
// Pure Resolution Functions
// =====================================================================

pub fn resolve_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    req: &AttackReq,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    // Determine cover bonus
    let cover_bonus = match req.cover.as_deref() {
        Some("half") => 2,
        Some("three_quarters") => 5,
        _ => 0,
    };

    // Determine advantage/disadvantage
    let mut adv = req.advantage || attacker_stats.attack_advantage;
    let mut dis = req.disadvantage || attacker_stats.attack_disadvantage;

    // Resolve weapon properties early so prone/ranged checks can use them
    let weapon = req.weapon_id.as_deref().and_then(|wid| find_weapon(attacker, wid));
    let weapon_props = weapon.as_ref().map(|(_, p)| p.clone()).unwrap_or_default();
    let is_ranged_attack = weapon_props.ranged || weapon_props.thrown || req.is_spell_attack;

    // Target conditions affect attacker
    if target_stats.prone {
        if is_ranged_attack {
            dis = true; // ranged attacks vs prone target = disadvantage
        } else {
            adv = true; // melee attacks vs prone target = advantage
        }
    }
    if target_stats.invisible {
        dis = true;
    }
    if target_stats.paralyzed || target_stats.unconscious || target_stats.restrained {
        adv = true;
    }
    // Target's effects that affect attacker's rolls (Dodge, Help, Reckless)
    if target_stats.attack_disadvantage_against {
        dis = true;
    }
    if target_stats.attack_advantage_against {
        adv = true;
    }

    // Invisible attacker has advantage
    if attacker_stats.invisible {
        adv = true;
    }
    // Frightened attacker has disadvantage if source is visible
    if attacker_stats.frightened {
        dis = true;
    }
    // Poisoned attacker has disadvantage
    if attacker_stats.poisoned {
        dis = true;
    }
    // Blinded attacker has disadvantage
    if attacker_stats.blinded {
        dis = true;
    }

    // Charmed attacker: disadvantage on attacks (can't attack charmer)
    // Full enforcement requires knowing who charmed — simplified: all attacks have disadvantage
    if attacker_stats.charmed {
        dis = true;
    }

    // Prone ranged disadvantage: being prone + using ranged/thrown weapon = disadvantage
    if attacker_stats.prone_ranged_disadvantage && is_ranged_attack {
        dis = true;
    }

    // Final cancel out
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    // Archery fighting style: +2 to ranged attack rolls
    let archery_bonus = if attacker_stats.archery_style && (weapon_props.ranged || weapon_props.thrown) { 2 } else { 0 };
    // power_attack (Sharpshooter / Great Weapon Master): -5 attack roll
    let power_attack_penalty = if req.power_attack { -5 } else { 0 };

    // Roll attack
    let attack_expr = if let Some(ref expr) = req.attack_expression {
        expr.clone()
    } else {
        // Auto-compute: 1d20 + pb + ability_mod + attack_bonus from effects
        let pb = if attacker.proficiency_bonus > 0 {
            attacker.proficiency_bonus
        } else {
            proficiency_from_level(attacker.level_total)
        };
        let ability = req.ability.as_deref().unwrap_or_else(|| {
            if weapon_props.ranged || weapon_props.thrown { "dex" }
            else { "str" }
        });
        let ability_mod = if weapon_props.finesse {
            ability_mod(attacker, "str").max(ability_mod(attacker, "dex"))
        } else {
            ability_mod(attacker, ability)
        };
        let prof = if req.proficient.unwrap_or(true) { pb } else { 0 };
        let bonus = attacker_stats.attack_bonus + archery_bonus + power_attack_penalty;

        // Bless: +1d4 (or +Nd4 if multiple bless sources)
        let bless_str = if let Some(n) = req.bless_dice.filter(|&n| n > 0) {
            if n == 1 { "+1d4".to_string() }
            else { format!("+{}d4", n) }
        } else { String::new() };

        // Bardic Inspiration: +1dX
        let bardic_str = if let Some(die) = req.bardic_inspiration_dice {
            format!("+1d{}", die)
        } else { String::new() };

        if effective_adv {
            format!("2d20kh1+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        } else if effective_dis {
            format!("2d20kl1+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        } else {
            format!("1d20+{}+{}+{}{}{}", ability_mod, prof, bonus, bless_str, bardic_str)
        }
    };

    let attack_roll = roll(&attack_expr, &mut rng)
        .map_err(|e| format!("attack roll error: {}", e))?;

    // Determine natural roll (kept die for adv/dis, first roll for straight rolls)
    let natural_roll = attack_roll.terms.first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let crit_range = attacker.sheet_raw.get("crit_range")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(20);
    let critical = natural_roll >= crit_range;
    let auto_miss = natural_roll == 1;

    let target_ac = target_stats.ac + cover_bonus;
    let hit = if critical { true } else if auto_miss { false } else { attack_roll.total >= target_ac };

    let mut result = AttackResult {
        hit,
        critical,
        natural_roll,
        attack_total: attack_roll.total,
        target_ac,
        attack_roll,
        damage_roll: None,
        damage_base: 0,
        damage_applied: 0,
        extra_damage_applied: 0,
        extra_damage_type: None,
        target_hp_before: target.hp_current,
        target_hp_after: target.hp_current,
        target_temp_hp_after: target.temp_hp,
        concentration_broken: false,
        concentration_roll: None,
        combat_event_id: None,
        cover_bonus,
        attack_advantage: effective_adv,
        attack_disadvantage: effective_dis,
        damage_resisted: false,
        damage_vulnerable: false,
        damage_immune: false,
        reach_weapon: weapon_props.reach,
        needs_ammo: weapon_props.ammunition,
        instant_death: false,
    };

    if hit {
        let dmg_expr = if let Some(ref expr) = req.damage_expression {
            expr.clone()
        } else if let Some((weapon, _)) = req.weapon_id.as_deref().and_then(|wid| find_weapon(attacker, wid)) {
            compute_weapon_damage_expression(weapon, attacker, false)
        } else {
            // Default: unarmed strike = 1 + str mod
            let str_mod = ability_mod(attacker, "str").max(1);
            format!("1+{}", str_mod)
        };

        let mut dmg_roll = roll(&dmg_expr, &mut rng)
            .map_err(|e| format!("damage roll error: {}", e))?;

        // GWF: reroll weapon damage once if any die landed 1 or 2
        // Only applies to melee weapons; take the better of two rolls
        if attacker_stats.gwf_style && !weapon_props.ranged && !weapon_props.thrown {
            let has_low = dmg_roll.terms.iter()
                .flat_map(|t| t.rolls.iter())
                .any(|&r| r <= 2);
            if has_low {
                if let Ok(rerolled) = roll(&dmg_expr, &mut rng) {
                    if rerolled.total > dmg_roll.total {
                        dmg_roll = rerolled;
                    }
                }
            }
        }

        // Critical = double dice
        if critical {
            let crit_expr = crit_double_dice(&dmg_expr);
            dmg_roll = roll(&crit_expr, &mut rng)
                .map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        // Savage Attacks (Half-orc): extra weapon die on crit
        let savage_bonus = if critical && attacker_stats.savage_attacks {
            let die = req.damage_die.as_deref().unwrap_or("d6");
            roll(&format!("1{}", die), &mut rng).map(|r| r.total).unwrap_or(0)
        } else { 0 };

        // Dueling style: +2 damage when wielding a one-handed weapon and no off-hand weapon
        // (simplified: +2 if not two-handed and not ranged)
        let dueling_bonus = if attacker_stats.dueling_style
            && !weapon_props.two_handed
            && !weapon_props.ranged
            && !weapon_props.thrown
        { 2 } else { 0 };

        // Power attack (Sharpshooter / GWM): +10 damage on hit
        let power_attack_damage = if req.power_attack { 10 } else { 0 };

        let raw_dmg = dmg_roll.total + attacker_stats.damage_bonus + attacker_stats.weapon_damage_bonus + savage_bonus + dueling_bonus + power_attack_damage;
        let dtype = req.damage_type.to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) = apply_damage_type(raw_dmg, &dtype, target_stats, req.is_magical);

        // Extra damage (Sneak Attack, Divine Smite, Rage, etc.)
        // PHB p.196: all attack damage dice are doubled on a critical hit.
        let (extra_applied, extra_dtype) = if let Some(ref extra_expr) = req.extra_damage_expression {
            let expr = if critical { crit_double_dice(extra_expr) } else { extra_expr.clone() };
            let extra_roll = roll(&expr, &mut rng).map_err(|e| format!("extra damage roll error: {}", e))?;
            let extra_raw = extra_roll.total;
            let extra_type = req.extra_damage_type.as_deref().unwrap_or("piercing");
            let (extra_eff, _, _, _) = apply_damage_type(extra_raw, extra_type, target_stats, req.is_magical);
            (extra_eff, Some(extra_type.to_string()))
        } else {
            (0, None)
        };

        result.damage_roll = Some(dmg_roll);
        result.damage_base = raw_dmg;
        result.damage_applied = effective_dmg;
        result.extra_damage_applied = extra_applied;
        result.extra_damage_type = extra_dtype;
        result.damage_resisted = resisted;
        result.damage_vulnerable = vulnerable;
        result.damage_immune = immune;

        let total_damage = effective_dmg + extra_applied;

        // PHB p.197: massive damage = single hit ≥ hp_max → instant death
        result.instant_death = is_massive_damage(target.hp_max, total_damage);

        // Apply HP damage
        let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, total_damage);
        result.target_hp_after = new_hp;
        result.target_temp_hp_after = new_temp;

        // Concentration check if target has concentration
        if target.active_effects.iter().any(|e| e.concentration) {
            let (broken, roll_res) = concentration_check(target, total_damage, &mut rng);
            result.concentration_broken = broken;
            result.concentration_roll = Some(roll_res);
        }
    }

    Ok(result)
}

/// Resolve a two-weapon fighting bonus-action off-hand attack.
/// Off-hand weapon must have the "light" property.
/// Damage does NOT include ability modifier unless the attacker has the TWF fighting style.
pub fn resolve_two_weapon_attack(
    attacker: &CombatantSnapshot,
    target: &CombatantSnapshot,
    offhand_weapon_id: &str,
    attacker_stats: &ComputedStats,
    target_stats: &ComputedStats,
    twf_fighting_style: bool,
) -> Result<AttackResult, String> {
    let mut rng = StdRng::from_os_rng();

    let weapon = find_weapon(attacker, offhand_weapon_id)
        .ok_or_else(|| "off-hand weapon not found".to_string())?;
    let weapon_props = weapon.1;

    if !weapon_props.light {
        return Err("off-hand weapon must have the light property".to_string());
    }

    let pb = if attacker.proficiency_bonus > 0 {
        attacker.proficiency_bonus
    } else {
        proficiency_from_level(attacker.level_total)
    };

    let ability = if weapon_props.finesse {
        if ability_mod(attacker, "dex") > ability_mod(attacker, "str") { "dex" } else { "str" }
    } else if weapon_props.ranged || weapon_props.thrown {
        "dex"
    } else {
        "str"
    };
    let ability_mod_val = ability_mod(attacker, ability);

    let attack_expr = format!("1d20+{}+{}", ability_mod_val, pb);
    let attack_roll = roll(&attack_expr, &mut rng)
        .map_err(|e| format!("attack roll error: {}", e))?;

    let natural_roll = attack_roll.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    let crit_range = attacker.sheet_raw.get("crit_range")
        .and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(20);
    let critical = natural_roll >= crit_range;
    let auto_miss = natural_roll == 1;
    let target_ac = target_stats.ac;
    let hit = if critical { true } else if auto_miss { false } else { attack_roll.total >= target_ac };

    let mut result = AttackResult {
        hit,
        critical,
        natural_roll,
        attack_total: attack_roll.total,
        target_ac,
        attack_roll,
        damage_roll: None,
        damage_base: 0,
        damage_applied: 0,
        extra_damage_applied: 0,
        extra_damage_type: None,
        target_hp_before: target.hp_current,
        target_hp_after: target.hp_current,
        target_temp_hp_after: target.temp_hp,
        concentration_broken: false,
        concentration_roll: None,
        combat_event_id: None,
        cover_bonus: 0,
        attack_advantage: false,
        attack_disadvantage: false,
        damage_resisted: false,
        damage_vulnerable: false,
        damage_immune: false,
        reach_weapon: weapon_props.reach,
        needs_ammo: weapon_props.ammunition,
        instant_death: false,
    };

    if hit {
        let dmg_expr = compute_weapon_damage_expression(weapon.0, attacker, false);
        // For TWF: remove ability mod from damage unless fighting style
        let dmg_expr_no_mod = if twf_fighting_style {
            dmg_expr
        } else {
            // Strip the +ability_mod suffix if present
            let base_die = weapon.0.get("damage_die").and_then(|v| v.as_str())
                .or_else(|| weapon.0.get("damage").and_then(|v| v.as_str()))
                .unwrap_or("1d4");
            base_die.to_string()
        };

        let mut dmg_roll = roll(&dmg_expr_no_mod, &mut rng)
            .map_err(|e| format!("damage roll error: {}", e))?;

        if critical {
            let crit_expr = crit_double_dice(&dmg_expr_no_mod);
            dmg_roll = roll(&crit_expr, &mut rng)
                .map_err(|e| format!("crit damage roll error: {}", e))?;
        }

        let raw_dmg = dmg_roll.total + attacker_stats.damage_bonus + attacker_stats.weapon_damage_bonus;
        let dtype = weapon.0.get("damage_type").and_then(|v| v.as_str()).unwrap_or("slashing").to_lowercase();

        let (effective_dmg, resisted, vulnerable, immune) = apply_damage_type(raw_dmg, &dtype, target_stats, false);

        result.damage_roll = Some(dmg_roll);
        result.damage_base = raw_dmg;
        result.damage_applied = effective_dmg;
        result.damage_resisted = resisted;
        result.damage_vulnerable = vulnerable;
        result.damage_immune = immune;

        let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, effective_dmg);
        result.target_hp_after = new_hp;
        result.target_temp_hp_after = new_temp;

        if target.active_effects.iter().any(|e| e.concentration) {
            let (broken, roll_res) = concentration_check(target, effective_dmg, &mut rng);
            result.concentration_broken = broken;
            result.concentration_roll = Some(roll_res);
        }
    }

    Ok(result)
}

pub fn resolve_damage(
    target: &CombatantSnapshot,
    req: &DamageReq,
    target_stats: &ComputedStats,
) -> Result<DamageResult, String> {
    let mut rng = StdRng::from_os_rng();
    let dtype = req.damage_type.to_lowercase();
    let (effective_dmg, damage_resisted, damage_vulnerable, damage_immune) = apply_damage_type(req.amount, &dtype, target_stats, req.is_magical);

    let (new_hp, new_temp) = apply_hp_damage(target.hp_current, target.temp_hp, effective_dmg);

    let mut concentration_broken = false;
    let mut concentration_roll = None;
    if target.active_effects.iter().any(|e| e.concentration) {
        let (broken, roll_res) = concentration_check(target, effective_dmg, &mut rng);
        concentration_broken = broken;
        concentration_roll = Some(roll_res);
    }

    Ok(DamageResult {
        damage_raw: req.amount,
        damage_applied: effective_dmg,
        hp_before: target.hp_current,
        hp_after: new_hp,
        temp_hp_after: new_temp,
        concentration_broken,
        concentration_roll,
        combat_event_id: None,
        damage_resisted,
        damage_vulnerable,
        damage_immune,
        instant_death: is_massive_damage(target.hp_max, effective_dmg),
    })
}

pub fn resolve_save(
    snap: &CombatantSnapshot,
    req: &SaveReq,
    stats: &ComputedStats,
) -> Result<SaveResult, String> {
    let mut rng = StdRng::from_os_rng();
    let ability = req.ability.to_lowercase();

    let mut adv = req.advantage || stats.save_advantage;
    let mut dis = req.disadvantage || stats.save_disadvantage;

    // Poisoned = CON save disadvantage
    if stats.poisoned && ability == "con" {
        dis = true;
    }
    // Gnome Cunning: advantage on INT/WIS/CHA saves vs magic
    if stats.gnome_cunning && req.is_magical.unwrap_or(false)
        && matches!(ability.as_str(), "int" | "wis" | "cha")
    {
        adv = true;
    }
    // Paralyzed/Petrified = auto-fail STR and DEX saves
    if (stats.paralyzed || stats.petrified) && (ability == "str" || ability == "dex") {
        return Ok(SaveResult {
            passed: false,
            natural_roll: 1,
            save_total: 1,
            dc: req.dc,
            save_roll: roll("1d20", &mut rng).unwrap(),
            save_advantage: false,
            save_disadvantage: true,
        });
    }

    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let save_mod = stats.save_mods.iter()
        .find(|(a, _)| a == &ability)
        .map(|(_, m)| *m)
        .unwrap_or(ability_mod(snap, &ability));

    let expr = if effective_adv {
        format!("2d20kh1+{}", save_mod)
    } else if effective_dis {
        format!("2d20kl1+{}", save_mod)
    } else {
        format!("1d20+{}", save_mod)
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("save roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.kept.first().copied().or_else(|| t.rolls.first().copied()))
        .unwrap_or(0);

    let passed = roll_res.total >= req.dc;

    Ok(SaveResult {
        passed,
        natural_roll: natural,
        save_total: roll_res.total,
        dc: req.dc,
        save_roll: roll_res,
        save_advantage: effective_adv,
        save_disadvantage: effective_dis,
    })
}

pub fn resolve_heal(target: &CombatantSnapshot, req: &HealReq) -> HealResult {
    let hp_before = target.hp_current;
    let hp_after = (target.hp_current + req.amount).min(target.hp_max);
    let stabilized = hp_before <= 0 && hp_after > 0;
    let revived = stabilized; // same concept here
    HealResult {
        amount: req.amount,
        hp_before,
        hp_after,
        temp_hp_after: target.temp_hp,
        stabilized,
        revived,
    }
}

pub fn resolve_death_save(
    snap: &CombatantSnapshot,
    req: &DeathSaveReq,
) -> Result<DeathSaveResult, String> {
    let mut rng = StdRng::from_os_rng();

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        "2d20kh1".to_string()
    } else if effective_dis {
        "2d20kl1".to_string()
    } else {
        "1d20".to_string()
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("death save roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    let nat20 = natural == 20;
    let nat1 = natural == 1;

    // Read current death saves from sheet
    let ds = snap.sheet_raw.get("death_saves");
    let successes_before = ds.and_then(|d| d.get("successes"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(0, 3) as i32;
    let failures_before = ds.and_then(|d| d.get("failures"))
        .and_then(|v| v.as_i64()).unwrap_or(0).clamp(0, 3) as i32;

    let mut successes_after = successes_before;
    let mut failures_after = failures_before;
    let mut hp_after = snap.hp_current;
    let mut alive = true;
    let mut stabilized = false;
    let mut died = false;

    if nat20 {
        // Regain 1 HP, stable
        hp_after = 1;
        alive = true;
        stabilized = true;
        successes_after = 0;
        failures_after = 0;
    } else if nat1 {
        failures_after = (failures_before + 2).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    } else if natural >= 10 {
        successes_after = (successes_before + 1).min(3);
        if successes_after >= 3 {
            stabilized = true;
            alive = true;
            successes_after = 0;
            failures_after = 0;
        }
    } else {
        failures_after = (failures_before + 1).min(3);
        if failures_after >= 3 {
            died = true;
            alive = false;
        }
    }

    Ok(DeathSaveResult {
        natural_roll: natural,
        passed: natural >= 10 && !nat1,
        successes_before,
        failures_before,
        successes_after,
        failures_after,
        stabilized,
        died,
        nat20,
        nat1,
        hp_after,
        alive,
    })
}

pub fn resolve_skill_check(
    snap: &CombatantSnapshot,
    req: &SkillCheckReq,
    stats: &ComputedStats,
) -> Result<SkillCheckResult, String> {
    let mut rng = StdRng::from_os_rng();
    let skill = req.skill.to_lowercase().replace(' ', "_");

    let pb = if snap.proficiency_bonus > 0 {
        snap.proficiency_bonus
    } else {
        proficiency_from_level(snap.level_total)
    };

    let skill_prof_for_jack = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient_for_jack = matches!(skill_prof_for_jack, Some("prof") | Some("proficient") | Some("expert"));

    let modv = stats.skill_mods.iter()
        .find(|(s, _)| s == &skill)
        .map(|(_, m)| if !is_proficient_for_jack && stats.jack_of_all_trades {
            *m + (pb / 2)
        } else {
            *m
        })
        .unwrap_or_else(|| {
            // fallback: try ability mod based on skill name
            let ability = skill_ability(&skill);
            let base = ability_mod(snap, ability);
            if stats.jack_of_all_trades { base + pb / 2 } else { base }
        });

    let adv = req.advantage;
    let dis = req.disadvantage;
    let effective_adv = adv && !dis;
    let effective_dis = dis && !adv;

    let expr = if effective_adv {
        format!("2d20kh1+{}", modv)
    } else if effective_dis {
        format!("2d20kl1+{}", modv)
    } else {
        format!("1d20+{}", modv)
    };

    let roll_res = roll(&expr, &mut rng)
        .map_err(|e| format!("skill check roll error: {}", e))?;

    let natural = roll_res.terms.first()
        .and_then(|t| t.rolls.first().copied())
        .unwrap_or(0);

    // Reliable Talent (Rogue 11+): treat any d20 ≤9 as 10 for proficient/expert skills
    let has_reliable_talent = snap.classes.as_array().map(|arr| {
        arr.iter().any(|c| {
            let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
            let level = c.get("level").and_then(|v| v.as_i64()).map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32).unwrap_or(0);
            name == "rogue" && level >= 11
        })
    }).unwrap_or(false);
    let skill_prof = snap.skills.get(&skill)
        .or_else(|| snap.skills.get(&skill.replace('_', " ")))
        .and_then(|v| v.as_str());
    let is_proficient = matches!(skill_prof, Some("prof") | Some("proficient") | Some("expert"));
    let total = if has_reliable_talent && is_proficient && natural < 10 {
        roll_res.total - natural + 10
    } else {
        roll_res.total
    };

    let passed = req.dc.map(|dc| total >= dc);

    Ok(SkillCheckResult {
        skill: req.skill.clone(),
        natural_roll: natural,
        total,
        dc: req.dc,
        passed,
        advantage: effective_adv,
        disadvantage: effective_dis,
    })
}

fn skill_ability(skill: &str) -> &str {
    match skill {
        "athletics" => "str",
        "acrobatics" | "sleight_of_hand" | "stealth" => "dex",
        "arcana" | "history" | "investigation" | "nature" | "religion" => "int",
        "animal_handling" | "insight" | "medicine" | "perception" | "survival" => "wis",
        "deception" | "intimidation" | "performance" | "persuasion" => "cha",
        _ => "wis",
    }
}

// =====================================================================
// Helpers
// =====================================================================

pub fn apply_damage_type(raw: i32, dtype: &str, stats: &ComputedStats, is_magical: bool) -> (i32, bool, bool, bool) {
    if stats.immunities.contains(dtype) || stats.immunities.contains("all") {
        return (0, false, false, true);
    }
    if stats.immunities.contains("nonmagical") && !is_magical {
        return (0, false, false, true);
    }
    // PHB p.197: resistance and vulnerability cancel each other out.
    let has_resistance = stats.resistances.contains(dtype) || stats.resistances.contains(&"all".to_string());
    let has_vulnerability = stats.vulnerabilities.contains(dtype) || stats.vulnerabilities.contains(&"all".to_string());
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
    if stats.nonmagical_damage_reduction > 0 && !is_magical
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
    if dmg <= 0 { return (hp, temp); }
    let remaining = dmg - temp;
    if remaining <= 0 {
        (hp, temp - dmg)
    } else {
        (hp - remaining, 0)
    }
}

pub fn concentration_check(target: &CombatantSnapshot, damage: i32, rng: &mut StdRng) -> (bool, RollResult) {
    // DC = max(10, floor(damage / 2))
    let dc = (damage / 2).max(10);
    let con_mod = ability_mod(target, "con");
    let has_war_caster = target.sheet_raw.get("feats")
        .and_then(|v| v.as_array())
        .map(|feats| feats.iter().any(|f| f.get("key").and_then(|k| k.as_str()) == Some("war_caster")))
        .unwrap_or(false);
    let expr = if has_war_caster {
        format!("2d20kh1+{}", con_mod)
    } else {
        format!("1d20+{}", con_mod)
    };
    let roll_res = roll(&expr, rng).expect("valid expression");
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
    if result.is_empty() { result = expr.to_string(); }
    result
}

// =====================================================================
// DB Loading Helpers (used by routes)
// =====================================================================

use sqlx::PgPool;

#[derive(sqlx::FromRow)]
struct SnapRow {
    id: uuid::Uuid,
    encounter_id: uuid::Uuid,
    display_name: String,
    character_id: Option<uuid::Uuid>,
    npc_id: Option<uuid::Uuid>,
    hp_current: i32,
    hp_max: i32,
    temp_hp: i32,
    ac: i32,
    level_override: i32,
    token_x: Option<f32>,
    token_y: Option<f32>,
    abilities: serde_json::Value,
    saves: serde_json::Value,
    skills: serde_json::Value,
    casting: serde_json::Value,
    conditions: Vec<String>,
    weapons: serde_json::Value,
    level_total: i32,
    equipment: serde_json::Value,
    npc_stats_raw: Option<serde_json::Value>,
    race: Option<String>,
    classes: serde_json::Value,
    sheet_raw: serde_json::Value,
}

pub async fn load_snapshot(db: &PgPool, combatant_id: uuid::Uuid) -> Result<CombatantSnapshot, crate::error::AppError> {
    let row: SnapRow = sqlx::query_as(
        r#"select
            c.id, c.encounter_id, c.display_name, c.character_id, c.npc_id,
            c.hp_current, c.hp_max, c.temp_hp, c.ac, c.level_override,
            c.token_x, c.token_y,
            coalesce(ch.sheet->'abilities', n.stats->'abilities', '{}'::jsonb) as abilities,
            coalesce(ch.sheet->'saves', n.stats->'saves', '{}'::jsonb) as saves,
            coalesce(ch.sheet->'skills', n.stats->'skills', '{}'::jsonb) as skills,
            coalesce(ch.sheet->'casting', n.stats->'casting', '{}'::jsonb) as casting,
            c.conditions,
            coalesce(ch.sheet->'weapons', n.stats->'weapons', '[]'::jsonb) as weapons,
            coalesce((ch.sheet->>'level_total')::int, (n.stats->>'pb')::int, 1) as level_total,
            coalesce(ch.sheet->'equipment', n.stats->'equipment', '[]'::jsonb) as equipment,
            n.stats as npc_stats_raw,
            ch.race,
            coalesce(ch.sheet->'classes', n.stats->'classes', '[]'::jsonb) as classes,
            coalesce(ch.sheet, n.stats, '{}'::jsonb) as sheet_raw
         from combatants c
         left join characters ch on ch.id = c.character_id
         left join npcs n on n.id = c.npc_id
         where c.id = $1"#,
    )
    .bind(combatant_id)
    .fetch_optional(db)
    .await?
    .ok_or(crate::error::AppError::NotFound)?;

    let effects: Vec<(uuid::Uuid, String, serde_json::Value, bool, String)> = sqlx::query_as(
        r#"select id, name, modifiers, concentration, source_type::text
           from combatant_effects
           where combatant_id = $1 and active = true"#,
    )
    .bind(combatant_id)
    .fetch_all(db)
    .await?;

    // If this is an NPC combatant (no linked character), parse structured stats
    // and use them to populate the snapshot fields.
    let npc_stats = row.npc_stats_raw.as_ref().and_then(NpcStats::from_value);
    let is_npc = row.character_id.is_none() && row.npc_id.is_some();

    let abilities = if is_npc {
        npc_stats.as_ref().map(|n| n.abilities_value()).unwrap_or(row.abilities)
    } else {
        row.abilities
    };
    let saves = if is_npc {
        npc_stats.as_ref().map(|n| n.saves_value()).unwrap_or(row.saves)
    } else {
        row.saves
    };
    let skills = if is_npc {
        npc_stats.as_ref().map(|n| n.skills_value()).unwrap_or(row.skills)
    } else {
        row.skills
    };
    let casting = if is_npc {
        npc_stats.as_ref().map(|n| n.casting_value()).unwrap_or(row.casting)
    } else {
        row.casting
    };
    let weapons = if is_npc {
        npc_stats.as_ref().map(|n| n.weapons_value()).unwrap_or(row.weapons)
    } else {
        row.weapons
    };
    let equipment = if is_npc {
        npc_stats.as_ref().map(|n| n.equipment_value()).unwrap_or(row.equipment)
    } else {
        row.equipment
    };

    let base_speed = if is_npc {
        npc_stats.as_ref().map(|n| n.speed).unwrap_or(30)
    } else {
        row.sheet_raw.get("speed")
            .and_then(|v| v.as_i64())
            .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
            .unwrap_or(30)
    };

    let level_total = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
        if stats.pb > 0 {
            // Use proficiency bonus directly; the combat engine only uses pb for
            // save/skill/spell calculations, so we can set a synthetic level.
            // pb=2 → level 1-4, pb=3 → 5-8, etc.
            let pb = stats.pb;
            ((pb - 2) * 4 + 1).max(1)
        } else {
            row.level_total
        }
    } else {
        row.level_total
    };

    let proficiency_bonus = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
        if stats.pb > 0 {
            stats.pb
        } else {
            row.level_override
        }
    } else {
        row.level_override
    };

    let race = if is_npc { None } else { row.race };
    let classes = if is_npc {
        row.classes
    } else {
        row.classes
    };
    let sheet_raw = if is_npc {
        row.npc_stats_raw.unwrap_or(row.sheet_raw)
    } else {
        row.sheet_raw
    };

    Ok(CombatantSnapshot {
        id: row.id,
        encounter_id: row.encounter_id,
        display_name: row.display_name,
        character_id: row.character_id,
        npc_id: row.npc_id,
        hp_current: row.hp_current,
        hp_max: row.hp_max,
        temp_hp: row.temp_hp,
        base_ac: row.ac,
        base_speed: base_speed.max(0),
        level_total,
        token_x: row.token_x,
        token_y: row.token_y,
        abilities,
        saves,
        skills,
        proficiency_bonus,
        conditions: row.conditions,
        active_effects: effects.into_iter().map(|(id, name, mods, conc, st)| EffectSnapshot {
            id, name, modifiers: mods, concentration: conc, source_type: st,
        }).collect(),
        casting,
        weapons,
        equipment,
        race,
        classes,
        sheet_raw,
    })
}

pub async fn load_snapshots_batch(
    db: &PgPool,
    combatant_ids: &[uuid::Uuid],
) -> Result<std::collections::HashMap<uuid::Uuid, CombatantSnapshot>, crate::error::AppError> {
    if combatant_ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }

    let rows: Vec<SnapRow> = sqlx::query_as(
        r#"select
            c.id, c.encounter_id, c.display_name, c.character_id, c.npc_id,
            c.hp_current, c.hp_max, c.temp_hp, c.ac, c.level_override,
            c.token_x, c.token_y,
            coalesce(ch.sheet->'abilities', n.stats->'abilities', '{}'::jsonb) as abilities,
            coalesce(ch.sheet->'saves', n.stats->'saves', '{}'::jsonb) as saves,
            coalesce(ch.sheet->'skills', n.stats->'skills', '{}'::jsonb) as skills,
            coalesce(ch.sheet->'casting', n.stats->'casting', '{}'::jsonb) as casting,
            c.conditions,
            coalesce(ch.sheet->'weapons', n.stats->'weapons', '[]'::jsonb) as weapons,
            coalesce((ch.sheet->>'level_total')::int, (n.stats->>'pb')::int, 1) as level_total,
            coalesce(ch.sheet->'equipment', n.stats->'equipment', '[]'::jsonb) as equipment,
            n.stats as npc_stats_raw,
            ch.race,
            coalesce(ch.sheet->'classes', n.stats->'classes', '[]'::jsonb) as classes,
            coalesce(ch.sheet, n.stats, '{}'::jsonb) as sheet_raw
         from combatants c
         left join characters ch on ch.id = c.character_id
         left join npcs n on n.id = c.npc_id
         where c.id = ANY($1)"#,
    )
    .bind(combatant_ids)
    .fetch_all(db)
    .await?;

    let effects_rows: Vec<(uuid::Uuid, uuid::Uuid, String, serde_json::Value, bool, String)> = sqlx::query_as(
        r#"select combatant_id, id, name, modifiers, concentration, source_type::text
           from combatant_effects
           where combatant_id = ANY($1) and active = true"#,
    )
    .bind(combatant_ids)
    .fetch_all(db)
    .await?;

    let mut effects_map: std::collections::HashMap<uuid::Uuid, Vec<EffectSnapshot>> = std::collections::HashMap::new();
    for (cid, id, name, mods, conc, st) in effects_rows {
        effects_map.entry(cid).or_default().push(EffectSnapshot {
            id, name, modifiers: mods, concentration: conc, source_type: st,
        });
    }

    let mut results = std::collections::HashMap::new();
    for row in rows {
        let npc_stats = row.npc_stats_raw.as_ref().and_then(NpcStats::from_value);
        let is_npc = row.character_id.is_none() && row.npc_id.is_some();

        let abilities = if is_npc {
            npc_stats.as_ref().map(|n| n.abilities_value()).unwrap_or(row.abilities)
        } else {
            row.abilities
        };
        let saves = if is_npc {
            npc_stats.as_ref().map(|n| n.saves_value()).unwrap_or(row.saves)
        } else {
            row.saves
        };
        let skills = if is_npc {
            npc_stats.as_ref().map(|n| n.skills_value()).unwrap_or(row.skills)
        } else {
            row.skills
        };
        let casting = if is_npc {
            npc_stats.as_ref().map(|n| n.casting_value()).unwrap_or(row.casting)
        } else {
            row.casting
        };
        let weapons = if is_npc {
            npc_stats.as_ref().map(|n| n.weapons_value()).unwrap_or(row.weapons)
        } else {
            row.weapons
        };
        let equipment = if is_npc {
            npc_stats.as_ref().map(|n| n.equipment_value()).unwrap_or(row.equipment)
        } else {
            row.equipment
        };
        let base_speed = if is_npc {
            npc_stats.as_ref().map(|n| n.speed).unwrap_or(30)
        } else {
            row.sheet_raw.get("speed")
                .and_then(|v| v.as_i64())
                .map(|v| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
                .unwrap_or(30)
        };

        let level_total = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
            if stats.pb > 0 {
                let pb = stats.pb;
                ((pb - 2) * 4 + 1).max(1)
            } else {
                row.level_total
            }
        } else {
            row.level_total
        };

        let proficiency_bonus = if let (true, Some(ref stats)) = (is_npc, npc_stats.as_ref()) {
            if stats.pb > 0 {
                stats.pb
            } else {
                row.level_override
            }
        } else {
            row.level_override
        };

        let race = if is_npc { None } else { row.race };
        let classes = row.classes;
        let sheet_raw = if is_npc {
            row.npc_stats_raw.unwrap_or(row.sheet_raw)
        } else {
            row.sheet_raw
        };

        let effects = effects_map.remove(&row.id).unwrap_or_default();
        results.insert(row.id, CombatantSnapshot {
            id: row.id,
            encounter_id: row.encounter_id,
            display_name: row.display_name,
            character_id: row.character_id,
            npc_id: row.npc_id,
            hp_current: row.hp_current,
            hp_max: row.hp_max,
            temp_hp: row.temp_hp,
            base_ac: row.ac,
            base_speed: base_speed.max(0),
            level_total,
            token_x: row.token_x,
            token_y: row.token_y,
            abilities,
            saves,
            skills,
            proficiency_bonus,
            conditions: row.conditions,
            active_effects: effects,
            casting,
            weapons,
            equipment,
            race,
            classes,
            sheet_raw,
        });
    }

    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proficiency_from_level() {
        assert_eq!(proficiency_from_level(1), 2);
        assert_eq!(proficiency_from_level(4), 2);
        assert_eq!(proficiency_from_level(5), 3);
        assert_eq!(proficiency_from_level(8), 3);
        assert_eq!(proficiency_from_level(9), 4);
        assert_eq!(proficiency_from_level(17), 6);
    }

    #[test]
    fn test_crit_double_dice() {
        assert_eq!(crit_double_dice("1d8+3"), "2d8+3");
        assert_eq!(crit_double_dice("2d6+1d4+5"), "4d6+2d4+5");
        assert_eq!(crit_double_dice("1d10"), "2d10");
        assert_eq!(crit_double_dice("5"), "5");
    }

    #[test]
    fn test_apply_damage_type() {
        let mut stats = ComputedStats::default();
        stats.resistances.insert("fire".into());
        stats.vulnerabilities.insert("cold".into());
        stats.immunities.insert("poison".into());

        assert_eq!(apply_damage_type(10, "fire", &stats, false), (5, true, false, false));
        assert_eq!(apply_damage_type(10, "cold", &stats, false), (20, false, true, false));
        assert_eq!(apply_damage_type(10, "poison", &stats, false), (0, false, false, true));
        assert_eq!(apply_damage_type(10, "slashing", &stats, false), (10, false, false, false));
    }

    #[test]
    fn test_apply_hp_damage() {
        assert_eq!(apply_hp_damage(20, 5, 3), (20, 2));
        assert_eq!(apply_hp_damage(20, 5, 7), (18, 0));
        assert_eq!(apply_hp_damage(20, 0, 5), (15, 0));
        assert_eq!(apply_hp_damage(20, 10, 0), (20, 10));
    }
}
