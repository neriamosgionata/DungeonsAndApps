/** Complete SRD 5.1 feat list for cinghialapp. */

export type Ability = 'str' | 'dex' | 'con' | 'int' | 'wis' | 'cha';
export type SaveKey = Ability;

export type FeatEffects = {
  /** Fixed ability score +1 */
  ability?: Ability;
  /** Player must choose one of these abilities for the +1 */
  ability_choice?: Ability[];
  /** Saving throw proficiency added */
  save_prof?: SaveKey;
  /** Grant save proficiency for the chosen ability (used by Resilient) */
  save_prof_from_config?: true;
  /** Flat passive_perception_bonus increase */
  passive_perception?: number;
  /** Flat passive_investigation_bonus increase */
  passive_investigation?: number;
  /** Flat AC bonus (e.g. Dual Wielder +1) */
  ac_bonus?: number;
  /** Override medium armor max DEX cap */
  medium_armor_max_dex?: number;
  /** Non-magical B/P/S damage reduction (Heavy Armor Master) */
  nonmagical_damage_reduction?: number;
  /** Flat speed increase */
  speed?: number;
  /** Flat initiative bonus */
  initiative?: number;
  /** HP max += 2 * level when taken; +2/level after (Tough) */
  tough?: true;
  /** Text appended to proficiencies.armor */
  armor_prof?: string;
  /** Text appended to proficiencies.weapons */
  weapon_prof?: string;
  /** Number of free skill proficiencies the player chooses */
  free_skills?: number;
  /** Resource auto-added to sheet.resources */
  resource?: { name: string; max: number; reset: 'short' | 'long' };
  /** Whether a secondary config param is required (ability/class/damage_type) */
  config_type?: 'ability' | 'ability_choice' | 'class' | 'damage_type' | 'skills';
};

export type FeatPrereq = {
  ability?: { key: Ability; min: number };
  armor_prof?: string;
  can_cast?: true;
};

export type Feat = {
  key: string;
  name: string;
  prereqs: FeatPrereq[];
  description: string;
  effects: FeatEffects;
  /** Short mechanical summary */
  mechanics: string;
  multi?: true; // can be taken multiple times
};

export const FEATS: Feat[] = [
  {
    key: 'alert',
    name: 'Alert',
    prereqs: [],
    description: 'Always on the lookout for danger. +5 to initiative, cannot be surprised while conscious, hidden creatures gain no advantage on attacks against you.',
    mechanics: '+5 initiative · immune to Surprised · no hidden-attacker advantage',
    effects: { initiative: 5 },
  },
  {
    key: 'athlete',
    name: 'Athlete',
    prereqs: [],
    description: '+1 STR or DEX. Standing from prone costs only 5 ft. Climbing has no extra cost. Can long/high jump after only 5 ft of movement.',
    mechanics: '+1 STR or DEX · easier climbing & jumping',
    effects: { ability_choice: ['str', 'dex'], config_type: 'ability_choice' },
  },
  {
    key: 'actor',
    name: 'Actor',
    prereqs: [],
    description: '+1 CHA. Advantage on Deception and Performance checks when impersonating. Can mimic voices and sounds.',
    mechanics: '+1 CHA · advantage on impersonation checks',
    effects: { ability: 'cha' },
  },
  {
    key: 'charger',
    name: 'Charger',
    prereqs: [],
    description: 'When you Dash, you can use a bonus action to attack or shove. If you moved 10+ ft in a straight line, gain +5 damage or push target 10 ft.',
    mechanics: 'Bonus action attack/shove after Dash · +5 dmg or 10 ft push',
    effects: {},
  },
  {
    key: 'crossbow_expert',
    name: 'Crossbow Expert',
    prereqs: [],
    description: 'Ignore the loading property of crossbows. No disadvantage on ranged attacks within 5 ft of a hostile. Bonus action hand crossbow attack after one-handed attack.',
    mechanics: 'No loading penalty · no melee ranged penalty · BA hand crossbow',
    effects: {},
  },
  {
    key: 'defensive_duelist',
    name: 'Defensive Duelist',
    prereqs: [{ ability: { key: 'dex', min: 13 } }],
    description: 'When you are hit by a melee attack while wielding a finesse weapon you are proficient with, use your reaction to add your proficiency bonus to AC.',
    mechanics: 'Reaction: +prof bonus to AC vs one melee hit',
    effects: {},
  },
  {
    key: 'dual_wielder',
    name: 'Dual Wielder',
    prereqs: [],
    description: '+1 AC while dual-wielding melee weapons. You can two-weapon fight with non-light weapons. Draw/stow two weapons simultaneously.',
    mechanics: '+1 AC (dual wield) · non-light two-weapon fighting',
    effects: { ac_bonus: 1 },
  },
  {
    key: 'dungeon_delver',
    name: 'Dungeon Delver',
    prereqs: [],
    description: 'Advantage on Perception/Investigation to find secret doors. Advantage on saves vs. traps. Resistance to trap damage. Search for traps at normal travel pace.',
    mechanics: 'Advantage vs. traps & secret doors · resistance to trap damage',
    effects: {},
  },
  {
    key: 'durable',
    name: 'Durable',
    prereqs: [],
    description: '+1 CON. Minimum HP regained from Hit Dice equals twice your CON modifier (minimum 2).',
    mechanics: '+1 CON · minimum Hit Die heal = 2 × CON mod',
    effects: { ability: 'con' },
  },
  {
    key: 'elemental_adept',
    name: 'Elemental Adept',
    prereqs: [{ can_cast: true }],
    description: 'Choose acid, cold, fire, lightning, or thunder. Your spells ignore resistance to that type. Treat 1s as 2s on damage dice of that type. Can be taken multiple times.',
    mechanics: 'Chosen damage type: ignore resistance, 1→2 on damage dice',
    effects: { config_type: 'damage_type' },
    multi: true,
  },
  {
    key: 'grappler',
    name: 'Grappler',
    prereqs: [{ ability: { key: 'str', min: 13 } }],
    description: 'Advantage on attacks against creatures you are grappling. Action to pin a grappled creature (contested check): success = both Restrained.',
    mechanics: 'Advantage on attacks vs. grappled · Action: pin grappled creature',
    effects: {},
  },
  {
    key: 'great_weapon_master',
    name: 'Great Weapon Master',
    prereqs: [],
    description: 'Bonus action melee attack after a crit or kill. Optional -5/+10 trade on heavy weapon attacks.',
    mechanics: 'BA attack after crit/kill · optional −5 atk/+10 dmg',
    effects: {},
  },
  {
    key: 'healer',
    name: 'Healer',
    prereqs: [],
    description: 'Stabilizing with a healer\'s kit also restores 1 HP. Action + healer\'s kit: restore 1d6+4+max_hit_dice HP to a creature (once per short/long rest per creature).',
    mechanics: 'Stabilize also grants 1 HP · Action: heal 1d6+4+max HD',
    effects: {},
  },
  {
    key: 'heavily_armored',
    name: 'Heavily Armored',
    prereqs: [{ armor_prof: 'medium' }],
    description: '+1 STR. Gain proficiency with heavy armor.',
    mechanics: '+1 STR · heavy armor proficiency',
    effects: { ability: 'str', armor_prof: 'Heavy armor' },
  },
  {
    key: 'heavy_armor_master',
    name: 'Heavy Armor Master',
    prereqs: [{ armor_prof: 'heavy' }],
    description: '+1 STR. While wearing heavy armor, reduce bludgeoning, piercing, and slashing damage from nonmagical weapons by 3.',
    mechanics: '+1 STR · DR 3 vs. nonmagical B/P/S (heavy armor)',
    effects: { ability: 'str', nonmagical_damage_reduction: 3 },
  },
  {
    key: 'inspiring_leader',
    name: 'Inspiring Leader',
    prereqs: [{ ability: { key: 'cha', min: 13 } }],
    description: 'Spend 10 minutes to grant up to 6 friendly creatures temp HP equal to your level + CHA modifier. Once per short/long rest per creature.',
    mechanics: '10 min: grant up to 6 creatures temp HP = level + CHA mod',
    effects: { resource: { name: 'Inspiring Leader', max: 1, reset: 'short' } },
  },
  {
    key: 'keen_mind',
    name: 'Keen Mind',
    prereqs: [],
    description: '+1 INT. Always know which way is north, hours to next sunrise/sunset, and can accurately recall anything seen/heard in the past month.',
    mechanics: '+1 INT · perfect recall & navigation',
    effects: { ability: 'int' },
  },
  {
    key: 'lightly_armored',
    name: 'Lightly Armored',
    prereqs: [],
    description: '+1 STR or DEX. Gain proficiency with light armor.',
    mechanics: '+1 STR or DEX · light armor proficiency',
    effects: { ability_choice: ['str', 'dex'], armor_prof: 'Light armor', config_type: 'ability_choice' },
  },
  {
    key: 'linguist',
    name: 'Linguist',
    prereqs: [],
    description: '+1 INT. Learn three languages of your choice. Can create written ciphers others cannot decode without your help or a high Intelligence check.',
    mechanics: '+1 INT · +3 languages · cipher creation',
    effects: { ability: 'int' },
  },
  {
    key: 'lucky',
    name: 'Lucky',
    prereqs: [],
    description: '3 luck points per long rest. Spend a point to roll an extra d20 on any attack/check/save and choose which to use. Can also negate advantage on attacks against you.',
    mechanics: '3 luck points/long rest · reroll any d20',
    effects: { resource: { name: 'Luck Points', max: 3, reset: 'long' } },
  },
  {
    key: 'mage_slayer',
    name: 'Mage Slayer',
    prereqs: [],
    description: 'Reaction: melee attack when creature within 5 ft casts a spell. Damage you deal causes disadvantage on concentration saves. Advantage on saves vs. spells from creatures within 5 ft.',
    mechanics: 'Reaction attack vs. nearby casters · concentration disadvantage · adv on close-range spell saves',
    effects: {},
  },
  {
    key: 'magic_initiate',
    name: 'Magic Initiate',
    prereqs: [],
    description: 'Choose a class. Learn 2 cantrips and 1 first-level spell from that class\'s list. Cast the 1st-level spell once per long rest.',
    mechanics: '2 cantrips + 1st-level spell (1/long rest)',
    effects: { config_type: 'class' },
  },
  {
    key: 'martial_adept',
    name: 'Martial Adept',
    prereqs: [],
    description: 'Learn 2 Battle Master maneuvers. Gain 1 Superiority Die (d6) or +1 if you already have them. Recharges on short/long rest.',
    mechanics: '2 maneuvers · 1 Superiority Die d6/short rest',
    effects: { resource: { name: 'Superiority Die', max: 1, reset: 'short' } },
  },
  {
    key: 'medium_armor_master',
    name: 'Medium Armor Master',
    prereqs: [{ armor_prof: 'medium' }],
    description: 'Medium armor no longer imposes Stealth disadvantage. With DEX 16+, add up to 3 to AC from medium armor instead of 2.',
    mechanics: 'No Stealth penalty in medium armor · DEX cap +3',
    effects: { medium_armor_max_dex: 3 },
  },
  {
    key: 'mobile',
    name: 'Mobile',
    prereqs: [],
    description: '+10 ft speed. Difficult terrain costs no extra movement while Dashing. Melee attacks against a creature stop it from taking opportunity attacks against you.',
    mechanics: '+10 speed · no OA from attacked creatures · no diff terrain on Dash',
    effects: { speed: 10 },
  },
  {
    key: 'moderately_armored',
    name: 'Moderately Armored',
    prereqs: [{ armor_prof: 'light' }],
    description: '+1 STR or DEX. Gain proficiency with medium armor and shields.',
    mechanics: '+1 STR or DEX · medium armor & shield proficiency',
    effects: { ability_choice: ['str', 'dex'], armor_prof: 'Medium armor, Shield', config_type: 'ability_choice' },
  },
  {
    key: 'mounted_combatant',
    name: 'Mounted Combatant',
    prereqs: [],
    description: 'Advantage on melee attacks vs. unmounted smaller creatures. Redirect attacks targeting your mount to yourself. Your mount gains Evasion vs. DEX saves.',
    mechanics: 'Advantage vs. smaller unmounted · redirect mount attacks · mount Evasion',
    effects: {},
  },
  {
    key: 'observant',
    name: 'Observant',
    prereqs: [],
    description: '+1 INT or WIS. Can read lips. +5 to passive Perception and passive Investigation.',
    mechanics: '+1 INT or WIS · lip reading · +5 passive Perception & Investigation',
    effects: { ability_choice: ['int', 'wis'], passive_perception: 5, passive_investigation: 5, config_type: 'ability_choice' },
  },
  {
    key: 'polearm_master',
    name: 'Polearm Master',
    prereqs: [],
    description: 'Bonus action d4 butt-end attack when attacking with glaive/halberd/quarterstaff. Creatures entering your reach provoke opportunity attacks when wielding those weapons.',
    mechanics: 'BA d4 attack · OAs on enter reach',
    effects: {},
  },
  {
    key: 'resilient',
    name: 'Resilient',
    prereqs: [],
    description: 'Choose an ability. +1 to that score. Gain proficiency in saving throws using that ability.',
    mechanics: '+1 to chosen ability · proficiency in that saving throw',
    effects: { config_type: 'ability', save_prof_from_config: true },
  },
  {
    key: 'ritual_caster',
    name: 'Ritual Caster',
    prereqs: [{ ability: { key: 'int', min: 13 } }],
    description: 'Acquire a ritual book with 2 first-level ritual spells from a chosen class. Can copy ritual spells of level ≤ half your level into the book.',
    mechanics: 'Ritual book: 2 1st-level rituals, copy more up to half-level',
    effects: { config_type: 'class' },
  },
  {
    key: 'savage_attacker',
    name: 'Savage Attacker',
    prereqs: [],
    description: 'Once per turn when you roll melee weapon damage, you can reroll the weapon\'s damage dice and use either result.',
    mechanics: 'Once/turn: reroll melee weapon damage dice, keep higher',
    effects: {},
  },
  {
    key: 'sentinel',
    name: 'Sentinel',
    prereqs: [],
    description: 'Opportunity attack hit sets target speed to 0. Disengage doesn\'t prevent your OAs. Reaction: attack a creature within 5 ft that attacks someone other than you.',
    mechanics: 'OA hit → speed 0 · Disengage no escape · Reaction attack on nearby hostile',
    effects: {},
  },
  {
    key: 'sharpshooter',
    name: 'Sharpshooter',
    prereqs: [],
    description: 'No disadvantage at long range. Ranged attacks ignore half and three-quarters cover. Optional -5/+10 trade on ranged weapon attacks.',
    mechanics: 'No long-range penalty · ignore cover · optional −5 atk/+10 dmg',
    effects: {},
  },
  {
    key: 'shield_master',
    name: 'Shield Master',
    prereqs: [],
    description: 'Bonus action shove with shield after Attack. Add shield AC to DEX saves vs. single-target effects. Reaction: take no damage on successful DEX save (Evasion with shield).',
    mechanics: 'BA shield shove · shield bonus to DEX saves · Reaction: 0 damage on DEX save success',
    effects: {},
  },
  {
    key: 'skilled',
    name: 'Skilled',
    prereqs: [],
    description: 'Gain proficiency in any combination of three skills or tools of your choice.',
    mechanics: '+3 skill or tool proficiencies',
    effects: { free_skills: 3, config_type: 'skills' },
  },
  {
    key: 'skulker',
    name: 'Skulker',
    prereqs: [{ ability: { key: 'dex', min: 13 } }],
    description: 'Can Hide when lightly obscured. Missing with a ranged attack while hidden doesn\'t reveal position. No disadvantage on Perception in dim light.',
    mechanics: 'Hide in light obscurement · ranged miss keeps stealth · no Perception penalty in dim light',
    effects: {},
  },
  {
    key: 'spell_sniper',
    name: 'Spell Sniper',
    prereqs: [{ can_cast: true }],
    description: 'Attack-roll spells have double range. Ranged spell attacks ignore half and three-quarters cover. Learn 1 attack-roll cantrip from a class of your choice.',
    mechanics: 'Double spell range · ignore cover · +1 attack cantrip',
    effects: { config_type: 'class' },
  },
  {
    key: 'tavern_brawler',
    name: 'Tavern Brawler',
    prereqs: [],
    description: '+1 STR or CON. Proficient with improvised weapons. Unarmed strikes deal d4. Bonus action grapple after unarmed or improvised weapon hit.',
    mechanics: '+1 STR or CON · d4 unarmed · BA grapple after hit',
    effects: { ability_choice: ['str', 'con'], config_type: 'ability_choice' },
  },
  {
    key: 'tough',
    name: 'Tough',
    prereqs: [],
    description: 'HP maximum increases by twice your current level. Each level thereafter, HP maximum increases by an additional 2.',
    mechanics: 'HP max +2×level now, +2/level permanently',
    effects: { tough: true },
  },
  {
    key: 'war_caster',
    name: 'War Caster',
    prereqs: [{ can_cast: true }],
    description: 'Advantage on CON saves to maintain concentration. Can perform somatic components with weapons/shield in hand. Reaction: cast a 1-action single-target spell as an opportunity attack.',
    mechanics: 'Adv on concentration saves · somatic with full hands · Reaction: spell as OA',
    effects: {},
  },
  {
    key: 'weapon_master',
    name: 'Weapon Master',
    prereqs: [],
    description: '+1 STR or DEX. Gain proficiency with four weapons of your choice (simple or martial).',
    mechanics: '+1 STR or DEX · +4 weapon proficiencies',
    effects: { ability_choice: ['str', 'dex'], config_type: 'ability_choice' },
  },
];

export function featByKey(key: string): Feat | undefined {
  return FEATS.find((f) => f.key === key);
}

/** Check if a feat's prerequisites are met given the current sheet. */
export function featPrereqsMet(
  feat: Feat,
  sheet: Record<string, unknown>,
): boolean {
  for (const p of feat.prereqs) {
    if (p.ability) {
      const ab = (sheet.abilities as Record<string, number | undefined> | undefined)?.[p.ability.key] ?? 10;
      if (ab < p.ability.min) return false;
    }
    if (p.armor_prof) {
      const ap = ((sheet.proficiencies as Record<string, string | undefined> | undefined)?.armor ?? '').toLowerCase();
      if (!ap.includes(p.armor_prof.toLowerCase())) return false;
    }
    if (p.can_cast) {
      const classes = (sheet.classes as { name?: string; level?: number }[] | undefined) ?? [];
      const casters = ['bard','cleric','druid','paladin','ranger','sorcerer','warlock','wizard'];
      const hasCaster = classes.some((c) => casters.some((k) => (c.name ?? '').toLowerCase().includes(k)));
      const hasSlot = Object.values((sheet.slots as Record<string, { max: number }> | undefined) ?? {}).some((s) => s.max > 0);
      const hasSpell = ((sheet.spells as unknown[]) ?? []).length > 0;
      if (!hasCaster && !hasSlot && !hasSpell) return false;
    }
  }
  return true;
}
