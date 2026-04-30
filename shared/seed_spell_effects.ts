#!/usr/bin/env bun
// Seed spell effect templates from SRD 5.1 data.
// Reads spells-srd.json and outputs SQL to update the spells.effects column.

const input = new URL('./spells-srd.json', import.meta.url).pathname;
const output = process.argv[2] ?? new URL('./seed_spell_effects.sql', import.meta.url).pathname;

interface Spell {
  slug: string;
  name: string;
  duration: string;
  concentration: boolean;
}

interface EffectTemplate {
  name: string;
  kind: 'buff' | 'debuff' | 'neutral' | 'condition';
  icon: string;
  duration_override?: { unit: 'rounds' | 'minutes' | 'hours' | 'permanent'; value: number | null } | null;
  tick_trigger?: 'round_end' | 'target_turn_start' | 'target_turn_end' | 'caster_turn_start' | 'caster_turn_end' | 'never';
  modifiers?: Record<string, unknown>;
}

// Manual mapping: spell slug → effect template(s)
// Spells not listed here have no auto-generated effects (e.g. Fireball = instantaneous damage)
const EFFECT_MAP: Record<string, EffectTemplate[]> = {
  // Buffs — concentration
  'bless': [{ name: 'Blessed', kind: 'buff', icon: 'sparkles', modifiers: { attack_bonus: '1d4', save_bonus: '1d4' } }],
  'aid': [{ name: 'Aided', kind: 'buff', icon: 'heart-pulse', modifiers: { hp_bonus: '5' } }],
  'barkskin': [{ name: 'Barkskin', kind: 'buff', icon: 'tree-pine', modifiers: { ac_min: '16' } }],
  'blur': [{ name: 'Blurred', kind: 'buff', icon: 'cloud-fog', modifiers: { disadvantage_attackers: 'true' } }],
  'death-ward': [{ name: 'Death Ward', kind: 'buff', icon: 'shield-check', modifiers: { death_ward: 'true' } }],
  'divine-favor': [{ name: 'Divine Favor', kind: 'buff', icon: 'sun', modifiers: { weapon_damage_bonus: '1d4' } }],
  'enhance-ability': [{ name: 'Enhanced', kind: 'buff', icon: 'zap', modifiers: { ability_advantage: 'true' } }],
  'fly': [{ name: 'Flying', kind: 'buff', icon: 'feather', modifiers: { flying_speed: '60' } }],
  'freedom-of-movement': [{ name: 'Freedom of Movement', kind: 'buff', icon: 'footprints', modifiers: { ignore_difficult_terrain: 'true', grapple_immunity: 'true' } }],
  'gaseous-form': [{ name: 'Gaseous Form', kind: 'buff', icon: 'cloud', modifiers: { flying_speed: '10', damage_resistance: 'true' } }],
  'greater-invisibility': [{ name: 'Invisible', kind: 'buff', icon: 'eye-off', modifiers: { invisible: 'true', attack_advantage: 'true' } }],
  'guidance': [{ name: 'Guided', kind: 'buff', icon: 'compass', modifiers: { ability_bonus: '1d4' } }],
  'haste': [{ name: 'Hasted', kind: 'buff', icon: 'zap-fast', modifiers: { speed_doubled: 'true', ac_bonus: '2', dex_save_advantage: 'true' } }],
  'heroism': [{ name: 'Heroic', kind: 'buff', icon: 'flame', modifiers: { temp_hp_per_turn: '5', charm_fear_immunity: 'true' } }],
  'holy-aura': [{ name: 'Holy Aura', kind: 'buff', icon: 'sun-medium', modifiers: { ac_bonus: '2', save_advantage: 'true', blinded_immunity: 'true' } }],
  'invisibility': [{ name: 'Invisible', kind: 'buff', icon: 'eye-off', modifiers: { invisible: 'true', attack_advantage: 'true' } }],
  'longstrider': [{ name: 'Longstrider', kind: 'buff', icon: 'footprints', modifiers: { speed_bonus: '10' } }],
  'mage-armor': [{ name: 'Mage Armor', kind: 'buff', icon: 'shield', modifiers: { ac_base: '13+dex' } }],
  'mirror-image': [{ name: 'Mirror Images', kind: 'buff', icon: 'copy', modifiers: { mirror_images: '3' } }],
  'pass-without-trace': [{ name: 'Pass Without Trace', kind: 'buff', icon: 'leaf', modifiers: { stealth_bonus: '10' } }],
  'protection-from-energy': [{ name: 'Energy Resistance', kind: 'buff', icon: 'flame-kindling' }],
  'protection-from-evil-and-good': [{ name: 'Protected', kind: 'buff', icon: 'shield-check', modifiers: { fiend_aberration_celestial_protection: 'true' } }],
  'regenerate': [{ name: 'Regenerating', kind: 'buff', icon: 'heart-pulse', modifiers: { hp_regen_per_turn: '10', limb_regen: 'true' } }],
  'resistance': [{ name: 'Resistant', kind: 'buff', icon: 'shield', modifiers: { save_bonus: '1d4' } }],
  'sanctuary': [{ name: 'Sanctuary', kind: 'buff', icon: 'shield-plus', modifiers: { sanctuary: 'true' } }],
  'shield-of-faith': [{ name: 'Shield of Faith', kind: 'buff', icon: 'shield-check', modifiers: { ac_bonus: '2' } }],
  'spider-climb': [{ name: 'Spider Climb', kind: 'buff', icon: 'move-vertical', modifiers: { spider_climb: 'true' } }],
  'stoneskin': [{ name: 'Stoneskin', kind: 'buff', icon: 'gem', modifiers: { nonmagical_damage_resistance: 'true' } }],
  'true-seeing': [{ name: 'True Sight', kind: 'buff', icon: 'eye', modifiers: { truesight: '120' } }],
  'water-walk': [{ name: 'Water Walk', kind: 'buff', icon: 'waves', modifiers: { water_walk: 'true' } }],

  // Debuffs — concentration
  'bane': [{ name: 'Baned', kind: 'debuff', icon: 'skull', modifiers: { attack_penalty: '1d4', save_penalty: '1d4' } }],
  'bestow-curse': [{ name: 'Cursed', kind: 'debuff', icon: 'flame', modifiers: { curse: 'true' } }],
  'confusion': [{ name: 'Confused', kind: 'debuff', icon: 'brain-circuit', modifiers: { confused: 'true' } }],
  'dominate-monster': [{ name: 'Dominated', kind: 'debuff', icon: 'crown', modifiers: { charmed: 'true', dominated: 'true' } }],
  'dominate-person': [{ name: 'Dominated', kind: 'debuff', icon: 'crown', modifiers: { charmed: 'true', dominated: 'true' } }],
  'entangle': [{ name: 'Entangled', kind: 'debuff', icon: 'vine', modifiers: { restrained: 'true' } }],
  'faerie-fire': [{ name: 'Faerie Fire', kind: 'debuff', icon: 'sparkles', modifiers: { invisible_revealed: 'true', attack_disadvantage: 'true' } }],
  'fear': [{ name: 'Frightened', kind: 'debuff', icon: 'ghost', modifiers: { frightened: 'true' } }],
  'hold-monster': [{ name: 'Paralyzed', kind: 'debuff', icon: 'lock', modifiers: { paralyzed: 'true' } }],
  'hold-person': [{ name: 'Paralyzed', kind: 'debuff', icon: 'lock', modifiers: { paralyzed: 'true' } }],
  'hunters-mark': [{ name: "Hunter's Mark", kind: 'debuff', icon: 'target', modifiers: { hunter_damage_bonus: '1d6' } }],
  'hypnotic-pattern': [{ name: 'Charmed', kind: 'debuff', icon: 'eye', modifiers: { incapacitated: 'true', charmed: 'true' } }],
  'ray-of-enfeeblement': [{ name: 'Enfeebled', kind: 'debuff', icon: 'zap-off', modifiers: { weapon_damage_halved: 'true' } }],
  'slow': [{ name: 'Slowed', kind: 'debuff', icon: 'clock', modifiers: { speed_halved: 'true', ac_penalty: '2', dex_save_penalty: 'true' } }],
  'web': [{ name: 'Restrained', kind: 'debuff', icon: 'spider-web', modifiers: { restrained: 'true' } }],

  // Neutral / transformation
  'enlargereduce': [{ name: 'Enlarged', kind: 'neutral', icon: 'arrow-up', modifiers: { size_increase: '1', str_advantage: 'true', weapon_damage_bonus: '1d4' } }],
  'polymorph': [{ name: 'Polymorphed', kind: 'neutral', icon: 'rabbit', modifiers: { polymorphed: 'true' } }],

  // Special — 1 round, no concentration
  'shield': [{ name: 'Shielded', kind: 'buff', icon: 'shield-plus', duration_override: { unit: 'rounds', value: 1 }, tick_trigger: 'target_turn_start', modifiers: { ac_bonus: '5' } }],
  'jump': [{ name: 'Jumping', kind: 'buff', icon: 'arrow-up', modifiers: { jump_tripled: 'true' } }],
  'spiritual-weapon': [{ name: 'Spiritual Weapon', kind: 'buff', icon: 'sword', modifiers: { spiritual_weapon: 'true' } }],
  'darkvision': [{ name: 'Darkvision', kind: 'buff', icon: 'eye', modifiers: { darkvision: '60' } }],
  'daylight': [{ name: 'Daylight', kind: 'buff', icon: 'sun', modifiers: { daylight: 'true' } }],
  'antimagic-field': [{ name: 'Antimagic Field', kind: 'debuff', icon: 'ban', modifiers: { antimagic: 'true' } }],

  // Movement effects — push, pull, forced move
  'thunderwave': [{ name: 'Pushed', kind: 'debuff', icon: 'wind', duration_override: { unit: 'rounds', value: 1 }, tick_trigger: 'target_turn_start', modifiers: { movement: { type: 'push', distance_ft: 10, direction: 'away_from_caster' } } }],
  'gust-of-wind': [{ name: 'Gust Pushed', kind: 'debuff', icon: 'wind', modifiers: { movement: { type: 'push', distance_ft: 10, direction: 'away_from_caster' } } }],
  'command': [{ name: 'Commanded to Flee', kind: 'debuff', icon: 'footprints', duration_override: { unit: 'rounds', value: 1 }, tick_trigger: 'target_turn_start', modifiers: { movement: { type: 'forced_move', distance_ft: 60, direction: 'away_from_caster' } } }],
  'fear': [{ name: 'Frightened', kind: 'debuff', icon: 'ghost', modifiers: { frightened: 'true', movement: { type: 'forced_move', distance_ft: 999, direction: 'away_from_caster' } } }],
  'misty-step': [{ name: 'Teleport Ready', kind: 'buff', icon: 'zap', duration_override: { unit: 'rounds', value: 1 }, tick_trigger: 'target_turn_start', modifiers: { movement: { type: 'teleport', distance_ft: 30, direction: 'chosen' } } }],
  'dimension-door': [{ name: 'Teleport Ready', kind: 'buff', icon: 'zap', duration_override: { unit: 'rounds', value: 1 }, tick_trigger: 'target_turn_start', modifiers: { movement: { type: 'teleport', distance_ft: 500, direction: 'chosen' } } }],
};

function parseDuration(spell: Spell): { unit: 'rounds' | 'minutes' | 'hours' | 'permanent'; value: number | null; tick: 'round_end' | 'target_turn_start' | 'never' } {
  const d = spell.duration.trim().toLowerCase();

  // Instantaneous / Special → no effect
  if (d === 'instantaneous' || d === 'special') {
    return { unit: 'permanent', value: null, tick: 'never' };
  }

  // 1 round → special: expires at start of target's next turn
  if (d === '1 round') {
    return { unit: 'rounds', value: 1, tick: 'target_turn_start' };
  }

  // Concentration, up to X
  const concMatch = d.match(/^concentration, up to (\d+) (minute|hour|round|day|year)s?$/);
  if (concMatch) {
    const num = parseInt(concMatch[1], 10);
    const unit = concMatch[2];
    return convertUnit(unit, num);
  }

  // Up to X (no concentration)
  const upToMatch = d.match(/^up to (\d+) (minute|hour|round|day|year)s?$/);
  if (upToMatch) {
    const num = parseInt(upToMatch[1], 10);
    const unit = upToMatch[2];
    return convertUnit(unit, num);
  }

  // Plain X minutes/hours/rounds
  const plainMatch = d.match(/^(\d+) (minute|hour|round|day|year)s?$/);
  if (plainMatch) {
    const num = parseInt(plainMatch[1], 10);
    const unit = plainMatch[2];
    return convertUnit(unit, num);
  }

  // Until dispelled / Permanent
  if (d.includes('dispelled') || d === 'permanent' || d === 'until triggered') {
    return { unit: 'permanent', value: null, tick: 'never' };
  }

  // Default: treat as permanent (manual removal)
  return { unit: 'permanent', value: null, tick: 'never' };
}

function convertUnit(unit: string, num: number): { unit: 'rounds' | 'minutes' | 'hours' | 'permanent'; value: number | null; tick: 'round_end' | 'target_turn_start' | 'never' } {
  switch (unit) {
    case 'round':
      return { unit: 'rounds', value: num, tick: num === 1 ? 'target_turn_start' : 'round_end' };
    case 'minute':
      return { unit: 'rounds', value: num * 10, tick: 'round_end' };
    case 'hour':
      return { unit: 'rounds', value: num * 600, tick: 'round_end' };
    case 'day':
      return { unit: 'permanent', value: null, tick: 'never' }; // too long for combat tracking
    case 'year':
      return { unit: 'permanent', value: null, tick: 'never' };
    default:
      return { unit: 'permanent', value: null, tick: 'never' };
  }
}

const data = JSON.parse(await Bun.file(input).text());
const spells: Spell[] = data.spells;

const updates: string[] = [];

for (const spell of spells) {
  const templates = EFFECT_MAP[spell.slug];
  if (!templates) continue;

  const parsed = parseDuration(spell);

  const effects = templates.map((t) => {
    const dur = t.duration_override ?? { unit: parsed.unit, value: parsed.value };
    const tick = t.tick_trigger ?? parsed.tick;
    return {
      name: t.name,
      kind: t.kind,
      icon: t.icon,
      duration_unit: dur.unit,
      duration_value: dur.value,
      tick_trigger: tick,
      modifiers: t.modifiers ?? {},
    };
  });

  const json = JSON.stringify(effects).replace(/'/g, "''");
  updates.push(`update spells set effects = '${json}'::jsonb where slug = '${spell.slug}';`);
}

const sql = `-- Auto-generated spell effect templates\n-- Generated: ${new Date().toISOString()}\n-- Source: SRD 5.1 spells-srd.json\n\n${updates.join('\n')}\n`;

await Bun.write(output, sql);
console.log(`Generated ${updates.length} spell effect mappings → ${output}`);
