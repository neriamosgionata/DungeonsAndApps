import { describe, it, expect } from 'vitest';

/**
 * D&D 5e calculation utilities
 * Extracted from character sheet for testing
 */

function abilityMod(score: number | undefined): number {
  if (!score || score < 1) return -5;
  return Math.floor((Math.min(score, 30) - 10) / 2);
}

function profBonus(level: number): number {
  if (level >= 17) return 6;
  if (level >= 13) return 5;
  if (level >= 9) return 4;
  if (level >= 5) return 3;
  return 2;
}

function cantripDiceMultiplier(totalLevel: number): number {
  if (totalLevel >= 17) return 4;
  if (totalLevel >= 11) return 3;
  if (totalLevel >= 5) return 2;
  return 1;
}

function sneakAttackDice(c: { level_total: number; classes?: Array<{ name: string; level: number }> }): number {
  // Rogue: 1d6 at level 1, +1d6 every 2 levels
  const rogueLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('rogue'))?.level ?? 0;
  if (rogueLevel === 0) return 0;
  return Math.ceil(rogueLevel / 2);
}

function martialArtsDie(c: { level_total: number; classes?: Array<{ name: string; level: number }> }): string | null {
  const monkLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('monk'))?.level ?? 0;
  if (monkLevel === 0) return null;
  if (monkLevel >= 17) return 'd10';
  if (monkLevel >= 11) return 'd8';
  if (monkLevel >= 5) return 'd6';
  return 'd4';
}

function extraAttackCount(c: { level_total: number; classes?: Array<{ name: string; level: number }> }): number {
  // Most martial classes get Extra Attack at level 5
  const martialClasses = ['fighter', 'barbarian', 'paladin', 'ranger', 'monk'];
  let maxAttacks = 1;

  for (const cl of c.classes ?? []) {
    const name = cl.name.toLowerCase();
    const level = cl.level;

    if (name.includes('fighter')) {
      // Fighter: 2 attacks at 5, 3 at 11, 4 at 20
      if (level >= 20) maxAttacks = Math.max(maxAttacks, 4);
      else if (level >= 11) maxAttacks = Math.max(maxAttacks, 3);
      else if (level >= 5) maxAttacks = Math.max(maxAttacks, 2);
    } else if (martialClasses.some(mc => name.includes(mc))) {
      // Others: 2 attacks at 5
      if (level >= 5) maxAttacks = Math.max(maxAttacks, 2);
    }
  }

  return maxAttacks;
}

function spellPrepCount(c: {
  level_total: number;
  classes?: Array<{ name: string; level: number }>;
  abilities?: Record<string, number>;
}): number | null {
  // Wizards: Int mod + level
  // Clerics/Druids/Paladins: Wis mod + half level (rounded down)
  const wizardLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('wizard'))?.level ?? 0;
  const clericLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('cleric'))?.level ?? 0;
  const druidLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('druid'))?.level ?? 0;
  const paladinLevel = c.classes?.find(cl => cl.name.toLowerCase().includes('paladin'))?.level ?? 0;

  if (wizardLevel > 0) {
    const intMod = abilityMod(c.abilities?.int);
    return Math.max(1, intMod + wizardLevel);
  }

  if (clericLevel > 0 || druidLevel > 0) {
    const wisMod = abilityMod(c.abilities?.wis);
    const level = Math.max(clericLevel, druidLevel);
    return Math.max(1, wisMod + level);
  }

  if (paladinLevel > 0) {
    const chaMod = abilityMod(c.abilities?.cha);
    return Math.max(1, chaMod + Math.floor(paladinLevel / 2));
  }

  return null;
}

// =====================================================================
// Tests
// =====================================================================

describe('abilityMod', () => {
  it('returns -5 for score 1', () => {
    expect(abilityMod(1)).toBe(-5);
  });

  it('returns 0 for score 10-11', () => {
    expect(abilityMod(10)).toBe(0);
    expect(abilityMod(11)).toBe(0);
  });

  it('returns +1 for score 12-13', () => {
    expect(abilityMod(12)).toBe(1);
    expect(abilityMod(13)).toBe(1);
  });

  it('returns +3 for score 16-17', () => {
    expect(abilityMod(16)).toBe(3);
    expect(abilityMod(17)).toBe(3);
  });

  it('returns +5 for score 20', () => {
    expect(abilityMod(20)).toBe(5);
  });

  it('caps at +10 for score 30', () => {
    expect(abilityMod(30)).toBe(10);
  });

  it('returns -5 for undefined', () => {
    expect(abilityMod(undefined)).toBe(-5);
  });
});

describe('profBonus', () => {
  it('returns +2 for levels 1-4', () => {
    expect(profBonus(1)).toBe(2);
    expect(profBonus(4)).toBe(2);
  });

  it('returns +3 for levels 5-8', () => {
    expect(profBonus(5)).toBe(3);
    expect(profBonus(8)).toBe(3);
  });

  it('returns +4 for levels 9-12', () => {
    expect(profBonus(9)).toBe(4);
    expect(profBonus(12)).toBe(4);
  });

  it('returns +5 for levels 13-16', () => {
    expect(profBonus(13)).toBe(5);
    expect(profBonus(16)).toBe(5);
  });

  it('returns +6 for levels 17-20', () => {
    expect(profBonus(17)).toBe(6);
    expect(profBonus(20)).toBe(6);
  });
});

describe('cantripDiceMultiplier', () => {
  it('returns 1 for levels 1-4', () => {
    expect(cantripDiceMultiplier(1)).toBe(1);
    expect(cantripDiceMultiplier(4)).toBe(1);
  });

  it('returns 2 for levels 5-10', () => {
    expect(cantripDiceMultiplier(5)).toBe(2);
    expect(cantripDiceMultiplier(10)).toBe(2);
  });

  it('returns 3 for levels 11-16', () => {
    expect(cantripDiceMultiplier(11)).toBe(3);
    expect(cantripDiceMultiplier(16)).toBe(3);
  });

  it('returns 4 for levels 17+', () => {
    expect(cantripDiceMultiplier(17)).toBe(4);
    expect(cantripDiceMultiplier(20)).toBe(4);
  });
});

describe('sneakAttackDice', () => {
  it('returns 0 for non-rogue', () => {
    expect(sneakAttackDice({ level_total: 10, classes: [{ name: 'Fighter', level: 10 }] })).toBe(0);
  });

  it('returns 1d6 at rogue level 1', () => {
    expect(sneakAttackDice({ level_total: 1, classes: [{ name: 'Rogue', level: 1 }] })).toBe(1);
  });

  it('returns 3d6 at rogue level 5', () => {
    expect(sneakAttackDice({ level_total: 5, classes: [{ name: 'Rogue', level: 5 }] })).toBe(3);
  });

  it('returns 6d6 at rogue level 11', () => {
    expect(sneakAttackDice({ level_total: 11, classes: [{ name: 'Rogue', level: 11 }] })).toBe(6);
  });

  it('returns 10d6 at rogue level 20', () => {
    expect(sneakAttackDice({ level_total: 20, classes: [{ name: 'Rogue', level: 20 }] })).toBe(10);
  });
});

describe('martialArtsDie', () => {
  it('returns null for non-monk', () => {
    expect(martialArtsDie({ level_total: 10, classes: [{ name: 'Fighter', level: 10 }] })).toBeNull();
  });

  it('returns d4 at monk level 1-4', () => {
    expect(martialArtsDie({ level_total: 4, classes: [{ name: 'Monk', level: 4 }] })).toBe('d4');
  });

  it('returns d6 at monk level 5-10', () => {
    expect(martialArtsDie({ level_total: 5, classes: [{ name: 'Monk', level: 5 }] })).toBe('d6');
  });

  it('returns d8 at monk level 11-16', () => {
    expect(martialArtsDie({ level_total: 11, classes: [{ name: 'Monk', level: 11 }] })).toBe('d8');
  });

  it('returns d10 at monk level 17+', () => {
    expect(martialArtsDie({ level_total: 17, classes: [{ name: 'Monk', level: 17 }] })).toBe('d10');
  });
});

describe('extraAttackCount', () => {
  it('returns 1 for level 1-4 martial', () => {
    expect(extraAttackCount({ level_total: 4, classes: [{ name: 'Fighter', level: 4 }] })).toBe(1);
  });

  it('returns 2 for fighter level 5', () => {
    expect(extraAttackCount({ level_total: 5, classes: [{ name: 'Fighter', level: 5 }] })).toBe(2);
  });

  it('returns 2 for barbarian level 5', () => {
    expect(extraAttackCount({ level_total: 5, classes: [{ name: 'Barbarian', level: 5 }] })).toBe(2);
  });

  it('returns 3 for fighter level 11', () => {
    expect(extraAttackCount({ level_total: 11, classes: [{ name: 'Fighter', level: 11 }] })).toBe(3);
  });

  it('returns 4 for fighter level 20', () => {
    expect(extraAttackCount({ level_total: 20, classes: [{ name: 'Fighter', level: 20 }] })).toBe(4);
  });

  it('returns 1 for wizard level 20', () => {
    expect(extraAttackCount({ level_total: 20, classes: [{ name: 'Wizard', level: 20 }] })).toBe(1);
  });
});

describe('spellPrepCount', () => {
  it('returns null for non-preparer', () => {
    expect(spellPrepCount({ level_total: 10, classes: [{ name: 'Sorcerer', level: 10 }] })).toBeNull();
  });

  it('calculates wizard: Int mod + level', () => {
    const c = {
      level_total: 5,
      classes: [{ name: 'Wizard', level: 5 }],
      abilities: { int: 16 }
    };
    // Int 16 = +3 mod, + 5 level = 8
    expect(spellPrepCount(c)).toBe(8);
  });

  it('calculates cleric: Wis mod + level', () => {
    const c = {
      level_total: 5,
      classes: [{ name: 'Cleric', level: 5 }],
      abilities: { wis: 14 }
    };
    // Wis 14 = +2 mod, + 5 level = 7
    expect(spellPrepCount(c)).toBe(7);
  });

  it('calculates paladin: Cha mod + half level', () => {
    const c = {
      level_total: 6,
      classes: [{ name: 'Paladin', level: 6 }],
      abilities: { cha: 14 }
    };
    // Cha 14 = +2 mod, + floor(6/2) = 3 level = 5
    expect(spellPrepCount(c)).toBe(5);
  });

  it('minimum 1 prepared spell', () => {
    const c = {
      level_total: 1,
      classes: [{ name: 'Wizard', level: 1 }],
      abilities: { int: 8 } // -1 mod
    };
    expect(spellPrepCount(c)).toBe(1);
  });
});
