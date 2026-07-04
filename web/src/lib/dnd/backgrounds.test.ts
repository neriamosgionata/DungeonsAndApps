import { describe, it, expect } from 'vitest';
import { BACKGROUNDS, backgroundByKey, applyBackgroundMechanical } from '$lib/dnd/backgrounds';

describe('backgroundByKey', () => {
  it('finds Acolyte by key', () => {
    expect(backgroundByKey('acolyte')?.name).toBe('Acolyte');
    expect(backgroundByKey('acolyte')?.skills).toEqual(['insight', 'religion']);
  });
  it('finds Criminal by key', () => {
    expect(backgroundByKey('criminal')?.name).toBe('Criminal');
    expect(backgroundByKey('criminal')?.skills).toEqual(['deception', 'stealth']);
  });
  it('returns undefined for unknown key', () => {
    expect(backgroundByKey('does_not_exist')).toBeUndefined();
  });
  it('returns undefined for empty/null/undefined', () => {
    expect(backgroundByKey('')).toBeUndefined();
    expect(backgroundByKey(null)).toBeUndefined();
    expect(backgroundByKey(undefined)).toBeUndefined();
  });
});

describe('BACKGROUNDS completeness', () => {
  it('has at least 13 PHB backgrounds', () => {
    expect(BACKGROUNDS.length).toBeGreaterThanOrEqual(13);
  });
  it('every background has id + name + skills + languages', () => {
    for (const bg of BACKGROUNDS) {
      expect(typeof bg.key).toBe('string');
      expect(bg.key.length).toBeGreaterThan(0);
      expect(typeof bg.name).toBe('string');
      expect(bg.name.length).toBeGreaterThan(0);
      expect(Array.isArray(bg.skills)).toBe(true);
      expect(bg.skills.length).toBe(2);
      expect(typeof bg.languages).toBe('number');
      expect(bg.languages).toBeGreaterThanOrEqual(0);
      expect(bg.languages).toBeLessThanOrEqual(2);
    }
  });
  it('no duplicate background keys', () => {
    const keys = BACKGROUNDS.map((b) => b.key);
    const unique = new Set(keys);
    expect(unique.size).toBe(keys.length);
  });
});

describe('applyBackgroundMechanical', () => {
  it('adds 2 skill proficiencies to an empty sheet', () => {
    const bg = backgroundByKey('criminal')!;
    const result = applyBackgroundMechanical(bg, {});
    expect(result.skills.deception).toBe('prof');
    expect(result.skills.stealth).toBe('prof');
  });
  it('adds tool proficiency for backgrounds that grant one', () => {
    const bg = backgroundByKey('criminal')!;
    const result = applyBackgroundMechanical(bg, {});
    expect(result.tool_proficiencies.some((t) => t.name === "Thieves' tools")).toBe(true);
  });
  it('skips tool proficiency when background grants none (Sage)', () => {
    const bg = backgroundByKey('sage')!;
    const result = applyBackgroundMechanical(bg, {});
    expect(result.tool_proficiencies.length).toBe(0);
  });
  it('preserves existing expert proficiency when adding prof', () => {
    const bg = backgroundByKey('entertainer')!;
    const sheet = { skills: { acrobatics: 'expert' as const } };
    const result = applyBackgroundMechanical(bg, sheet);
    expect(result.skills.acrobatics).toBe('expert'); // not downgraded
    expect(result.skills.performance).toBe('prof');
  });
  it('preserves other unrelated skill proficiencies', () => {
    const bg = backgroundByKey('noble')!;
    const sheet = { skills: { athletics: 'expert' as const, arcana: 'prof' as const } };
    const result = applyBackgroundMechanical(bg, sheet);
    expect(result.skills.athletics).toBe('expert');
    expect(result.skills.arcana).toBe('prof');
    expect(result.skills.history).toBe('prof');
    expect(result.skills.persuasion).toBe('prof');
  });
  it('does not duplicate tool proficiency if already present', () => {
    const bg = backgroundByKey('entertainer')!;
    const sheet = { tool_proficiencies: [{ name: 'Disguise kit', proficient: true }] };
    const result = applyBackgroundMechanical(bg, sheet);
    const matching = result.tool_proficiencies.filter((t) => t.name === 'Disguise kit');
    expect(matching.length).toBe(1);
  });
});