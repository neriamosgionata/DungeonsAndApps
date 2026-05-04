import { describe, it, expect } from 'vitest';
import en from './en.json';
import loc from './it.json';

type JsonObj = Record<string, unknown>;

function flatKeys(obj: JsonObj, prefix = ''): string[] {
  const out: string[] = [];
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (typeof v === 'object' && v !== null) {
      out.push(...flatKeys(v as JsonObj, key));
    } else {
      out.push(key);
    }
  }
  return out;
}

function flatEntries(obj: JsonObj, prefix = ''): Array<[string, string]> {
  const out: Array<[string, string]> = [];
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (typeof v === 'object' && v !== null) {
      out.push(...flatEntries(v as JsonObj, key));
    } else {
      out.push([key, String(v)]);
    }
  }
  return out;
}

describe('EN and IT key parity', () => {
  it('EN and IT have the same set of flat keys', () => {
    const enKeys = flatKeys(en as unknown as JsonObj).sort();
    const itKeys = flatKeys(loc as unknown as JsonObj).sort();
    expect(enKeys).toEqual(itKeys);
  });

  it('EN and IT have the same top-level namespaces', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(loc).sort());
  });

  const namespaces = [
    'character', 'spells', 'common', 'nav', 'visibility',
    'initiative', 'chat', 'auth', 'campaigns', 'recap',
    'map', 'group', 'lore', 'news', 'factions', 'npcs',
    'presence', 'invitations', 'users', 'dice', 'errors',
    'members', 'settings', 'campaign', 'upload', 'notifications',
  ] as const;

  for (const ns of namespaces) {
    it(`EN and IT have same keys in "${ns}"`, () => {
      const enNs = (en as unknown as JsonObj)[ns] as JsonObj;
      const itNs = (loc as unknown as JsonObj)[ns] as JsonObj;
      expect(Object.keys(enNs).sort()).toEqual(Object.keys(itNs).sort());
    });
  }
});

describe('No empty string values', () => {
  it('EN has no empty string values', () => {
    const empties = flatEntries(en as unknown as JsonObj).filter(([, v]) => v === '');
    expect(empties).toHaveLength(0);
  });

  it('IT has no empty string values', () => {
    const empties = flatEntries(loc as unknown as JsonObj).filter(([, v]) => v === '');
    expect(empties).toHaveLength(0);
  });
});

describe('character.ability_* keys', () => {
  const abilities = ['str', 'dex', 'con', 'int', 'wis', 'cha'] as const;

  for (const ab of abilities) {
    it(`EN has character.ability_${ab}`, () => {
      expect(en.character).toHaveProperty(`ability_${ab}`);
    });

    it(`IT has character.ability_${ab}`, () => {
      expect(loc.character).toHaveProperty(`ability_${ab}`);
    });
  }
});

describe('character.skill_* keys', () => {
  const skills = [
    'acrobatics', 'animal_handling', 'arcana', 'athletics',
    'deception', 'history', 'insight', 'intimidation',
    'investigation', 'medicine', 'nature', 'perception',
    'performance', 'persuasion', 'religion', 'sleight_of_hand',
    'stealth', 'survival',
  ] as const;

  for (const skill of skills) {
    it(`EN has character.skill_${skill}`, () => {
      expect(en.character).toHaveProperty(`skill_${skill}`);
    });

    it(`IT has character.skill_${skill}`, () => {
      expect(loc.character).toHaveProperty(`skill_${skill}`);
    });
  }
});

describe('character feat badge keys', () => {
  const featBadgeKeys = [
    'feat_charger',
    'feat_crossbow_expert',
    'feat_defensive_duelist',
    'feat_gwm',
    'feat_healer',
    'feat_mage_slayer',
    'feat_mounted_combatant',
    'feat_polearm_master',
    'feat_savage_attacker',
    'feat_sentinel',
    'feat_sharpshooter',
    'feat_shield_master',
    'feat_skulker',
    'feat_spell_sniper',
  ] as const;

  for (const key of featBadgeKeys) {
    it(`EN has character.${key}`, () => {
      expect(en.character).toHaveProperty(key);
    });

    it(`IT has character.${key}`, () => {
      expect(loc.character).toHaveProperty(key);
    });
  }
});

describe('character condition-related keys', () => {
  it('EN has character.conditions', () => {
    expect(en.character).toHaveProperty('conditions');
  });

  it('IT has character.conditions', () => {
    expect(loc.character).toHaveProperty('conditions');
  });

  it('EN has character.exhaustion', () => {
    expect(en.character).toHaveProperty('exhaustion');
  });

  it('IT has character.exhaustion', () => {
    expect(loc.character).toHaveProperty('exhaustion');
  });

  it('EN has character.stealth_disadvantage', () => {
    expect(en.character).toHaveProperty('stealth_disadvantage');
  });

  it('IT has character.stealth_disadvantage', () => {
    expect(loc.character).toHaveProperty('stealth_disadvantage');
  });

  it('EN has character.death_saves', () => {
    expect(en.character).toHaveProperty('death_saves');
  });

  it('IT has character.death_saves', () => {
    expect(loc.character).toHaveProperty('death_saves');
  });
});

describe('recently added keys', () => {
  const recentCharacterKeys = [
    'rage_damage',
    'brutal_critical',
    'wild_shape_cr',
    'swim_speed',
    'climb_speed',
    'fly_speed',
    'feral_instinct',
    'persistent_rage',
    'primal_champion',
    'destroy_undead',
    'stunning_strike',
    'uncanny_dodge',
    'blindsense',
    'divine_health',
    'aura_of_courage',
    'remarkable_athlete',
    'alert_surprise',
    'heavy_armor_master_dr',
    'tavern_brawler_d4',
    'savage_attacks',
    'mask_of_wild',
    'sunlight_sensitivity',
    'hp_max_reduction',
  ] as const;

  for (const key of recentCharacterKeys) {
    it(`EN has character.${key}`, () => {
      expect(en.character).toHaveProperty(key);
    });

    it(`IT has character.${key}`, () => {
      expect(loc.character).toHaveProperty(key);
    });
  }
});

describe('visibility keys', () => {
  it('EN visibility has master, players, label — no private or public', () => {
    expect(en.visibility).toHaveProperty('master');
    expect(en.visibility).toHaveProperty('players');
    expect(en.visibility).toHaveProperty('label');
    expect(en.visibility).not.toHaveProperty('private');
    expect(en.visibility).not.toHaveProperty('public');
  });

  it('IT visibility has master, players, label — no private or public', () => {
    expect(loc.visibility).toHaveProperty('master');
    expect(loc.visibility).toHaveProperty('players');
    expect(loc.visibility).toHaveProperty('label');
    expect(loc.visibility).not.toHaveProperty('private');
    expect(loc.visibility).not.toHaveProperty('public');
  });
});

describe('presence locale', () => {
  it('IT presence.online is "in linea"', () => {
    expect(loc.presence.online).toBe('in linea');
  });

  it('IT presence.offline is "non in linea"', () => {
    expect(loc.presence.offline).toBe('non in linea');
  });
});

describe('spells keys', () => {
  it('IT spells.cantrip is "Trucchetto"', () => {
    expect(loc.spells.cantrip).toBe('Trucchetto');
  });

  it('EN spells.cantrip is "Cantrip"', () => {
    expect(en.spells.cantrip).toBe('Cantrip');
  });
});

describe('IT tab_vitals value', () => {
  it('IT character.tab_vitals is "Condizione"', () => {
    expect(loc.character.tab_vitals).toBe('Condizione');
  });

  it('IT character.background is "Trascorsi"', () => {
    expect(loc.character.background).toBe('Trascorsi');
  });

  it('IT character.tab_story is "Storia"', () => {
    expect(loc.character.tab_story).toBe('Storia');
  });
});

describe('combat and initiative keys', () => {
  const initiativeKeys = [
    'action_action', 'action_bonus', 'action_reaction',
    'action_movement', 'action_legendary', 'action_legendary_resistance',
    'action_lair', 'action_used', 'action_available',
    'effect_buff', 'effect_debuff', 'effect_neutral', 'effect_condition',
  ] as const;

  for (const key of initiativeKeys) {
    it(`EN has initiative.${key}`, () => {
      expect(en.initiative).toHaveProperty(key);
    });

    it(`IT has initiative.${key}`, () => {
      expect(loc.initiative).toHaveProperty(key);
    });
  }
});

describe('errors keys', () => {
  const errorKeys = [
    'not_found', 'unauthorized', 'forbidden',
    'conflict', 'bad_request', 'validation', 'internal',
  ] as const;

  for (const key of errorKeys) {
    it(`EN has errors.${key}`, () => {
      expect(en.errors).toHaveProperty(key);
    });

    it(`IT has errors.${key}`, () => {
      expect(loc.errors).toHaveProperty(key);
    });
  }
});

describe('common keys', () => {
  const commonKeys = [
    'add', 'create', 'cancel', 'save', 'delete', 'edit',
    'close', 'search', 'all', 'none', 'loading',
    'description', 'name', 'visibility', 'category',
    'title', 'body', 'remove', 'end',
  ] as const;

  for (const key of commonKeys) {
    it(`EN has common.${key}`, () => {
      expect(en.common).toHaveProperty(key);
    });

    it(`IT has common.${key}`, () => {
      expect(loc.common).toHaveProperty(key);
    });
  }
});
