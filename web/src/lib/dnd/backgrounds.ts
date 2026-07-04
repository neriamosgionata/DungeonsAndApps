/** PHB 5e backgrounds with mechanical effects (skill + tool proficiencies, languages). */

export interface BackgroundDef {
  /** Stable key, lowercase, used in `sheet.background_key`. */
  key: string;
  /** Display name, i18n-lookup key `character.background_${key}`. */
  name: string;
  /** Two skill proficiencies the background grants (PHB p.127-141). */
  skills: [string, string];
  /** Tool proficiency name (free-form, matches `tool_proficiencies[].name`). */
  tool: string;
  /** Number of extra languages the background grants (0, 1, or 2). */
  languages: number;
  /** Short mechanical summary (i18n-lookup key `character.background_${key}_mechanics`). */
  mechanics: string;
  /** Starting equipment summary (one-line). */
  equipment: string;
}

/** PHB backgrounds (PHB p.127-141). */
export const BACKGROUNDS: BackgroundDef[] = [
  {
    key: 'acolyte',
    name: 'Acolyte',
    skills: ['insight', 'religion'],
    tool: '',
    languages: 2,
    mechanics: 'Insight · Religion · 2 languages',
    equipment: 'Holy symbol, prayer book, 5 sticks of incense, vestments, common clothes, belt pouch (15 gp)',
  },
  {
    key: 'charlatan',
    name: 'Charlatan',
    skills: ['deception', 'sleight_of_hand'],
    tool: 'Disguise kit',
    languages: 0,
    mechanics: 'Deception · Sleight of Hand · Disguise kit',
    equipment: 'Fine clothes, disguise kit, tools of the con of your choice (ten stoppered bottles, box of switches, deck of marked cards), belt pouch (15 gp)',
  },
  {
    key: 'criminal',
    name: 'Criminal',
    skills: ['deception', 'stealth'],
    tool: "Thieves' tools",
    languages: 0,
    mechanics: 'Deception · Stealth · Thieves\' tools',
    equipment: 'Crowbar, common clothes, belt pouch (15 gp)',
  },
  {
    key: 'entertainer',
    name: 'Entertainer',
    skills: ['acrobatics', 'performance'],
    tool: 'Disguise kit',
    languages: 0,
    mechanics: 'Acrobatics · Performance · Disguise kit',
    equipment: 'Musical instrument (one of your choice), costume, belt pouch (15 gp)',
  },
  {
    key: 'folk_hero',
    name: 'Folk Hero',
    skills: ['animal_handling', 'survival'],
    tool: "Artisan's tools",
    languages: 0,
    mechanics: 'Animal Handling · Survival · Artisan\'s tools',
    equipment: "Artisan's tools (one of your choice), shovel, iron pot, common clothes, belt pouch (10 gp)",
  },
  {
    key: 'guild_artisan',
    name: 'Guild Artisan',
    skills: ['insight', 'persuasion'],
    tool: "Artisan's tools",
    languages: 0,
    mechanics: 'Insight · Persuasion · Artisan\'s tools',
    equipment: "Artisan's tools (one of your choice), letter of introduction from your guild, common clothes, belt pouch (15 gp)",
  },
  {
    key: 'hermit',
    name: 'Hermit',
    skills: ['medicine', 'religion'],
    tool: 'Herbalism kit',
    languages: 1,
    mechanics: 'Medicine · Religion · Herbalism kit · 1 language',
    equipment: 'Herbalism kit, scroll case stuffed with notes, winter blanket, common clothes, belt pouch (10 gp)',
  },
  {
    key: 'noble',
    name: 'Noble',
    skills: ['history', 'persuasion'],
    tool: 'Gaming set',
    languages: 0,
    mechanics: 'History · Persuasion · Gaming set',
    equipment: 'Fine clothes, signet ring, scroll of pedigree, purse (25 gp)',
  },
  {
    key: 'outlander',
    name: 'Outlander',
    skills: ['athletics', 'survival'],
    tool: 'Musical instrument',
    languages: 1,
    mechanics: 'Athletics · Survival · Musical instrument · 1 language',
    equipment: 'Staff, hunting trap, trophy from a slain animal, common clothes, belt pouch (10 gp)',
  },
  {
    key: 'sage',
    name: 'Sage',
    skills: ['arcana', 'history'],
    tool: '',
    languages: 2,
    mechanics: 'Arcana · History · 2 languages',
    equipment: 'Bottle of black ink, quill, small knife, letter from a dead colleague asking for help, common clothes, belt pouch (10 gp)',
  },
  {
    key: 'sailor',
    name: 'Sailor',
    skills: ['athletics', 'perception'],
    tool: "Navigator's tools",
    languages: 0,
    mechanics: 'Athletics · Perception · Navigator\'s tools',
    equipment: "Belaying pin, 50 ft of silk rope, lucky charm (rabbit foot), common clothes, belt pouch (10 gp)",
  },
  {
    key: 'soldier',
    name: 'Soldier',
    skills: ['athletics', 'intimidation'],
    tool: 'Gaming set',
    languages: 0,
    mechanics: 'Athletics · Intimidation · Gaming set',
    equipment: 'Insignia of rank, trophy from a fallen enemy, bone dice or deck of cards, common clothes, belt pouch (10 gp)',
  },
  {
    key: 'urchin',
    name: 'Urchin',
    skills: ['sleight_of_hand', 'stealth'],
    tool: 'Disguise kit',
    languages: 0,
    mechanics: 'Sleight of Hand · Stealth · Disguise kit',
    equipment: 'Small knife, map of the city, pet mouse, token to remember parents by, common clothes, belt pouch (10 gp)',
  },
];

/** Look up a background by its key. */
export function backgroundByKey(key: string | null | undefined): BackgroundDef | undefined {
  if (!key) return undefined;
  return BACKGROUNDS.find((b) => b.key === key);
}

/**
 * Apply a background's mechanical effects to a sheet:
 *   - sets the 2 skill proficiencies to 'prof'
 *   - adds the tool proficiency (if any) to tool_proficiencies
 *   - language slots are noted in `background.languages` for the user to fill
 *     in (the picker doesn't auto-add because languages are player-chosen).
 *
 * Existing proficiencies in those positions are preserved; the new ones
 * merge in. Removing the background does NOT undo these (the user can
 * clear them manually if desired — matches PHB "you can replace
 * proficiencies" rule).
 */
export function applyBackgroundMechanical(
  bg: BackgroundDef,
  sheet: { skills?: Record<string, 'prof' | 'expert'>; tool_proficiencies?: Array<{ name: string; proficient?: boolean }> }
): { skills: Record<string, 'prof' | 'expert'>; tool_proficiencies: Array<{ name: string; proficient?: boolean }> } {
  const skills: Record<string, 'prof' | 'expert'> = { ...(sheet.skills ?? {}) };
  for (const sk of bg.skills) {
    if (skills[sk] !== 'expert') skills[sk] = 'prof';
  }
  let tool_proficiencies = sheet.tool_proficiencies ?? [];
  if (bg.tool) {
    const has = tool_proficiencies.some((t) => t.name === bg.tool);
    if (!has) {
      tool_proficiencies = [...tool_proficiencies, { name: bg.tool, proficient: true }];
    }
  }
  return { skills, tool_proficiencies };
}
