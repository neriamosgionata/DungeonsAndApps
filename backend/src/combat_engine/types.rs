// Data structures for the combat engine: NPC stats, ComputedStats,
// CombatantSnapshot, EffectSnapshot, and ComputedStats impl.
// Pure data — no DB or heavy logic.

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
    pub fn from_value(v: &Value) -> Option<Self> {
        if v.is_null() || (v.is_object() && v.as_object().map(|o| o.is_empty()).unwrap_or(true)) {
            return None;
        }
        serde_json::from_value(v.clone()).ok()
    }

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

    pub fn saves_value(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.saves {
            map.insert(k.clone(), Value::Bool(*v));
        }
        Value::Object(map)
    }

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

impl ComputedStats {
    pub(crate) fn save_disadvantage_for(&mut self, _ability: &str) {
        self.save_disadvantage = true;
    }
    pub(crate) fn ignore_speed_halved(&self, snap: &CombatantSnapshot) -> bool {
        snap.active_effects.iter().any(|e| {
            e.modifiers.as_object()
                .map(|m| m.get("ignore_speed_reduction").is_some())
                .unwrap_or(false)
        })
    }
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
    pub base_ac: i32,
    pub base_speed: i32,
    pub level_total: i32,
    pub token_x: Option<f32>,
    pub token_y: Option<f32>,
    pub abilities: Value,
    pub saves: Value,
    pub skills: Value,
    pub proficiency_bonus: i32,
    pub conditions: Vec<String>,
    pub active_effects: Vec<EffectSnapshot>,
    pub casting: Value,
    pub weapons: Value,
    pub equipment: Value,
    pub race: Option<String>,
    pub classes: Value,
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