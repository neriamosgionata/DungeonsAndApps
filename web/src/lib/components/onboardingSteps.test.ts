import { describe, it, expect } from 'vitest';
import { onboardingSteps, type OnboardingChar } from './onboardingSteps';

function makeChar(overrides: Partial<OnboardingChar> & { sheet?: Record<string, unknown> } = {}): OnboardingChar {
  return {
    id: 'test-id',
    name: 'Testo',
    race: null,
    level_total: 1,
    sheet: {},
    ...overrides,
  };
}

describe('onboardingSteps', () => {
  it('fresh level 1 character has all core steps', () => {
    const c = makeChar();
    const steps = onboardingSteps(c);
    const ids = steps.map((s) => s.id);
    expect(ids).toContain('level');
    expect(ids).toContain('race');
    expect(ids).toContain('class');
    expect(ids).toContain('abilities');
    expect(ids).toContain('skills');
    expect(ids).toContain('hp');
    expect(ids).toContain('ac');
    expect(ids).toContain('equipment');
    expect(ids).toContain('background');
  });

  it('level > 1 skips level step', () => {
    const c = makeChar({ level_total: 3 });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('level');
  });

  it('race set skips race step', () => {
    const c = makeChar({ race: 'Elf' });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('race');
  });

  it('class present skips class step, adds subclass step', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Fighter', level: 1 }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('class');
    expect(steps.map((s) => s.id)).toContain('subclass');
  });

  it('class with subclass skips both class and subclass steps', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Fighter', level: 1, subclass: 'Champion' }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('class');
    expect(steps.map((s) => s.id)).not.toContain('subclass');
  });

  it('class without subclass options (Blood Hunter) skips subclass step', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Blood Hunter', level: 1 }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('subclass');
  });

  it('non-default abilities skip abilities step', () => {
    const c = makeChar({
      sheet: { abilities: { str: 15, dex: 12, con: 14, int: 10, wis: 13, cha: 8 } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('abilities');
  });

  it('abilities step still shows if only one ability is non-default', () => {
    const c = makeChar({
      sheet: { abilities: { str: 12 } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('abilities');
  });

  it('skills present skips skills step', () => {
    const c = makeChar({
      sheet: { skills: { athletics: 'prof' } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('skills');
  });

  it('hp max > 0 skips hp step', () => {
    const c = makeChar({
      sheet: { hp: { max: 10, current: 10 } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('hp');
  });

  it('ac > 10 skips ac step', () => {
    const c = makeChar({
      sheet: { ac: 16 },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('ac');
  });

  it('armor type set skips ac step even if ac is 10', () => {
    const c = makeChar({
      sheet: { ac: 10, armor: { type: 'heavy', ac_base: 16, max_dex: 0 } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('ac');
  });

  it('caster class without spells adds spells step', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Wizard', level: 1 }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).toContain('spells');
  });

  it('non-caster class skips spells step', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Fighter', level: 1, subclass: 'Champion' }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('spells');
  });

  it('caster with spells skips spells step', () => {
    const c = makeChar({
      sheet: {
        classes: [{ name: 'Wizard', level: 1 }],
        spells: [{ slug: 'magic_missile', name: 'Magic Missile', level: 1 }],
      },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('spells');
  });

  it('half-caster (Paladin) also gets spells step', () => {
    const c = makeChar({
      sheet: { classes: [{ name: 'Paladin', level: 2 }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).toContain('spells');
  });

  it('equipment present skips equipment step', () => {
    const c = makeChar({
      sheet: { equipment: [{ id: '1', name: 'Sword', qty: 1, weight: 3 }] },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('equipment');
  });

  it('background backstory set skips background step', () => {
    const c = makeChar({
      sheet: { background: { backstory: 'A long tale.' } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).not.toContain('background');
  });

  it('empty backstory string still shows step', () => {
    const c = makeChar({
      sheet: { background: { backstory: '' } },
    });
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).toContain('background');
  });

  it('fully complete character returns empty steps', () => {
    const c = makeChar({
      level_total: 3,
      race: 'Dwarf',
      sheet: {
        classes: [{ name: 'Fighter', level: 3, subclass: 'Champion' }],
        abilities: { str: 16, dex: 14, con: 16, int: 10, wis: 12, cha: 8 },
        skills: { athletics: 'prof', perception: 'prof' },
        hp: { max: 31, current: 31 },
        ac: 16,
        armor: { type: 'heavy', ac_base: 16, max_dex: 0 },
        equipment: [{ id: '1', name: 'Greatsword', qty: 1, weight: 6 }],
        background: { backstory: 'Born a warrior.' },
      },
    });
    const steps = onboardingSteps(c);
    expect(steps).toHaveLength(0);
  });

  it('autoComplete returns true after condition is met', () => {
    const c = makeChar({ level_total: 1, race: 'Elf' });
    const steps = onboardingSteps(c);
    const levelStep = steps.find((s) => s.id === 'level')!;
    expect(levelStep.autoComplete()).toBe(false);
    c.level_total = 5;
    expect(levelStep.autoComplete()).toBe(true);
  });

  it('autoComplete for class detects added class on the fly', () => {
    const c = makeChar();
    const steps = onboardingSteps(c);
    const classStep = steps.find((s) => s.id === 'class')!;
    expect(classStep.autoComplete()).toBe(false);
    (c.sheet as Record<string, unknown>).classes = [{ name: 'Fighter', level: 1 }];
    expect(classStep.autoComplete()).toBe(true);
  });

  it('steps are ordered correctly for fresh character', () => {
    const c = makeChar();
    const steps = onboardingSteps(c);
    expect(steps.map((s) => s.id)).toEqual([
      'level', 'race', 'class', 'abilities', 'skills', 'hp', 'ac', 'equipment', 'background',
    ]);
  });

  it('each step has required properties', () => {
    const c = makeChar();
    const steps = onboardingSteps(c);
    for (const step of steps) {
      expect(typeof step.id).toBe('string');
      expect(typeof step.msgKey).toBe('string');
      expect(step.msgKey).toMatch(/^onboarding\./);
      expect(typeof step.anchorSel).toBe('string');
      expect(step.anchorSel.startsWith('#ob-')).toBe(true);
      expect(typeof step.autoComplete).toBe('function');
    }
  });
});
