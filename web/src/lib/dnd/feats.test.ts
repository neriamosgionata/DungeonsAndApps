import { describe, it, expect } from 'vitest';
import { FEATS, featByKey, featPrereqsMet } from '$lib/feats';

describe('featByKey', () => {
  it('finds a known feat', () => {
    const feat = featByKey('alert');
    expect(feat).toBeDefined();
    expect(feat?.key).toBe('alert');
    expect(feat?.name).toBe('Alert');
  });

  it('returns undefined for unknown key', () => {
    expect(featByKey('does_not_exist')).toBeUndefined();
  });

  it('returns undefined for empty string', () => {
    expect(featByKey('')).toBeUndefined();
  });
});

describe('featPrereqsMet — ability prereq', () => {
  it('passes when ability score meets minimum', () => {
    const feat = featByKey('defensive_duelist')!;
    const sheet = { abilities: { dex: 13 } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('passes when ability score exceeds minimum', () => {
    const feat = featByKey('defensive_duelist')!;
    const sheet = { abilities: { dex: 20 } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('fails when ability score is below minimum', () => {
    const feat = featByKey('defensive_duelist')!;
    const sheet = { abilities: { dex: 12 } };
    expect(featPrereqsMet(feat, sheet)).toBe(false);
  });

  it('falls back to 10 when ability is absent', () => {
    const feat = featByKey('defensive_duelist')!;
    expect(featPrereqsMet(feat, {})).toBe(false);
  });

  it('passes inspiring_leader with cha 13', () => {
    const feat = featByKey('inspiring_leader')!;
    expect(featPrereqsMet(feat, { abilities: { cha: 13 } })).toBe(true);
  });

  it('fails inspiring_leader with cha 12', () => {
    const feat = featByKey('inspiring_leader')!;
    expect(featPrereqsMet(feat, { abilities: { cha: 12 } })).toBe(false);
  });

  it('passes grappler with str 13', () => {
    const feat = featByKey('grappler')!;
    expect(featPrereqsMet(feat, { abilities: { str: 13 } })).toBe(true);
  });

  it('fails grappler with str 10', () => {
    const feat = featByKey('grappler')!;
    expect(featPrereqsMet(feat, { abilities: { str: 10 } })).toBe(false);
  });
});

describe('featPrereqsMet — armor_prof prereq', () => {
  it('passes medium_armor_master when sheet has medium armor', () => {
    const feat = featByKey('medium_armor_master')!;
    const sheet = { proficiencies: { armor: 'Light armor, Medium armor' } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('fails medium_armor_master when sheet lacks medium armor', () => {
    const feat = featByKey('medium_armor_master')!;
    const sheet = { proficiencies: { armor: 'Light armor' } };
    expect(featPrereqsMet(feat, sheet)).toBe(false);
  });

  it('case-insensitive armor_prof check', () => {
    const feat = featByKey('medium_armor_master')!;
    const sheet = { proficiencies: { armor: 'MEDIUM ARMOR' } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('passes heavy_armor_master when sheet has heavy armor', () => {
    const feat = featByKey('heavy_armor_master')!;
    const sheet = { proficiencies: { armor: 'Light armor, Medium armor, Heavy armor' } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('fails heavy_armor_master when sheet lacks heavy armor', () => {
    const feat = featByKey('heavy_armor_master')!;
    const sheet = { proficiencies: { armor: 'Medium armor' } };
    expect(featPrereqsMet(feat, sheet)).toBe(false);
  });

  it('fails when proficiencies.armor is empty string', () => {
    const feat = featByKey('medium_armor_master')!;
    expect(featPrereqsMet(feat, { proficiencies: { armor: '' } })).toBe(false);
  });

  it('fails when proficiencies is missing entirely', () => {
    const feat = featByKey('medium_armor_master')!;
    expect(featPrereqsMet(feat, {})).toBe(false);
  });
});

describe('featPrereqsMet — can_cast prereq', () => {
  it('passes when sheet has a caster class', () => {
    const feat = featByKey('war_caster')!;
    const sheet = { abilities: { wis: 13 }, classes: [{ name: 'Wizard', level: 1 }] };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('passes when sheet has spell slots', () => {
    const feat = featByKey('war_caster')!;
    const sheet = { abilities: { wis: 13 }, slots: { '1': { max: 2 } } };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('passes when sheet has spells', () => {
    const feat = featByKey('war_caster')!;
    const sheet = { abilities: { wis: 13 }, spells: [{ slug: 'fire-bolt' }] };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('fails when sheet has no caster indicators', () => {
    const feat = featByKey('war_caster')!;
    expect(featPrereqsMet(feat, { classes: [{ name: 'Fighter', level: 5 }] })).toBe(false);
  });

  it('fails with empty sheet', () => {
    const feat = featByKey('war_caster')!;
    expect(featPrereqsMet(feat, {})).toBe(false);
  });

  it('passes elemental_adept for bard class', () => {
    const feat = featByKey('elemental_adept')!;
    const sheet = { classes: [{ name: 'Bard', level: 3 }] };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });

  it('passes elemental_adept for partial caster name match (e.g. "Cleric of Life")', () => {
    const feat = featByKey('elemental_adept')!;
    const sheet = { classes: [{ name: 'Cleric of Life', level: 2 }] };
    expect(featPrereqsMet(feat, sheet)).toBe(true);
  });
});

describe('featPrereqsMet — no prereqs', () => {
  it('always passes for feats with no prereqs', () => {
    const noPrereqKeys = ['alert', 'tough', 'lucky', 'mobile', 'resilient', 'skilled'];
    for (const key of noPrereqKeys) {
      const feat = featByKey(key)!;
      expect(featPrereqsMet(feat, {})).toBe(true);
    }
  });
});

describe('FEATS completeness', () => {
  it('every feat has required fields', () => {
    for (const feat of FEATS) {
      expect(typeof feat.key).toBe('string');
      expect(feat.key.length).toBeGreaterThan(0);
      expect(typeof feat.name).toBe('string');
      expect(feat.name.length).toBeGreaterThan(0);
      expect(Array.isArray(feat.prereqs)).toBe(true);
      expect(typeof feat.description).toBe('string');
      expect(feat.description.length).toBeGreaterThan(0);
      expect(typeof feat.mechanics).toBe('string');
      expect(feat.mechanics.length).toBeGreaterThan(0);
      expect(feat.effects).toBeDefined();
      expect(typeof feat.effects).toBe('object');
    }
  });

  it('no duplicate feat keys', () => {
    const keys = FEATS.map((f) => f.key);
    const unique = new Set(keys);
    expect(unique.size).toBe(keys.length);
  });

  it('FEATS contains at least 30 entries', () => {
    expect(FEATS.length).toBeGreaterThanOrEqual(30);
  });
});

describe('FEATS — specific effect fields', () => {
  it('tough has { tough: true }', () => {
    const feat = featByKey('tough')!;
    expect(feat.effects.tough).toBe(true);
  });

  it('alert has { initiative: 5 }', () => {
    const feat = featByKey('alert')!;
    expect(feat.effects.initiative).toBe(5);
  });

  it('lucky has resource Luck Points max 3 reset long', () => {
    const feat = featByKey('lucky')!;
    expect(feat.effects.resource).toEqual({ name: 'Luck Points', max: 3, reset: 'long' });
  });

  it('resilient has save_prof_from_config: true and config_type: ability', () => {
    const feat = featByKey('resilient')!;
    expect(feat.effects.save_prof_from_config).toBe(true);
    expect(feat.effects.config_type).toBe('ability');
  });

  it('dual_wielder has no static ac_bonus (now dynamic in computedAC)', () => {
    const feat = featByKey('dual_wielder')!;
    expect(feat.effects.ac_bonus).toBeUndefined();
  });

  it('medium_armor_master has medium_armor_max_dex: 3', () => {
    const feat = featByKey('medium_armor_master')!;
    expect(feat.effects.medium_armor_max_dex).toBe(3);
  });

  it('heavy_armor_master has nonmagical_damage_reduction: 3', () => {
    const feat = featByKey('heavy_armor_master')!;
    expect(feat.effects.nonmagical_damage_reduction).toBe(3);
  });

  it('observant has passive_perception: 5 and passive_investigation: 5', () => {
    const feat = featByKey('observant')!;
    expect(feat.effects.passive_perception).toBe(5);
    expect(feat.effects.passive_investigation).toBe(5);
  });

  it('mobile has speed: 10', () => {
    const feat = featByKey('mobile')!;
    expect(feat.effects.speed).toBe(10);
  });

  it('inspiring_leader has resource with reset short', () => {
    const feat = featByKey('inspiring_leader')!;
    expect(feat.effects.resource?.reset).toBe('short');
  });

  it('elemental_adept has multi: true', () => {
    const feat = featByKey('elemental_adept')!;
    expect(feat.multi).toBe(true);
  });

  it('martial_adept resource is Superiority Die reset short', () => {
    const feat = featByKey('martial_adept')!;
    expect(feat.effects.resource).toEqual({ name: 'Superiority Die', max: 1, reset: 'short' });
  });

  it('skilled has free_skills: 3 and config_type: skills', () => {
    const feat = featByKey('skilled')!;
    expect(feat.effects.free_skills).toBe(3);
    expect(feat.effects.config_type).toBe('skills');
  });

  it('heavily_armored grants armor_prof Heavy armor and ability str', () => {
    const feat = featByKey('heavily_armored')!;
    expect(feat.effects.armor_prof).toBe('Heavy armor');
    expect(feat.effects.ability).toBe('str');
  });

  it('actor grants ability cha', () => {
    const feat = featByKey('actor')!;
    expect(feat.effects.ability).toBe('cha');
  });

  it('athlete has ability_choice [str, dex] and config_type ability_choice', () => {
    const feat = featByKey('athlete')!;
    expect(feat.effects.ability_choice).toEqual(['str', 'dex']);
    expect(feat.effects.config_type).toBe('ability_choice');
  });
});

describe('FEATS — combat_tag wiring (#35)', () => {
  // The 6 major feats with dynamic combat behavior each carry a combat_tag
  // string that the backend reads from `sheet_raw.feats[].key` to set
  // matching booleans on `ComputedStats`. See backend/src/combat_engine/
  // stats/compute.rs and resolvers/attack.rs for enforcement.
  const tagged: Array<[string, string]> = [
    ['sharpshooter', 'sharpshooter'],
    ['great_weapon_master', 'great_weapon_master'],
    ['crossbow_expert', 'crossbow_expert'],
    ['sentinel', 'sentinel'],
    ['polearm_master', 'polearm_master'],
    ['war_caster', 'war_caster'],
  ];
  for (const [key, tag] of tagged) {
    it(`${key} has combat_tag = '${tag}'`, () => {
      const feat = featByKey(key)!;
      expect(feat.effects.combat_tag).toBe(tag);
    });
  }

  it('non-combat feats do NOT have combat_tag (regression for accidental tagging)', () => {
    for (const key of ['alert', 'mobile', 'tough', 'lucky', 'dual_wielder', 'athlete']) {
      const feat = featByKey(key)!;
      expect(feat.effects.combat_tag).toBeUndefined();
    }
  });

  it('CombatTag union is exhaustive across the 6 tagged feats', () => {
    const expected = new Set([
      'sharpshooter', 'great_weapon_master', 'crossbow_expert',
      'sentinel', 'polearm_master', 'war_caster',
    ]);
    const seen = new Set<string>();
    for (const f of FEATS) {
      if (f.effects.combat_tag) seen.add(f.effects.combat_tag);
    }
    expect(seen).toEqual(expected);
  });
});
