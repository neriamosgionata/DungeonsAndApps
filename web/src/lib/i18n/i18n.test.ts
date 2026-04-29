import { describe, it, expect } from 'vitest';
import en from './en.json';
import itLocale from './it.json';

describe('i18n', () => {
  it('en has all nav keys', () => {
    for (const k of ['character','recap','map','npcs','factions','lore','news','spells','group','messages','dice','initiative']) {
      expect(en.nav).toHaveProperty(k);
    }
  });
  it('en and it have the same top-level keys', () => {
    expect(Object.keys(en).sort()).toEqual(Object.keys(itLocale).sort());
  });
  it('en and it nav keys match', () => {
    expect(Object.keys(en.nav).sort()).toEqual(Object.keys(itLocale.nav).sort());
  });
});
