import { describe, it, expect } from 'vitest';

/**
 * Dice parsing utilities
 */

interface ParsedDice {
  count: number;
  sides: number;
  modifier: number;
}

function parseDiceExpression(expr: string): ParsedDice | null {
  // Match patterns like: "2d6", "1d8+3", "2d10-1", "d20" (defaults to 1d20)
  const match = expr.trim().toLowerCase().match(/^(\d*)d(\d+)(?:\s*([+-])\s*(\d+))?$/);
  if (!match) return null;

  const count = match[1] ? parseInt(match[1], 10) : 1;
  const sides = parseInt(match[2], 10);
  const modifier = match[3] && match[4]
    ? (match[3] === '+' ? 1 : -1) * parseInt(match[4], 10)
    : 0;

  if (count < 1 || sides < 1) return null;

  return { count, sides, modifier };
}

function averageRoll(parsed: ParsedDice): number {
  // Average of NdX = N * (X+1)/2 + modifier
  return parsed.count * (parsed.sides + 1) / 2 + parsed.modifier;
}

function minRoll(parsed: ParsedDice): number {
  return parsed.count * 1 + parsed.modifier;
}

function maxRoll(parsed: ParsedDice): number {
  return parsed.count * parsed.sides + parsed.modifier;
}

function formatDiceResult(rolls: number[], modifier: number): string {
  const sum = rolls.reduce((a, b) => a + b, 0) + modifier;
  const rollStr = rolls.join(' + ');
  if (modifier > 0) return `${rollStr} + ${modifier} = ${sum}`;
  if (modifier < 0) return `${rollStr} - ${Math.abs(modifier)} = ${sum}`;
  return `${rollStr} = ${sum}`;
}

// =====================================================================
// HP Calculation utilities
// =====================================================================

interface HPResult {
  max: number;
  current: number;
  temp: number;
  effective: number; // current + temp, but temp doesn't heal
}

function calculateHP(
  baseMax: number,
  conMod: number,
  level: number,
  currentDamage: number = 0,
  tempHP: number = 0,
  maxReduction: number = 0
): HPResult {
  // Simple formula: base + (conMod * level), min 1 total
  // This matches test expectations
  const rawMax = baseMax + (conMod * level);
  const max = Math.max(1, rawMax - maxReduction);
  const current = Math.max(0, max - currentDamage);
  const effective = current + tempHP;

  return { max, current, temp: tempHP, effective };
}

function applyDamage(hp: HPResult, damage: number): HPResult {
  // Temp HP absorbs damage first
  const remainingAfterTemp = Math.max(0, damage - hp.temp);
  const newTemp = Math.max(0, hp.temp - damage);
  const newCurrent = Math.max(0, hp.current - remainingAfterTemp);
  const damageTaken = hp.current - newCurrent;

  return {
    max: hp.max,
    current: newCurrent,
    temp: newTemp,
    effective: newCurrent + newTemp
  };
}

function applyHealing(hp: HPResult, healing: number): HPResult {
  // Healing doesn't restore temp HP
  const newCurrent = Math.min(hp.max, hp.current + healing);

  return {
    max: hp.max,
    current: newCurrent,
    temp: hp.temp,
    effective: newCurrent + hp.temp
  };
}

// =====================================================================
// Tests
// =====================================================================

describe('parseDiceExpression', () => {
  it('parses simple "2d6"', () => {
    const result = parseDiceExpression('2d6');
    expect(result).toEqual({ count: 2, sides: 6, modifier: 0 });
  });

  it('parses "d20" as 1d20', () => {
    const result = parseDiceExpression('d20');
    expect(result).toEqual({ count: 1, sides: 20, modifier: 0 });
  });

  it('parses "1d8+3" with bonus', () => {
    const result = parseDiceExpression('1d8+3');
    expect(result).toEqual({ count: 1, sides: 8, modifier: 3 });
  });

  it('parses "2d10-1" with penalty', () => {
    const result = parseDiceExpression('2d10-1');
    expect(result).toEqual({ count: 2, sides: 10, modifier: -1 });
  });

  it('handles whitespace', () => {
    const result = parseDiceExpression('  3d4  +  2  ');
    expect(result).toEqual({ count: 3, sides: 4, modifier: 2 });
  });

  it('is case insensitive', () => {
    const result = parseDiceExpression('1D6+2');
    expect(result).toEqual({ count: 1, sides: 6, modifier: 2 });
  });

  it('returns null for invalid expressions', () => {
    expect(parseDiceExpression('not dice')).toBeNull();
    expect(parseDiceExpression('')).toBeNull();
    expect(parseDiceExpression('2d')).toBeNull();
    expect(parseDiceExpression('d')).toBeNull();
  });

  it('returns null for zero/negative values', () => {
    expect(parseDiceExpression('0d6')).toBeNull();
    expect(parseDiceExpression('1d0')).toBeNull();
  });
});

describe('averageRoll', () => {
  it('2d6 averages 7', () => {
    expect(averageRoll({ count: 2, sides: 6, modifier: 0 })).toBe(7);
  });

  it('1d8+3 averages 7.5', () => {
    expect(averageRoll({ count: 1, sides: 8, modifier: 3 })).toBe(7.5);
  });

  it('1d20 averages 10.5', () => {
    expect(averageRoll({ count: 1, sides: 20, modifier: 0 })).toBe(10.5);
  });
});

describe('minRoll', () => {
  it('2d6 min is 2', () => {
    expect(minRoll({ count: 2, sides: 6, modifier: 0 })).toBe(2);
  });

  it('1d8+3 min is 4', () => {
    expect(minRoll({ count: 1, sides: 8, modifier: 3 })).toBe(4);
  });
});

describe('maxRoll', () => {
  it('2d6 max is 12', () => {
    expect(maxRoll({ count: 2, sides: 6, modifier: 0 })).toBe(12);
  });

  it('1d8+3 max is 11', () => {
    expect(maxRoll({ count: 1, sides: 8, modifier: 3 })).toBe(11);
  });
});

describe('formatDiceResult', () => {
  it('formats rolls without modifier', () => {
    expect(formatDiceResult([3, 4], 0)).toBe('3 + 4 = 7');
  });

  it('formats rolls with positive modifier', () => {
    expect(formatDiceResult([5], 3)).toBe('5 + 3 = 8');
  });

  it('formats rolls with negative modifier', () => {
    expect(formatDiceResult([6], -2)).toBe('6 - 2 = 4');
  });

  it('handles single roll', () => {
    expect(formatDiceResult([10], 0)).toBe('10 = 10');
  });
});

describe('calculateHP', () => {
  it('calculates basic HP for level 1', () => {
    const hp = calculateHP(10, 2, 1); // 10 base, +2 con, level 1
    expect(hp.max).toBe(12);
    expect(hp.current).toBe(12);
    expect(hp.effective).toBe(12);
  });

  it('scales with level and con mod', () => {
    const hp = calculateHP(10, 2, 5); // +2 con per level
    expect(hp.max).toBe(20); // 10 + (2*5) = 20
  });

  it('handles negative con mod', () => {
    const hp = calculateHP(12, -1, 3); // 12 base, -1 con mod * 3
    expect(hp.max).toBe(9); // 12 - 3 = 9
  });

  it('applies current damage', () => {
    const hp = calculateHP(20, 2, 1, 5); // 5 damage taken
    expect(hp.max).toBe(22); // 20 + 2
    expect(hp.current).toBe(17); // 22 - 5
  });

  it('applies temp HP', () => {
    const hp = calculateHP(18, 2, 1, 0, 5); // 5 temp HP
    expect(hp.current).toBe(20); // 18 + 2
    expect(hp.temp).toBe(5);
    expect(hp.effective).toBe(25); // 20 + 5
  });

  it('handles max reduction', () => {
    const hp = calculateHP(10, 2, 5, 0, 0, 5); // max reduced by 5
    expect(hp.max).toBe(15); // (10 + 2*5) - 5 = 15
  });

  it('minimum max HP is 1', () => {
    const hp = calculateHP(1, -5, 1); // Would be negative
    expect(hp.max).toBe(1);
  });
});

describe('applyDamage', () => {
  it('reduces current HP', () => {
    const before = calculateHP(20, 0, 1);
    const after = applyDamage(before, 5);
    expect(after.current).toBe(15);
    expect(after.temp).toBe(0);
  });

  it('absorbs with temp HP first', () => {
    const before = calculateHP(20, 0, 1, 0, 5);
    const after = applyDamage(before, 3);
    expect(after.temp).toBe(2);
    expect(after.current).toBe(20);
  });

  it('temp and current both absorb if damage exceeds temp', () => {
    const before = calculateHP(20, 0, 1, 0, 5);
    const after = applyDamage(before, 8);
    expect(after.temp).toBe(0);
    expect(after.current).toBe(17); // 20 - (8-5)
  });

  it('does not go below 0 current', () => {
    const before = calculateHP(20, 0, 1);
    const after = applyDamage(before, 50);
    expect(after.current).toBe(0);
  });
});

describe('applyHealing', () => {
  it('increases current HP', () => {
    const before = calculateHP(20, 0, 1, 5); // 5 damage
    const after = applyHealing(before, 3);
    expect(after.current).toBe(18); // 15 + 3
  });

  it('does not exceed max', () => {
    const before = calculateHP(20, 0, 1, 5);
    const after = applyHealing(before, 10);
    expect(after.current).toBe(20); // capped at max
  });

  it('does not restore temp HP', () => {
    const before = calculateHP(20, 0, 1, 0, 5);
    const after = applyHealing(before, 5);
    expect(after.temp).toBe(5); // unchanged
  });
});
