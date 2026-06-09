import { describe, it, expect } from 'vitest';
import { itemBySlug, itemsByCategory, ITEMS } from './items';

describe('itemBySlug', () => {
  it('finds existing item by slug', () => {
    const item = itemBySlug('longsword');
    expect(item).toBeDefined();
    expect(item!.name).toBe('Longsword');
    expect(item!.category).toBe('weapon');
    expect(item!.damage_die).toBe('1d8');
  });

  it('finds armor items', () => {
    const item = itemBySlug('plate');
    expect(item).toBeDefined();
    expect(item!.category).toBe('armor');
    expect(item!.armor_type).toBe('heavy');
    expect(item!.ac_base).toBe(18);
  });

  it('finds adventuring gear', () => {
    const item = itemBySlug('backpack');
    expect(item).toBeDefined();
    expect(item!.category).toBe('adventuring_gear');
  });

  it('returns undefined for unknown slug', () => {
    expect(itemBySlug('lightsaber')).toBeUndefined();
  });

  it('returns undefined for empty string', () => {
    expect(itemBySlug('')).toBeUndefined();
  });

  it('is case-sensitive (slugs are lowercase)', () => {
    expect(itemBySlug('Longsword')).toBeUndefined();
    expect(itemBySlug('LONGSWORD')).toBeUndefined();
  });
});

describe('itemsByCategory', () => {
  it('filters by weapon category', () => {
    const weapons = itemsByCategory('weapon');
    expect(weapons.length).toBeGreaterThan(0);
    for (const w of weapons) {
      expect(w.category).toBe('weapon');
    }
  });

  it('filters by armor category', () => {
    const armors = itemsByCategory('armor');
    expect(armors.length).toBeGreaterThan(0);
    expect(armors.some((a) => a.slug === 'leather')).toBe(true);
    expect(armors.some((a) => a.slug === 'plate')).toBe(true);
  });

  it('returns empty array for pack category', () => {
    const packs = itemsByCategory('pack');
    expect(packs).toEqual([]);
  });

  it('returns all items from category, no cross-contamination', () => {
    const shields = itemsByCategory('shield');
    expect(shields.length).toBe(1);
    expect(shields[0].slug).toBe('shield');
  });
});

describe('ITEMS array integrity', () => {
  it('all items have unique slugs', () => {
    const slugs = ITEMS.map((i) => i.slug);
    expect(new Set(slugs).size).toBe(slugs.length);
  });

  it('all weapons have damage_die', () => {
    const weapons = ITEMS.filter((i) => i.category === 'weapon');
    expect(weapons.length).toBeGreaterThan(0);
    for (const w of weapons) {
      expect(w.damage_die).toBeDefined();
      expect(typeof w.damage_die).toBe('string');
    }
  });

  it('all armors have ac_base and armor_type', () => {
    const armors = ITEMS.filter((i) => i.category === 'armor');
    expect(armors.length).toBeGreaterThan(0);
    for (const a of armors) {
      expect(a.ac_base).toBeDefined();
      expect(a.armor_type).toBeDefined();
    }
  });

  it('all items have name, slug, category, weight_lb', () => {
    for (const item of ITEMS) {
      expect(item.name).toBeTruthy();
      expect(item.slug).toBeTruthy();
      expect(item.category).toBeTruthy();
      expect(typeof item.weight_lb).toBe('number');
      expect(item.weight_lb).toBeGreaterThanOrEqual(0);
    }
  });
});
