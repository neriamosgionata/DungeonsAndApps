import { describe, it, expect } from 'vitest';
import { isCustomClass, subclassesFor, DND_CLASSES } from './classes';

describe('isCustomClass', () => {
  it('returns false for standard classes', () => {
    expect(isCustomClass('Fighter')).toBe(false);
    expect(isCustomClass('Wizard')).toBe(false);
    expect(isCustomClass('Blood Hunter')).toBe(false);
  });

  it('is case-insensitive', () => {
    expect(isCustomClass('wizard')).toBe(false);
    expect(isCustomClass('FIGHTER')).toBe(false);
    expect(isCustomClass('Cleric')).toBe(false);
  });

  it('handles whitespace', () => {
    expect(isCustomClass('  Wizard  ')).toBe(false);
  });

  it('returns true for custom classes', () => {
    expect(isCustomClass('Necromancer')).toBe(true);
    expect(isCustomClass('Witch')).toBe(true);
  });

  it('returns true for empty string (not in standard set)', () => {
    expect(isCustomClass('')).toBe(true);
  });
});

describe('subclassesFor', () => {
  it('returns subclasses for a known class', () => {
    const subs = subclassesFor('Fighter');
    expect(subs.length).toBeGreaterThan(0);
    expect(subs.some((s) => s.name === 'Champion')).toBe(true);
    expect(subs.some((s) => s.name === 'Battle Master')).toBe(true);
    expect(subs.some((s) => s.name === 'Eldritch Knight')).toBe(true);
  });

  it('is case-insensitive', () => {
    const upper = subclassesFor('FIGHTER');
    const lower = subclassesFor('fighter');
    expect(upper).toEqual(lower);
  });

  it('returns empty array for unknown class', () => {
    expect(subclassesFor('Necromancer')).toEqual([]);
  });

  it('returns empty array for null/undefined', () => {
    expect(subclassesFor(null)).toEqual([]);
    expect(subclassesFor(undefined)).toEqual([]);
  });

  it('returns empty array for empty string', () => {
    expect(subclassesFor('')).toEqual([]);
  });

  it('every DND_CLASS has subclass entries', () => {
    for (const cls of DND_CLASSES) {
      const subs = subclassesFor(cls);
      expect(subs.length).toBeGreaterThan(0);
    }
  });

  it('subclass entries have name and optional source', () => {
    const subs = subclassesFor('Wizard');
    for (const s of subs) {
      expect(typeof s.name).toBe('string');
      expect(s.name.length).toBeGreaterThan(0);
      if (s.source !== undefined) {
        expect(typeof s.source).toBe('string');
      }
    }
  });

  it('trims whitespace in class name', () => {
    expect(subclassesFor('  Rogue  ').length).toBeGreaterThan(0);
  });
});
