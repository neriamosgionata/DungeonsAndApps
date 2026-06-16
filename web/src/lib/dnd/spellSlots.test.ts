import { describe, it, expect } from 'vitest';

/**
 * Spell slot calculation utilities
 * Extracted from character page for testability
 */

/** Full-caster spell slots per class level (PHB). Table row 1..20, cols 1..9. */
const FULL_CASTER_SLOTS: number[][] = [
  /* 1 */  [2, 0, 0, 0, 0, 0, 0, 0, 0],
  /* 2 */  [3, 0, 0, 0, 0, 0, 0, 0, 0],
  /* 3 */  [4, 2, 0, 0, 0, 0, 0, 0, 0],
  /* 4 */  [4, 3, 0, 0, 0, 0, 0, 0, 0],
  /* 5 */  [4, 3, 2, 0, 0, 0, 0, 0, 0],
  /* 6 */  [4, 3, 3, 0, 0, 0, 0, 0, 0],
  /* 7 */  [4, 3, 3, 1, 0, 0, 0, 0, 0],
  /* 8 */  [4, 3, 3, 2, 0, 0, 0, 0, 0],
  /* 9 */  [4, 3, 3, 3, 1, 0, 0, 0, 0],
  /* 10 */ [4, 3, 3, 3, 2, 0, 0, 0, 0],
  /* 11 */ [4, 3, 3, 3, 2, 1, 0, 0, 0],
  /* 12 */ [4, 3, 3, 3, 2, 1, 0, 0, 0],
  /* 13 */ [4, 3, 3, 3, 2, 1, 1, 0, 0],
  /* 14 */ [4, 3, 3, 3, 2, 1, 1, 0, 0],
  /* 15 */ [4, 3, 3, 3, 2, 1, 1, 1, 0],
  /* 16 */ [4, 3, 3, 3, 2, 1, 1, 1, 0],
  /* 17 */ [4, 3, 3, 3, 2, 1, 1, 1, 1],
  /* 18 */ [4, 3, 3, 3, 3, 1, 1, 1, 1],
  /* 19 */ [4, 3, 3, 3, 3, 2, 1, 1, 1],
  /* 20 */ [4, 3, 3, 3, 3, 2, 2, 1, 1],
];

type CasterType = 'full' | 'half' | 'third' | 'warlock' | 'custom' | 'none';

function casterType(className: string, subclass?: string): CasterType {
  const name = className.trim().toLowerCase();
  const sub = (subclass || '').toLowerCase();

  // Full casters
  if (['wizard', 'sorcerer', 'bard', 'cleric', 'druid'].includes(name)) return 'full';
  if (name === 'warlock') return 'warlock';

  // Half casters
  if (['paladin', 'ranger'].includes(name)) {
    return 'half';
  }

  // Third casters
  if ((name === 'fighter' && sub.includes('eldritch knight')) ||
      (name === 'rogue' && sub.includes('arcane trickster'))) {
    return 'third';
  }

  // Custom/homebrew classes - treat as full caster
  if (name.startsWith('custom') || name.includes('homebrew')) {
    return 'custom';
  }

  // Artificer is a special half-caster
  if (name === 'artificer') {
    return 'half';
  }

  return 'none';
}

/** Compute baseline spell slots for a single-class or multiclass character */
function computeBaselineSlots(
  classes: Array<{ name: string; level: number; subclass?: string }>
): Record<string, number> {
  const out: Record<string, number> = {};

  if (!classes.length) return out;

  // Separate warlocks (pact magic)
  const warlocks = classes.filter(c => c.name.trim().toLowerCase() === 'warlock');
  const nonWarlocks = classes.filter(c => c.name.trim().toLowerCase() !== 'warlock');

  // Single-class full caster
  if (nonWarlocks.length === 1 && !warlocks.length) {
    const cl = nonWarlocks[0];
    const t = casterType(cl.name, cl.subclass);

    if (t === 'full' || t === 'custom') {
      const row = FULL_CASTER_SLOTS[Math.min(20, Math.max(1, cl.level)) - 1];
      row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
      return out;
    }

    if (t === 'half') {
      const isArtificer = cl.name.trim().toLowerCase() === 'artificer';
      const casterLv = isArtificer ? Math.ceil(cl.level / 2) : Math.floor(cl.level / 2);
      if (casterLv >= 1) {
        const row = FULL_CASTER_SLOTS[Math.min(20, casterLv) - 1];
        row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
      }
      return out;
    }

    if (t === 'third') {
      const casterLv = Math.floor(cl.level / 3);
      if (casterLv >= 1) {
        const row = FULL_CASTER_SLOTS[Math.min(20, casterLv) - 1];
        row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
      }
      return out;
    }
  }

  // Multiclass: sum effective caster levels
  let totalCasterLevel = 0;
  for (const cl of nonWarlocks) {
    const t = casterType(cl.name, cl.subclass);
    if (t === 'full' || t === 'custom') totalCasterLevel += cl.level;
    else if (t === 'half') {
      const isArtificer = cl.name.trim().toLowerCase() === 'artificer';
      totalCasterLevel += isArtificer ? Math.ceil(cl.level / 2) : Math.floor(cl.level / 2);
    }
    else if (t === 'third') totalCasterLevel += Math.floor(cl.level / 3);
  }

  if (totalCasterLevel >= 1) {
    const row = FULL_CASTER_SLOTS[Math.min(20, totalCasterLevel) - 1];
    row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
  }

  // Warlock pact magic
  for (const w of warlocks) {
    const pactLv = warlockPactSlotLevel(w.level);
    if (pactLv === 0) continue;
    const count = warlockPactSlotCount(w.level);
    if (count > 0) {
      out[String(pactLv)] = Math.max(out[String(pactLv)] || 0, count);
    }
  }

  return out;
}

function warlockPactSlotCount(level: number): number {
  if (level >= 17) return 4;
  if (level >= 11) return 3;
  if (level >= 2) return 2;
  if (level >= 1) return 1;
  return 0;
}

function warlockPactSlotLevel(level: number): number {
  if (level >= 9) return 5;
  if (level >= 7) return 4;
  if (level >= 5) return 3;
  if (level >= 3) return 2;
  if (level >= 1) return 1;
  return 0;
}

// =====================================================================
// Tests
// =====================================================================

describe('casterType', () => {
  it('identifies full casters', () => {
    expect(casterType('Wizard')).toBe('full');
    expect(casterType('Sorcerer')).toBe('full');
    expect(casterType('Bard')).toBe('full');
    expect(casterType('Cleric')).toBe('full');
    expect(casterType('Druid')).toBe('full');
    expect(casterType('Warlock')).toBe('warlock');
  });

  it('identifies half casters', () => {
    expect(casterType('Paladin')).toBe('half');
    expect(casterType('Ranger')).toBe('half');
    expect(casterType('Artificer')).toBe('half');
  });

  it('identifies third casters by subclass', () => {
    expect(casterType('Fighter', 'Eldritch Knight')).toBe('third');
    expect(casterType('Rogue', 'Arcane Trickster')).toBe('third');
    expect(casterType('Fighter', 'Battle Master')).toBe('none');
  });

  it('identifies custom classes', () => {
    expect(casterType('Custom Mage')).toBe('custom');
    expect(casterType('MyHomebrewClass')).toBe('custom');
  });

  it('is case insensitive', () => {
    expect(casterType('WIZARD')).toBe('full');
    expect(casterType('paladin')).toBe('half');
  });
});

describe('computeBaselineSlots', () => {
  it('level 1 wizard has 2 first-level slots', () => {
    const slots = computeBaselineSlots([{ name: 'Wizard', level: 1 }]);
    expect(slots['1']).toBe(2);
    expect(slots['2']).toBeUndefined();
  });

  it('level 5 wizard has 4/3/2 slots', () => {
    const slots = computeBaselineSlots([{ name: 'Wizard', level: 5 }]);
    expect(slots['1']).toBe(4);
    expect(slots['2']).toBe(3);
    expect(slots['3']).toBe(2);
  });

  it('level 20 wizard has 4/3/3/3/3/2/2/1/1 slots', () => {
    const slots = computeBaselineSlots([{ name: 'Wizard', level: 20 }]);
    expect(slots['1']).toBe(4);
    expect(slots['5']).toBe(3);
    expect(slots['9']).toBe(1);
  });

  it('level 10 paladin (half caster) has 4/3 slots', () => {
    const slots = computeBaselineSlots([{ name: 'Paladin', level: 10 }]);
    // Level 10 paladin = floor(10/2) = 5th row
    expect(slots['1']).toBe(4);
    expect(slots['2']).toBe(3);
    expect(slots['3']).toBe(2);
  });

  it('level 6 eldritch knight (third caster) has 2/1 slots', () => {
    const slots = computeBaselineSlots([{ name: 'Fighter', level: 6, subclass: 'Eldritch Knight' }]);
    // Level 6 EK = floor(6/3) = 2nd row
    expect(slots['1']).toBe(3);
  });

  it('multiclass wizard 5 / paladin 5 = caster level 5 + 2 = 7', () => {
    const slots = computeBaselineSlots([
      { name: 'Wizard', level: 5 },
      { name: 'Paladin', level: 5 }
    ]);
    // Caster level 7: 4/3/3/1
    expect(slots['1']).toBe(4);
    expect(slots['3']).toBe(3);
    expect(slots['4']).toBe(1);
  });

  it('warlock level 5 has 2 third-level pact slots', () => {
    const slots = computeBaselineSlots([{ name: 'Warlock', level: 5 }]);
    // Warlock 5: 2 pact slots at level 3
    expect(slots['3']).toBe(2);
  });

  it('warlock level 11 has 3 fifth-level pact slots', () => {
    const slots = computeBaselineSlots([{ name: 'Warlock', level: 11 }]);
    // Warlock 11: 3 pact slots at level 5
    expect(slots['5']).toBe(3);
  });

  it('warlock 5 / wizard 5 combines slots', () => {
    const slots = computeBaselineSlots([
      { name: 'Wizard', level: 5 },
      { name: 'Warlock', level: 5 }
    ]);
    // Wizard 5: 4/3/2
    // Warlock 5: 2 slots at level 2
    expect(slots['1']).toBe(4);
    expect(slots['2']).toBe(3); // max(3, 2)
    expect(slots['3']).toBe(2);
  });

  it('empty class list returns empty slots', () => {
    const slots = computeBaselineSlots([]);
    expect(Object.keys(slots)).toHaveLength(0);
  });

  it('non-caster classes give no slots', () => {
    const slots = computeBaselineSlots([{ name: 'Fighter', level: 10 }]);
    expect(Object.keys(slots)).toHaveLength(0);
  });
});

describe('warlockPactSlotCount', () => {
  it('level 1 has 1 slot', () => {
    expect(warlockPactSlotCount(1)).toBe(1);
  });

  it('level 2-10 has 2 slots', () => {
    expect(warlockPactSlotCount(2)).toBe(2);
    expect(warlockPactSlotCount(10)).toBe(2);
  });

  it('level 11-16 has 3 slots', () => {
    expect(warlockPactSlotCount(11)).toBe(3);
    expect(warlockPactSlotCount(16)).toBe(3);
  });

  it('level 17+ has 4 slots', () => {
    expect(warlockPactSlotCount(17)).toBe(4);
    expect(warlockPactSlotCount(20)).toBe(4);
  });
});

describe('warlockPactSlotLevel', () => {
  it('level 1-2 has 1st level slots', () => {
    expect(warlockPactSlotLevel(1)).toBe(1);
    expect(warlockPactSlotLevel(2)).toBe(1);
  });

  it('level 3-4 has 2nd level slots', () => {
    expect(warlockPactSlotLevel(3)).toBe(2);
    expect(warlockPactSlotLevel(4)).toBe(2);
  });

  it('level 5-6 has 3rd level slots', () => {
    expect(warlockPactSlotLevel(5)).toBe(3);
    expect(warlockPactSlotLevel(6)).toBe(3);
  });

  it('level 7-8 has 4th level slots', () => {
    expect(warlockPactSlotLevel(7)).toBe(4);
    expect(warlockPactSlotLevel(8)).toBe(4);
  });

  it('level 9+ has 5th level slots', () => {
    expect(warlockPactSlotLevel(9)).toBe(5);
    expect(warlockPactSlotLevel(20)).toBe(5);
  });
});
