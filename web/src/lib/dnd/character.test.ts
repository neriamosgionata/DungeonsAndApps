import { describe, it, expect } from 'vitest';

/**
 * Character sheet calculation utilities
 * Extracted from character page for testing
 */

type Ability = 'str' | 'dex' | 'con' | 'int' | 'wis' | 'cha';
type Skill = string;

interface Character {
  sheet?: {
    abilities?: Record<Ability, number>;
    classes?: Array<{ name: string; level: number; subclass?: string }>;
    resources?: Array<{ name: string; current: number; max: number }>;
    features?: Array<{ name: string; source?: string }>;
    armor?: { type?: string; ac_base?: number; max_dex?: number };
    hp?: { max?: number; current?: number };
    saving_throws?: Record<string, boolean>;
    skills?: Record<string, 'proficient' | 'expert'>;
    race?: string;
    fighting_styles?: string[];
  };
}

const SKILLS: Array<{ key: Skill; name: string; ability: Ability }> = [
  { key: 'acrobatics', name: 'Acrobatics', ability: 'dex' },
  { key: 'animal_handling', name: 'Animal Handling', ability: 'wis' },
  { key: 'arcana', name: 'Arcana', ability: 'int' },
  { key: 'athletics', name: 'Athletics', ability: 'str' },
  { key: 'deception', name: 'Deception', ability: 'cha' },
  { key: 'history', name: 'History', ability: 'int' },
  { key: 'insight', name: 'Insight', ability: 'wis' },
  { key: 'intimidation', name: 'Intimidation', ability: 'cha' },
  { key: 'investigation', name: 'Investigation', ability: 'int' },
  { key: 'medicine', name: 'Medicine', ability: 'wis' },
  { key: 'nature', name: 'Nature', ability: 'int' },
  { key: 'perception', name: 'Perception', ability: 'wis' },
  { key: 'performance', name: 'Performance', ability: 'cha' },
  { key: 'persuasion', name: 'Persuasion', ability: 'cha' },
  { key: 'religion', name: 'Religion', ability: 'int' },
  { key: 'sleight_of_hand', name: 'Sleight of Hand', ability: 'dex' },
  { key: 'stealth', name: 'Stealth', ability: 'dex' },
  { key: 'survival', name: 'Survival', ability: 'wis' },
];

function abilityMod(score: number | undefined): number {
  if (!score || score < 1) return -5;
  return Math.floor((Math.min(score, 30) - 10) / 2);
}

function abilityScore(c: Character, ab: Ability): number {
  const base = c.sheet?.abilities?.[ab] ?? 10;
  return base + racialAbilityBonus(c, ab);
}

function racialAbilityBonus(c: Character, ab: Ability): number {
  const race = ((c as any).race ?? c.sheet?.race ?? '').toString().toLowerCase();
  const bonuses: Record<string, Partial<Record<Ability, number>>> = {
    'dragonborn': { str: 2, cha: 1 },
    'dwarf': { con: 2 },
    'elf': { dex: 2 },
    'gnome': { int: 2 },
    'half-elf': { cha: 2 },
    'halfling': { dex: 2 },
    'half-orc': { str: 2, con: 1 },
    'human': { str: 1, dex: 1, con: 1, int: 1, wis: 1, cha: 1 },
    'tiefling': { cha: 2, int: 1 },
  };
  return bonuses[race]?.[ab] ?? 0;
}

function skillMod(c: Character, sk: Skill): number {
  const skill = SKILLS.find((s) => s.key === sk);
  if (!skill) return 0;

  const abMod = abilityMod(abilityScore(c, skill.ability));
  const prof = profBonus(c.sheet?.classes?.reduce((sum, cl) => sum + cl.level, 0) ?? 1);
  const proficiency = c.sheet?.skills?.[sk];

  if (proficiency === 'expert') return abMod + prof * 2;
  if (proficiency === 'proficient') return abMod + prof;
  if (hasJackOfAllTrades(c)) return abMod + Math.floor(prof / 2);
  return abMod;
}

function profBonus(level: number): number {
  if (level >= 17) return 6;
  if (level >= 13) return 5;
  if (level >= 9) return 4;
  if (level >= 5) return 3;
  return 2;
}

function hasJackOfAllTrades(c: Character): boolean {
  return c.sheet?.classes?.some((cl) =>
    cl.name.toLowerCase().includes('bard') && cl.level >= 2
  ) ?? false;
}

function saveMod(c: Character, ab: Ability): number {
  const abScore = abilityScore(c, ab);
  const baseMod = abilityMod(abScore);
  const isProficient = c.sheet?.saving_throws?.[ab] ?? false;
  const level = c.sheet?.classes?.reduce((sum, cl) => sum + cl.level, 0) ?? 1;

  if (isProficient) {
    return baseMod + profBonus(level);
  }

  return baseMod;
}

function computedAC(c: Character): number {
  const armor = c.sheet?.armor;
  const dexMod = abilityMod(abilityScore(c, 'dex'));

  if (!armor || !armor.type) return 10 + dexMod;

  switch (armor.type) {
    case 'unarmored_barbarian':
      return 10 + dexMod + abilityMod(abilityScore(c, 'con'));
    case 'unarmored_monk':
      return 10 + dexMod + abilityMod(abilityScore(c, 'wis'));
    case 'mage_armor':
    case 'draconic':
      return 13 + dexMod;
    default:
      return (armor.ac_base ?? 10) + Math.min(dexMod, armor.max_dex ?? 99);
  }
}

function passivePerception(c: Character): number {
  const perc = SKILLS.find((s) => s.key === 'perception')!;
  const mod = skillMod(c, 'perception');
  const hasAdv = false; // Would check for advantage
  const bonus = hasAdv ? 5 : 0;
  return 10 + mod + bonus;
}

function classLevel(c: Character, cls: string): number {
  return c.sheet?.classes?.find((cl) =>
    cl.name.toLowerCase().includes(cls.toLowerCase())
  )?.level ?? 0;
}

function hasReliableTalent(c: Character): boolean {
  const rogueLevel = classLevel(c, 'rogue');
  return rogueLevel >= 11;
}

function hasEvasion(c: Character): boolean {
  const rogueLevel = classLevel(c, 'rogue');
  const monkLevel = classLevel(c, 'monk');
  return rogueLevel >= 7 || monkLevel >= 7;
}

// =====================================================================
// Tests
// =====================================================================

describe('abilityScore', () => {
  it('returns base + racial bonus', () => {
    const c: Character = {
      sheet: {
        race: 'Dragonborn',
        abilities: { str: 15, dex: 10, con: 10, int: 10, wis: 10, cha: 10 }
      }
    };
    expect(abilityScore(c, 'str')).toBe(17); // 15 + 2
    expect(abilityScore(c, 'cha')).toBe(11); // 10 + 1
  });

  it('returns base for no racial bonus', () => {
    const c: Character = {
      sheet: {
        race: 'Human',
        abilities: { str: 12, dex: 12, con: 12, int: 12, wis: 12, cha: 12 }
      }
    };
    expect(abilityScore(c, 'str')).toBe(13); // 12 + 1
  });
});

describe('saveMod', () => {
  it('adds proficiency for proficient saves', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 16, dex: 10, con: 10, int: 10, wis: 10, cha: 10 },
        classes: [{ name: 'Fighter', level: 5 }],
        saving_throws: { str: true }
      }
    };
    // Str 16 = +3, +3 prof = +6
    expect(saveMod(c, 'str')).toBe(6);
  });

  it('does not add proficiency for non-proficient', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 16, dex: 10, con: 10, int: 10, wis: 10, cha: 10 },
        classes: [{ name: 'Fighter', level: 5 }],
        saving_throws: { str: false }
      }
    };
    expect(saveMod(c, 'str')).toBe(3); // Just ability mod
  });
});

describe('skillMod', () => {
  it('adds proficiency for proficient skills', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 16, dex: 10, con: 10, int: 10, wis: 10, cha: 10 },
        classes: [{ name: 'Fighter', level: 5 }],
        skills: { athletics: 'proficient' }
      }
    };
    // Athletics uses Str (+3), +3 prof = +6
    expect(skillMod(c, 'athletics')).toBe(6);
  });

  it('adds double proficiency for expert skills', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 10, dex: 16, con: 10, int: 10, wis: 10, cha: 10 },
        classes: [{ name: 'Rogue', level: 5 }],
        skills: { stealth: 'expert' }
      }
    };
    // Stealth uses Dex (+3), +6 expertise = +9
    expect(skillMod(c, 'stealth')).toBe(9);
  });
});

describe('computedAC', () => {
  it('calculates unarmored', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 10, dex: 14, con: 10, int: 10, wis: 10, cha: 10 }
      }
    };
    // 10 + 2 dex = 12
    expect(computedAC(c)).toBe(12);
  });

  it('calculates with armor', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 10, dex: 14, con: 10, int: 10, wis: 10, cha: 10 },
        armor: { type: 'medium', ac_base: 14, max_dex: 2 }
      }
    };
    // 14 + 2 dex (capped) = 16
    expect(computedAC(c)).toBe(16);
  });

  it('respects max dex on armor', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 10, dex: 18, con: 10, int: 10, wis: 10, cha: 10 },
        armor: { type: 'medium', ac_base: 14, max_dex: 2 }
      }
    };
    // 14 + 2 (capped, not +4) = 16
    expect(computedAC(c)).toBe(16);
  });

  it('calculates monk unarmored defense', () => {
    const c: Character = {
      sheet: {
        race: 'Human',
        abilities: { str: 10, dex: 16, con: 10, int: 10, wis: 16, cha: 10 },
        classes: [{ name: 'Monk', level: 5 }],
        armor: { type: 'unarmored_monk' }
      }
    };
    // 10 + 3 dex + 3 wis = 16
    expect(computedAC(c)).toBe(16);
  });
});

describe('passivePerception', () => {
  it('calculates 10 + skill mod', () => {
    const c: Character = {
      sheet: {
        abilities: { str: 10, dex: 10, con: 10, int: 10, wis: 14, cha: 10 },
        skills: { perception: 'proficient' },
        classes: [{ name: 'Cleric', level: 5 }]
      }
    };
    // Wis 14 = +2, +3 prof = +5, 10 + 5 = 15
    expect(passivePerception(c)).toBe(15);
  });
});

describe('classLevel', () => {
  it('finds class level', () => {
    const c: Character = {
      sheet: {
        classes: [
          { name: 'Fighter', level: 5 },
          { name: 'Wizard', level: 3 }
        ]
      }
    };
    expect(classLevel(c, 'fighter')).toBe(5);
    expect(classLevel(c, 'wizard')).toBe(3);
    expect(classLevel(c, 'rogue')).toBe(0);
  });
});

describe('hasReliableTalent', () => {
  it('true for rogue 11+', () => {
    const c: Character = {
      sheet: { classes: [{ name: 'Rogue', level: 11 }] }
    };
    expect(hasReliableTalent(c)).toBe(true);
  });

  it('false for rogue below 11', () => {
    const c: Character = {
      sheet: { classes: [{ name: 'Rogue', level: 10 }] }
    };
    expect(hasReliableTalent(c)).toBe(false);
  });
});

describe('hasEvasion', () => {
  it('true for rogue 7+', () => {
    const c: Character = {
      sheet: { classes: [{ name: 'Rogue', level: 7 }] }
    };
    expect(hasEvasion(c)).toBe(true);
  });

  it('true for monk 7+', () => {
    const c: Character = {
      sheet: { classes: [{ name: 'Monk', level: 7 }] }
    };
    expect(hasEvasion(c)).toBe(true);
  });

  it('false for fighter', () => {
    const c: Character = {
      sheet: { classes: [{ name: 'Fighter', level: 10 }] }
    };
    expect(hasEvasion(c)).toBe(false);
  });
});
