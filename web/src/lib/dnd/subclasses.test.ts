import { describe, it, expect } from 'vitest';
import { getClassDef, getSubclassFeatures, getBaseFeatures, listSubclasses, ALL_CLASS_NAMES } from './subclasses';

describe('getClassDef', () => {
  it('returns ClassDef for known class', () => {
    const def = getClassDef('Fighter');
    expect(def).toBeDefined();
    expect(def!.base.length).toBeGreaterThan(0);
    expect(Object.keys(def!.subclasses).length).toBeGreaterThan(0);
  });

  it('is case-insensitive', () => {
    expect(getClassDef('wizard')).toBeDefined();
    expect(getClassDef('WIZARD')).toBeDefined();
  });

  it('returns undefined for unknown class', () => {
    expect(getClassDef('Necromancer')).toBeUndefined();
  });

  it('returns undefined for empty string', () => {
    expect(getClassDef('')).toBeUndefined();
  });
});

describe('getSubclassFeatures', () => {
  it('returns features for known subclass', () => {
    const features = getSubclassFeatures('Fighter', 'Champion');
    expect(features.length).toBeGreaterThan(0);
    expect(features.some((f) => f.name.includes('Improved Critical'))).toBe(true);
  });

  it('returns features for subclass with level-specific features', () => {
    const features = getSubclassFeatures('Rogue', 'Thief');
    expect(features.length).toBeGreaterThan(0);
    const levels = features.map((f) => f.level);
    expect(levels).toContain(3);
    expect(levels).toContain(9);
    expect(levels).toContain(13);
    expect(levels).toContain(17);
  });

  it('is case-insensitive for class and subclass', () => {
    const a = getSubclassFeatures('fighter', 'champion');
    const b = getSubclassFeatures('FIGHTER', 'CHAMPION');
    expect(a).toEqual(b);
    expect(a.length).toBeGreaterThan(0);
  });

  it('returns empty array for unknown class', () => {
    expect(getSubclassFeatures('Necromancer', 'Anything')).toEqual([]);
  });

  it('returns empty array for unknown subclass', () => {
    expect(getSubclassFeatures('Fighter', 'MadeUpSubclass')).toEqual([]);
  });

  it('each feature has required properties', () => {
    const allNames = ALL_CLASS_NAMES;
    for (const className of allNames) {
      const def = getClassDef(className)!;
      const subNames = Object.keys(def.subclasses);
      if (subNames.length === 0) continue;
      const features = getSubclassFeatures(className, subNames[0]);
      for (const f of features) {
        expect(typeof f.name).toBe('string');
        expect(typeof f.level).toBe('number');
        expect(f.level).toBeGreaterThanOrEqual(1);
        expect(f.level).toBeLessThanOrEqual(20);
        expect(typeof f.description).toBe('string');
      }
    }
  });
});

describe('getBaseFeatures', () => {
  it('returns base features for known class', () => {
    const features = getBaseFeatures('Fighter');
    expect(features.length).toBeGreaterThan(0);
  });

  it('returns empty array for unknown class', () => {
    expect(getBaseFeatures('Necromancer')).toEqual([]);
  });

  it('every class in ALL_CLASS_NAMES has base features', () => {
    for (const className of ALL_CLASS_NAMES) {
      const features = getBaseFeatures(className);
      expect(features.length).toBeGreaterThan(0);
    }
  });
});

describe('listSubclasses', () => {
  it('returns subclass names for known class', () => {
    const subs = listSubclasses('Wizard');
    expect(subs.length).toBeGreaterThan(0);
    expect(subs).toContain('School of Abjuration');
    expect(subs).toContain('School of Divination');
  });

  it('is case-insensitive', () => {
    const a = listSubclasses('wizard');
    const b = listSubclasses('WIZARD');
    expect(a).toEqual(b);
  });

  it('returns empty array for unknown class', () => {
    expect(listSubclasses('Necromancer')).toEqual([]);
  });
});

describe('ALL_CLASS_NAMES', () => {
  it('contains all standard 5e classes', () => {
    expect(ALL_CLASS_NAMES).toContain('Barbarian');
    expect(ALL_CLASS_NAMES).toContain('Bard');
    expect(ALL_CLASS_NAMES).toContain('Cleric');
    expect(ALL_CLASS_NAMES).toContain('Druid');
    expect(ALL_CLASS_NAMES).toContain('Fighter');
    expect(ALL_CLASS_NAMES).toContain('Monk');
    expect(ALL_CLASS_NAMES).toContain('Paladin');
    expect(ALL_CLASS_NAMES).toContain('Ranger');
    expect(ALL_CLASS_NAMES).toContain('Rogue');
    expect(ALL_CLASS_NAMES).toContain('Sorcerer');
    expect(ALL_CLASS_NAMES).toContain('Warlock');
    expect(ALL_CLASS_NAMES).toContain('Wizard');
  });
});
