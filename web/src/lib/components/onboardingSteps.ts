export type OnboardingStep = {
  id: string;
  msgKey: string;
  tab: string | null;
  anchorSel: string;
  autoComplete: () => boolean;
};

export type OnboardingChar = {
  id: string;
  name: string;
  race?: string | null;
  level_total: number;
  sheet: Record<string, unknown>;
};

const CASTER_NAMES = ['bard','cleric','druid','paladin','ranger','sorcerer','warlock','wizard','artificer'];
const SUBCLASS_CLASSES = ['fighter','rogue','wizard','cleric','druid','barbarian','bard','monk','paladin','ranger','sorcerer','warlock'];

export function onboardingSteps(c: OnboardingChar): OnboardingStep[] {
  const sheet = (c.sheet ?? {}) as Record<string, unknown>;
  const classes = (sheet.classes as Array<{ name?: string; level?: number; subclass?: string }>) ?? [];
  const named = classes.filter((cl) => !!cl.name?.trim());
  const spells = (sheet.spells as Array<unknown>) ?? [];
  const equipment = (sheet.equipment as Array<unknown>) ?? [];
  const abilities = (sheet.abilities as Record<string, number | undefined>) ?? {};
  const skills = (sheet.skills as Record<string, string> | undefined) ?? {};
  const hasSkills = Object.keys(skills).length > 0;
  const allAbilities10 = (['str','dex','con','int','wis','cha'] as const).every((k) => (abilities[k] ?? 10) === 10);

  const hasCasterClass = classes.some((cl) => {
    const n = (cl.name ?? '').toLowerCase();
    if (!n) return false;
    return CASTER_NAMES.some((cn) => n.includes(cn));
  });

  const steps: OnboardingStep[] = [];
  const raceSet = !!c.race;

  if (c.level_total <= 1) {
    steps.push({
      id: 'level',
      msgKey: 'onboarding.level_hint',
      tab: null,
      anchorSel: '#ob-level',
      autoComplete: () => c.level_total > 1,
    });
  }

  if (!raceSet) {
    steps.push({
      id: 'race',
      msgKey: 'onboarding.race_hint',
      tab: null,
      anchorSel: '#ob-race',
      autoComplete: () => !!c.race,
    });
  }

  if (named.length === 0) {
    steps.push({
      id: 'class',
      msgKey: 'onboarding.class_hint',
      tab: 'features',
      anchorSel: '#ob-tab-features',
      autoComplete: () => {
        const cl = (c.sheet as Record<string, unknown>)?.classes as Array<{ name?: string }> | undefined;
        return (cl ?? []).some((x) => !!x.name?.trim());
      },
    });
  } else {
    const classWithoutSub = named.find((cl) => !cl.subclass?.trim());
    if (classWithoutSub && classWithoutSub.name) {
      const cn = classWithoutSub.name.toLowerCase();
      const hasSubclasses = SUBCLASS_CLASSES.some((sc) => cn.includes(sc));
      if (hasSubclasses) {
        steps.push({
          id: 'subclass',
          msgKey: 'onboarding.subclass_hint',
          tab: 'features',
          anchorSel: '#ob-tab-features',
          autoComplete: () => {
            const cl = (c.sheet as Record<string, unknown>)?.classes as Array<{ subclass?: string }> | undefined;
            return (cl ?? []).some((x) => !!x.subclass?.trim());
          },
        });
      }
    }
  }

  if (allAbilities10) {
    steps.push({
      id: 'abilities',
      msgKey: 'onboarding.abilities_hint',
      tab: 'combat',
      anchorSel: '#ob-tab-combat',
      autoComplete: () => {
        const ab = (c.sheet as Record<string, unknown>)?.abilities as Record<string, number | undefined> | undefined;
        if (!ab) return false;
        return (['str','dex','con','int','wis','cha'] as const).some((k) => (ab[k] ?? 10) !== 10);
      },
    });
  }

  if (!hasSkills) {
    steps.push({
      id: 'skills',
      msgKey: 'onboarding.skills_hint',
      tab: 'combat',
      anchorSel: '#ob-tab-combat',
      autoComplete: () => Object.keys((c.sheet as Record<string, unknown>)?.skills as Record<string, unknown> || {}).length > 0,
    });
  }

  const hpMax = (sheet.hp as { max?: number } | undefined)?.max ?? 0;
  if (hpMax <= 0) {
    steps.push({
      id: 'hp',
      msgKey: 'onboarding.hp_hint',
      tab: 'vitals',
      anchorSel: '#ob-tab-vitals',
      autoComplete: () => {
        const hp = (c.sheet as Record<string, unknown>)?.hp as { max?: number } | undefined;
        return (hp?.max ?? 0) > 0;
      },
    });
  }

  const ac = (sheet.ac as number) ?? 10;
  const armor = sheet.armor as Record<string, unknown> | undefined;
  if (ac <= 10 && !armor?.type) {
    steps.push({
      id: 'ac',
      msgKey: 'onboarding.ac_hint',
      tab: 'combat',
      anchorSel: '#ob-tab-combat',
      autoComplete: () => {
        const s = c.sheet as Record<string, unknown>;
        const a = (s.ac as number) ?? 10;
        const ar = s.armor as Record<string, unknown> | undefined;
        return a !== 10 || !!ar?.type;
      },
    });
  }

  if (hasCasterClass && spells.length === 0) {
    steps.push({
      id: 'spells',
      msgKey: 'onboarding.spells_hint',
      tab: 'magic',
      anchorSel: '#ob-tab-magic',
      autoComplete: () => ((c.sheet as Record<string, unknown>)?.spells as Array<unknown> ?? []).length > 0,
    });
  }

  if (equipment.length === 0) {
    steps.push({
      id: 'equipment',
      msgKey: 'onboarding.equipment_hint',
      tab: 'loot',
      anchorSel: '#ob-tab-loot',
      autoComplete: () => ((c.sheet as Record<string, unknown>)?.equipment as Array<unknown> ?? []).length > 0,
    });
  }

  const bg = (sheet.background as { backstory?: string } | undefined)?.backstory;
  if (!bg?.trim()) {
    steps.push({
      id: 'background',
      msgKey: 'onboarding.background_hint',
      tab: 'story',
      anchorSel: '#ob-tab-story',
      autoComplete: () => {
        const b = (c.sheet as Record<string, unknown>)?.background as { backstory?: string } | undefined;
        return !!b?.backstory?.trim();
      },
    });
  }

  return steps;
}
