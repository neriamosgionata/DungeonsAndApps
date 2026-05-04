import { describe, it, expect } from 'vitest';
import { RESOURCES_BY_CLASS, templatesForClass } from './resources';

describe('templatesForClass', () => {
  it('returns templates for Barbarian', () => {
    const t = templatesForClass('Barbarian');
    expect(t.length).toBeGreaterThan(0);
    expect(t.some((r) => r.name === 'Rages')).toBe(true);
  });

  it('returns templates for Wizard', () => {
    const t = templatesForClass('Wizard');
    expect(t.some((r) => r.name === 'Arcane Recovery')).toBe(true);
  });

  it('returns templates for Fighter', () => {
    const t = templatesForClass('Fighter');
    expect(t.some((r) => r.name === 'Second Wind')).toBe(true);
  });

  it('returns templates for Warlock', () => {
    const t = templatesForClass('Warlock');
    expect(t.some((r) => r.name === 'Eldritch Invocations')).toBe(true);
  });

  it('returns templates for Monk', () => {
    const t = templatesForClass('Monk');
    expect(t.some((r) => r.name === 'Ki')).toBe(true);
  });

  it('returns templates for Paladin', () => {
    const t = templatesForClass('Paladin');
    expect(t.some((r) => r.name === 'Lay on Hands Pool')).toBe(true);
  });

  it('returns templates for Sorcerer', () => {
    const t = templatesForClass('Sorcerer');
    expect(t.some((r) => r.name === 'Sorcery Points')).toBe(true);
  });

  it('returns templates for Cleric', () => {
    const t = templatesForClass('Cleric');
    expect(t.some((r) => r.name === 'Channel Divinity')).toBe(true);
  });

  it('returns templates for Druid', () => {
    const t = templatesForClass('Druid');
    expect(t.some((r) => r.name === 'Wild Shape')).toBe(true);
  });

  it('returns templates for Rogue', () => {
    const t = templatesForClass('Rogue');
    expect(t.some((r) => r.name === 'Stroke of Luck')).toBe(true);
  });

  it('returns empty array for unknown class', () => {
    expect(templatesForClass('Necromancer')).toHaveLength(0);
  });

  it('returns empty array for empty string', () => {
    expect(templatesForClass('')).toHaveLength(0);
  });

  it('is case-insensitive — barbarian matches Barbarian', () => {
    expect(templatesForClass('barbarian').length).toBe(templatesForClass('Barbarian').length);
  });

  it('is case-insensitive — WIZARD matches Wizard', () => {
    expect(templatesForClass('WIZARD').length).toBe(templatesForClass('Wizard').length);
  });

  it('is case-insensitive — blood hunter matches Blood Hunter', () => {
    const t = templatesForClass('blood hunter');
    expect(t.length).toBeGreaterThan(0);
  });

  it('trims whitespace', () => {
    const t = templatesForClass('  Bard  ');
    expect(t.some((r) => r.name === 'Bardic Inspiration')).toBe(true);
  });
});

describe('Barbarian rages by level', () => {
  const rages = RESOURCES_BY_CLASS['Barbarian'].find((r) => r.name === 'Rages')!;

  it('has 2 rages at level 1', () => {
    expect(rages.maxFor(1)).toBe(2);
  });

  it('has 3 rages at level 3', () => {
    expect(rages.maxFor(3)).toBe(3);
  });

  it('has 4 rages at level 6', () => {
    expect(rages.maxFor(6)).toBe(4);
  });

  it('has 5 rages at level 12', () => {
    expect(rages.maxFor(12)).toBe(5);
  });

  it('has 6 rages at level 17', () => {
    expect(rages.maxFor(17)).toBe(6);
  });

  it('has 99 (unlimited) rages at level 20', () => {
    expect(rages.maxFor(20)).toBe(99);
  });

  it('has 0 rages at level 0', () => {
    expect(rages.maxFor(0)).toBe(0);
  });
});

describe('Bard', () => {
  it('bardic inspiration is seeded', () => {
    const t = templatesForClass('Bard');
    const bi = t.find((r) => r.name === 'Bardic Inspiration');
    expect(bi).toBeDefined();
  });

  it('bardic inspiration resets on short', () => {
    const bi = RESOURCES_BY_CLASS['Bard'].find((r) => r.name === 'Bardic Inspiration')!;
    expect(bi.reset).toBe('short');
  });

  it('bardic inspiration returns at least 1 at level 1', () => {
    const bi = RESOURCES_BY_CLASS['Bard'].find((r) => r.name === 'Bardic Inspiration')!;
    expect(bi.maxFor(1)).toBeGreaterThanOrEqual(1);
  });
});

describe('Fighter', () => {
  const fighter = RESOURCES_BY_CLASS['Fighter'];

  it('has Second Wind at level 1', () => {
    const sw = fighter.find((r) => r.name === 'Second Wind')!;
    expect(sw.maxFor(1)).toBe(1);
  });

  it('Second Wind resets on short', () => {
    const sw = fighter.find((r) => r.name === 'Second Wind')!;
    expect(sw.reset).toBe('short');
  });

  it('Action Surge has minLevel 2', () => {
    const as = fighter.find((r) => r.name === 'Action Surge')!;
    expect(as.minLevel).toBe(2);
  });

  it('Action Surge = 0 at level 1', () => {
    const as = fighter.find((r) => r.name === 'Action Surge')!;
    expect(as.maxFor(1)).toBe(0);
  });

  it('Action Surge = 1 at level 2', () => {
    const as = fighter.find((r) => r.name === 'Action Surge')!;
    expect(as.maxFor(2)).toBe(1);
  });

  it('Action Surge = 2 at level 17', () => {
    const as = fighter.find((r) => r.name === 'Action Surge')!;
    expect(as.maxFor(17)).toBe(2);
  });

  it('Indomitable has minLevel 9', () => {
    const ind = fighter.find((r) => r.name === 'Indomitable')!;
    expect(ind.minLevel).toBe(9);
  });

  it('Indomitable = 0 at level 8', () => {
    const ind = fighter.find((r) => r.name === 'Indomitable')!;
    expect(ind.maxFor(8)).toBe(0);
  });

  it('Indomitable = 1 at level 9', () => {
    const ind = fighter.find((r) => r.name === 'Indomitable')!;
    expect(ind.maxFor(9)).toBe(1);
  });

  it('Indomitable = 2 at level 13', () => {
    const ind = fighter.find((r) => r.name === 'Indomitable')!;
    expect(ind.maxFor(13)).toBe(2);
  });

  it('Indomitable = 3 at level 17', () => {
    const ind = fighter.find((r) => r.name === 'Indomitable')!;
    expect(ind.maxFor(17)).toBe(3);
  });
});

describe('Monk', () => {
  const monk = RESOURCES_BY_CLASS['Monk'];

  it('Ki has minLevel 2', () => {
    const ki = monk.find((r) => r.name === 'Ki')!;
    expect(ki.minLevel).toBe(2);
  });

  it('Ki = 0 at level 1', () => {
    const ki = monk.find((r) => r.name === 'Ki')!;
    expect(ki.maxFor(1)).toBe(0);
  });

  it('Ki = level at levels 2–19', () => {
    const ki = monk.find((r) => r.name === 'Ki')!;
    for (const L of [2, 5, 10, 15, 19]) {
      expect(ki.maxFor(L)).toBe(L);
    }
  });

  it('Perfect Self has minLevel 20', () => {
    const ps = monk.find((r) => r.name === 'Perfect Self')!;
    expect(ps.minLevel).toBe(20);
  });

  it('Perfect Self = 0 below level 20', () => {
    const ps = monk.find((r) => r.name === 'Perfect Self')!;
    expect(ps.maxFor(19)).toBe(0);
  });

  it('Perfect Self = 4 at level 20', () => {
    const ps = monk.find((r) => r.name === 'Perfect Self')!;
    expect(ps.maxFor(20)).toBe(4);
  });
});

describe('Warlock', () => {
  const warlock = RESOURCES_BY_CLASS['Warlock'];
  const ei = warlock.find((r) => r.name === 'Eldritch Invocations')!;

  it('Eldritch Invocations = 0 at level 1', () => {
    expect(ei.maxFor(1)).toBe(0);
  });

  it('Eldritch Invocations = 2 at level 2', () => {
    expect(ei.maxFor(2)).toBe(2);
  });

  it('Eldritch Invocations = 3 at level 5', () => {
    expect(ei.maxFor(5)).toBe(3);
  });

  it('Eldritch Invocations = 4 at level 7', () => {
    expect(ei.maxFor(7)).toBe(4);
  });

  it('Eldritch Invocations = 5 at level 9', () => {
    expect(ei.maxFor(9)).toBe(5);
  });

  it('Eldritch Invocations = 6 at level 12', () => {
    expect(ei.maxFor(12)).toBe(6);
  });

  it('Eldritch Invocations = 7 at level 15', () => {
    expect(ei.maxFor(15)).toBe(7);
  });

  it('Eldritch Invocations = 8 at level 18', () => {
    expect(ei.maxFor(18)).toBe(8);
  });

  it('Mystic Arcanum 6th = 0 at level 10', () => {
    const ma6 = warlock.find((r) => r.name === 'Mystic Arcanum (6th)')!;
    expect(ma6.maxFor(10)).toBe(0);
  });

  it('Mystic Arcanum 6th = 1 at level 11', () => {
    const ma6 = warlock.find((r) => r.name === 'Mystic Arcanum (6th)')!;
    expect(ma6.maxFor(11)).toBe(1);
  });

  it('Mystic Arcanum 7th = 0 at level 12', () => {
    const ma7 = warlock.find((r) => r.name === 'Mystic Arcanum (7th)')!;
    expect(ma7.maxFor(12)).toBe(0);
  });

  it('Mystic Arcanum 7th = 1 at level 13', () => {
    const ma7 = warlock.find((r) => r.name === 'Mystic Arcanum (7th)')!;
    expect(ma7.maxFor(13)).toBe(1);
  });

  it('Mystic Arcanum 8th = 1 at level 15', () => {
    const ma8 = warlock.find((r) => r.name === 'Mystic Arcanum (8th)')!;
    expect(ma8.maxFor(15)).toBe(1);
  });

  it('Mystic Arcanum 9th = 1 at level 17', () => {
    const ma9 = warlock.find((r) => r.name === 'Mystic Arcanum (9th)')!;
    expect(ma9.maxFor(17)).toBe(1);
  });
});

describe('Wizard', () => {
  const wizard = RESOURCES_BY_CLASS['Wizard'];

  it('Arcane Recovery = 1 at level 1', () => {
    const ar = wizard.find((r) => r.name === 'Arcane Recovery')!;
    expect(ar.maxFor(1)).toBe(1);
  });

  it('Spell Mastery has minLevel 18', () => {
    const sm = wizard.find((r) => r.name === 'Spell Mastery')!;
    expect(sm.minLevel).toBe(18);
  });

  it('Spell Mastery = 0 below level 18', () => {
    const sm = wizard.find((r) => r.name === 'Spell Mastery')!;
    expect(sm.maxFor(17)).toBe(0);
  });

  it('Spell Mastery = 1 at level 18', () => {
    const sm = wizard.find((r) => r.name === 'Spell Mastery')!;
    expect(sm.maxFor(18)).toBe(1);
  });
});

describe('Sorcerer', () => {
  const sorcerer = RESOURCES_BY_CLASS['Sorcerer'];

  it('Sorcery Points = 0 at level 1', () => {
    const sp = sorcerer.find((r) => r.name === 'Sorcery Points')!;
    expect(sp.maxFor(1)).toBe(0);
  });

  it('Sorcery Points = level at levels 2–20', () => {
    const sp = sorcerer.find((r) => r.name === 'Sorcery Points')!;
    for (const L of [2, 5, 10, 15, 20]) {
      expect(sp.maxFor(L)).toBe(L);
    }
  });

  it('Sorcerous Restoration has minLevel 20', () => {
    const sr = sorcerer.find((r) => r.name === 'Sorcerous Restoration')!;
    expect(sr.minLevel).toBe(20);
  });

  it('Sorcerous Restoration = 0 at level 19', () => {
    const sr = sorcerer.find((r) => r.name === 'Sorcerous Restoration')!;
    expect(sr.maxFor(19)).toBe(0);
  });

  it('Sorcerous Restoration = 4 at level 20', () => {
    const sr = sorcerer.find((r) => r.name === 'Sorcerous Restoration')!;
    expect(sr.maxFor(20)).toBe(4);
  });
});

describe('Paladin', () => {
  const paladin = RESOURCES_BY_CLASS['Paladin'];

  it('Lay on Hands Pool = level × 5', () => {
    const loh = paladin.find((r) => r.name === 'Lay on Hands Pool')!;
    expect(loh.maxFor(1)).toBe(5);
    expect(loh.maxFor(5)).toBe(25);
    expect(loh.maxFor(20)).toBe(100);
  });

  it('Cleansing Touch has minLevel 6', () => {
    const ct = paladin.find((r) => r.name === 'Cleansing Touch')!;
    expect(ct.minLevel).toBe(6);
  });

  it('Cleansing Touch = 0 at level 5', () => {
    const ct = paladin.find((r) => r.name === 'Cleansing Touch')!;
    expect(ct.maxFor(5)).toBe(0);
  });

  it('Cleansing Touch = 1 at level 6', () => {
    const ct = paladin.find((r) => r.name === 'Cleansing Touch')!;
    expect(ct.maxFor(6)).toBe(1);
  });
});

describe('Cleric', () => {
  const cleric = RESOURCES_BY_CLASS['Cleric'];

  it('Divine Intervention has minLevel 10', () => {
    const di = cleric.find((r) => r.name === 'Divine Intervention')!;
    expect(di.minLevel).toBe(10);
  });

  it('Divine Intervention = 0 at level 9', () => {
    const di = cleric.find((r) => r.name === 'Divine Intervention')!;
    expect(di.maxFor(9)).toBe(0);
  });

  it('Divine Intervention = 1 at level 10', () => {
    const di = cleric.find((r) => r.name === 'Divine Intervention')!;
    expect(di.maxFor(10)).toBe(1);
  });

  it('War Priest = 1 at level 1', () => {
    const wp = cleric.find((r) => r.name === 'War Priest')!;
    expect(wp.maxFor(1)).toBe(1);
  });
});

describe('Druid', () => {
  const druid = RESOURCES_BY_CLASS['Druid'];

  it('Wild Shape has minLevel 2', () => {
    const ws = druid.find((r) => r.name === 'Wild Shape')!;
    expect(ws.minLevel).toBe(2);
  });

  it('Wild Shape = 0 at level 1', () => {
    const ws = druid.find((r) => r.name === 'Wild Shape')!;
    expect(ws.maxFor(1)).toBe(0);
  });

  it('Wild Shape = 2 at level 2', () => {
    const ws = druid.find((r) => r.name === 'Wild Shape')!;
    expect(ws.maxFor(2)).toBe(2);
  });

  it('Wild Shape = 99 at level 20', () => {
    const ws = druid.find((r) => r.name === 'Wild Shape')!;
    expect(ws.maxFor(20)).toBe(99);
  });

  it('Natural Recovery has minLevel 2', () => {
    const nr = druid.find((r) => r.name === 'Natural Recovery')!;
    expect(nr.minLevel).toBe(2);
  });

  it('Natural Recovery = 1 at level 2', () => {
    const nr = druid.find((r) => r.name === 'Natural Recovery')!;
    expect(nr.maxFor(2)).toBe(1);
  });
});

describe('Rogue', () => {
  it('Stroke of Luck = 0 at level 19', () => {
    const sol = RESOURCES_BY_CLASS['Rogue'].find((r) => r.name === 'Stroke of Luck')!;
    expect(sol.maxFor(19)).toBe(0);
  });

  it('Stroke of Luck = 1 at level 20', () => {
    const sol = RESOURCES_BY_CLASS['Rogue'].find((r) => r.name === 'Stroke of Luck')!;
    expect(sol.maxFor(20)).toBe(1);
  });
});

describe('minLevel semantics', () => {
  it('Monk Ki minLevel is 2 and returns 0 at level 1', () => {
    const ki = RESOURCES_BY_CLASS['Monk'].find((r) => r.name === 'Ki')!;
    expect(ki.minLevel).toBe(2);
    expect(ki.maxFor(1)).toBe(0);
  });

  it('Fighter Indomitable minLevel is 9 and returns 0 at level 8', () => {
    const ind = RESOURCES_BY_CLASS['Fighter'].find((r) => r.name === 'Indomitable')!;
    expect(ind.minLevel).toBe(9);
    expect(ind.maxFor(8)).toBe(0);
  });

  it('Sorcerer Sorcerous Restoration minLevel is 20 and returns 0 at level 19', () => {
    const sr = RESOURCES_BY_CLASS['Sorcerer'].find((r) => r.name === 'Sorcerous Restoration')!;
    expect(sr.minLevel).toBe(20);
    expect(sr.maxFor(19)).toBe(0);
  });

  it('Warlock Eldritch Invocations minLevel is 2 and returns 0 at level 1', () => {
    const ei = RESOURCES_BY_CLASS['Warlock'].find((r) => r.name === 'Eldritch Invocations')!;
    expect(ei.minLevel).toBe(2);
    expect(ei.maxFor(1)).toBe(0);
  });

  it('Wizard Spell Mastery minLevel is 18 and returns 0 at level 17', () => {
    const sm = RESOURCES_BY_CLASS['Wizard'].find((r) => r.name === 'Spell Mastery')!;
    expect(sm.minLevel).toBe(18);
    expect(sm.maxFor(17)).toBe(0);
  });

  it('Druid Wild Shape minLevel is 2 and returns 0 at level 1', () => {
    const ws = RESOURCES_BY_CLASS['Druid'].find((r) => r.name === 'Wild Shape')!;
    expect(ws.minLevel).toBe(2);
    expect(ws.maxFor(1)).toBe(0);
  });

  it('Cleric Divine Intervention minLevel is 10 and returns 0 at level 9', () => {
    const di = RESOURCES_BY_CLASS['Cleric'].find((r) => r.name === 'Divine Intervention')!;
    expect(di.minLevel).toBe(10);
    expect(di.maxFor(9)).toBe(0);
  });

  it('Rogue Stroke of Luck minLevel is 20 and returns 0 at level 19', () => {
    const sol = RESOURCES_BY_CLASS['Rogue'].find((r) => r.name === 'Stroke of Luck')!;
    expect(sol.minLevel).toBe(20);
    expect(sol.maxFor(19)).toBe(0);
  });
});
