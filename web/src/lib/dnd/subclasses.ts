/** SRD 5.1 class features (base + subclass) for seeding character sheets. */

export type SubclassFeature = {
  name: string;
  level: number;
  description: string;
  /** If the feature has limited uses, describe the reset. */
  uses?: { max: string; reset: 'short' | 'long' | 'none' };
};

export type ClassDef = {
  /** Base class features every member of the class gets (key ones). */
  base: SubclassFeature[];
  /** Subclasses available; each maps to its feature list. */
  subclasses: Record<string, SubclassFeature[]>;
};

const CLASS_DATA: Record<string, ClassDef> = {
  Barbarian: {
    base: [
      { name: 'Rage', level: 1, description: 'Bonus action: advantage on STR checks/saves, +2 melee damage, resistance to B/P/S. Limited uses/long rest.', uses: { max: '2 (scales)', reset: 'long' } },
      { name: 'Unarmored Defense', level: 1, description: 'AC = 10 + DEX mod + CON mod when not wearing armor.' },
      { name: 'Reckless Attack', level: 2, description: 'Attack with advantage; attackers also have advantage against you until your next turn.' },
      { name: 'Danger Sense', level: 2, description: 'Advantage on DEX saves against visible effects (traps, spells, etc.).' },
      { name: 'Extra Attack', level: 5, description: 'Attack twice per Attack action.' },
      { name: 'Fast Movement', level: 5, description: '+10 ft speed when not wearing heavy armor.' },
      { name: 'Feral Instinct', level: 7, description: 'Advantage on initiative. Can enter rage to act even when surprised.' },
      { name: 'Brutal Critical', level: 9, description: 'Roll one extra weapon damage die on a critical hit.' },
      { name: 'Relentless Rage', level: 11, description: 'When reduced to 0 HP while raging, make DC 10 CON save (scales) to drop to 1 HP instead.' },
    ],
    subclasses: {
      'Path of the Berserker': [
        { name: 'Frenzy', level: 3, description: 'While frenzied, make one bonus attack per turn as a bonus action. Gain 1 level of exhaustion when rage ends.' },
        { name: 'Mindless Rage', level: 6, description: "Can't be charmed or frightened while raging; entering rage ends those conditions." },
        { name: 'Intimidating Presence', level: 10, description: 'Action: frighten one creature within 30 ft (WIS save, DC 8 + prof + CHA). 1 minute, repeated saves each turn.', uses: { max: '1', reset: 'long' } },
        { name: 'Retaliation', level: 14, description: 'Reaction: when you take melee damage from a creature within 5 ft, make one melee weapon attack against it.' },
      ],
    },
  },

  Bard: {
    base: [
      { name: 'Bardic Inspiration', level: 1, description: 'Bonus action: give a creature within 60 ft a Bardic Inspiration die (d6, scales). CHA mod uses/long rest.', uses: { max: 'CHA mod', reset: 'long' } },
      { name: 'Jack of All Trades', level: 2, description: 'Add half proficiency bonus (rounded down) to any ability check you are not proficient in.' },
      { name: 'Song of Rest', level: 2, description: 'Allies who spend Hit Dice during a short rest while you play regain extra HP (d6, scales).' },
      { name: 'Expertise', level: 3, description: 'Double proficiency bonus on two chosen skills.' },
      { name: 'Font of Inspiration', level: 5, description: 'Regain Bardic Inspiration uses on a short rest.' },
      { name: 'Countercharm', level: 6, description: 'Action: give friendly creatures within 30 ft advantage on saves vs charm and fright for the rest of the turn.' },
      { name: 'Magical Secrets', level: 10, description: 'Learn 2 spells from any class spell list.' },
    ],
    subclasses: {
      'College of Lore': [
        { name: 'Bonus Proficiencies', level: 3, description: 'Gain proficiency in 3 skills of your choice.' },
        { name: 'Cutting Words', level: 3, description: 'Reaction: expend one Bardic Inspiration die to subtract the result from a creature\'s attack roll, damage roll, or ability check (within 60 ft).' },
        { name: 'Additional Magical Secrets', level: 6, description: 'Learn 2 spells from any class spell list earlier than normal.' },
        { name: 'Peerless Skill', level: 14, description: 'When you make an ability check, expend one Bardic Inspiration die and add the result to your roll.' },
      ],
    },
  },

  Cleric: {
    base: [
      { name: 'Channel Divinity: Turn Undead', level: 2, description: 'Each undead within 30 ft must flee for 1 minute (WIS save to resist). Recharges on short rest.', uses: { max: '1 (scales)', reset: 'short' } },
      { name: 'Destroy Undead', level: 5, description: 'Turn Undead instantly destroys undead of low CR (scales with level).' },
      { name: 'Divine Intervention', level: 10, description: 'Implore your deity for aid. % chance of success = cleric level. Automatically succeeds at level 20.', uses: { max: '1', reset: 'long' } },
    ],
    subclasses: {
      'Life Domain': [
        { name: 'Disciple of Life', level: 1, description: 'Healing spells you cast restore additional HP equal to 2 + the spell\'s level.' },
        { name: 'Channel Divinity: Preserve Life', level: 2, description: 'Distribute up to 5× cleric level HP among any creatures within 30 ft (up to half each creature\'s max HP).', uses: { max: '1', reset: 'short' } },
        { name: 'Blessed Healer', level: 6, description: 'When you cast a healing spell on another creature, you regain 2 + spell level HP.' },
        { name: 'Divine Strike', level: 8, description: 'Once per turn, weapon attacks deal +1d8 radiant damage (+2d8 at level 14).' },
        { name: 'Supreme Healing', level: 17, description: 'Instead of rolling healing spell dice, use the maximum number for each die.' },
      ],
    },
  },

  Druid: {
    base: [
      { name: 'Wild Shape', level: 2, description: 'Transform into a beast of CR up to 1/4 (scales). Twice per short rest.', uses: { max: '2', reset: 'short' } },
      { name: 'Timeless Body', level: 18, description: 'Age 10× slower than normal. No longer need food or water.' },
      { name: 'Beast Spells', level: 18, description: 'Can cast spells while in Wild Shape form.' },
    ],
    subclasses: {
      'Circle of the Land': [
        { name: 'Bonus Cantrip', level: 2, description: 'Learn one additional druid cantrip of your choice.' },
        { name: 'Natural Recovery', level: 2, description: 'Once per long rest (after a short rest): recover expended spell slots with total levels ≤ half druid level (max 5th level each).', uses: { max: '1', reset: 'long' } },
        { name: 'Circle Spells', level: 3, description: 'Your chosen land type (Arctic, Coast, Desert, Forest, Grassland, Mountain, Swamp, or Underdark) grants always-prepared spells unlocked at levels 3, 5, 7, and 9.' },
        { name: "Land's Stride", level: 6, description: 'Move through nonmagical difficult terrain without penalty. Advantage on saves vs. magically created plants.' },
        { name: "Nature's Ward", level: 10, description: 'Immune to poison and disease. Fey and elementals cannot charm or frighten you.' },
        { name: "Nature's Sanctuary", level: 14, description: 'Beasts and plants must make a WIS save (DC 8 + prof + WIS) to attack you.' },
      ],
    },
  },

  Fighter: {
    base: [
      { name: 'Second Wind', level: 1, description: 'Bonus action: regain 1d10 + fighter level HP. Once per short rest.', uses: { max: '1', reset: 'short' } },
      { name: 'Action Surge', level: 2, description: 'Take one additional action on your turn. Once per short rest (twice/short rest at level 17).', uses: { max: '1', reset: 'short' } },
      { name: 'Extra Attack', level: 5, description: 'Attack twice per Attack action (3× at 11, 4× at 20).' },
      { name: 'Indomitable', level: 9, description: 'Reroll a failed saving throw. Once per long rest (twice at 13, three times at 17).', uses: { max: '1', reset: 'long' } },
    ],
    subclasses: {
      Champion: [
        { name: 'Improved Critical', level: 3, description: 'Your weapon attacks score a critical hit on a 19 or 20.' },
        { name: 'Remarkable Athlete', level: 7, description: 'Add half your proficiency bonus (rounded up) to STR, DEX, and CON checks you are not proficient in. Running long jump distance increases by DEX mod (ft).' },
        { name: 'Additional Fighting Style', level: 10, description: 'Gain one additional Fighting Style of your choice.' },
        { name: 'Superior Critical', level: 15, description: 'Your weapon attacks score a critical hit on an 18, 19, or 20.' },
        { name: 'Survivor', level: 18, description: 'At the start of your turn, if you are at or below half your HP max, regain 5 + CON mod HP. Does not trigger at 0 HP.' },
      ],
    },
  },

  Monk: {
    base: [
      { name: 'Martial Arts', level: 1, description: 'Use DEX for unarmed strikes and monk weapons. Unarmed strike deals 1d4 (scales to 1d10). Bonus unarmed strike after Attack action.' },
      { name: 'Ki', level: 2, description: 'Ki points = monk level. Powers: Flurry of Blows (2 bonus unarmed strikes), Patient Defense (Dodge), Step of the Wind (Dash/Disengage + doubled jump).', uses: { max: 'monk level', reset: 'short' } },
      { name: 'Unarmored Movement', level: 2, description: '+10 ft speed (scales to +30 ft at level 18) when not wearing armor or a shield.' },
      { name: 'Stunning Strike', level: 5, description: 'Spend 1 ki after hitting a creature: CON save or the target is Stunned until the end of your next turn.' },
      { name: 'Ki-Empowered Strikes', level: 6, description: 'Your unarmed strikes count as magical for overcoming resistance and immunity.' },
      { name: 'Evasion', level: 7, description: 'When subjected to a DEX save AOE effect: success = no damage, failure = half damage.' },
      { name: 'Diamond Soul', level: 14, description: 'Proficiency in all saving throws. Spend 1 ki to reroll a failed save.' },
    ],
    subclasses: {
      'Way of the Open Hand': [
        { name: 'Open Hand Technique', level: 3, description: 'When you Flurry of Blows, choose one effect per hit: knock prone (DEX save), push 15 ft (STR save), or prevent reactions until start of your next turn.' },
        { name: 'Wholeness of Body', level: 6, description: 'Action: regain HP equal to 3× monk level. Once per long rest.', uses: { max: '1', reset: 'long' } },
        { name: 'Tranquility', level: 11, description: 'At the end of a long rest, gain Sanctuary (WIS save for attackers) until you deal damage or cast a non-defensive spell.' },
        { name: 'Quivering Palm', level: 17, description: 'Spend 3 ki on a hit: set lethal vibrations. Use an action later to deal 10d10 necrotic (CON save for half). Once at a time.', uses: { max: '1 active', reset: 'none' } },
      ],
    },
  },

  Paladin: {
    base: [
      { name: 'Divine Sense', level: 1, description: 'Detect celestials, fiends, and undead within 60 ft not behind total cover. CHA mod + 1 uses/long rest.', uses: { max: 'CHA mod + 1', reset: 'long' } },
      { name: 'Lay on Hands', level: 1, description: 'Pool of 5× paladin level HP. Touch to restore HP or (5 HP) cure one disease/poison.', uses: { max: '5×level HP pool', reset: 'long' } },
      { name: 'Divine Smite', level: 2, description: 'On a hit, expend a spell slot: deal 2d8 radiant + 1d8/slot level above 1st (bonus d8 vs undead/fiends). Max 5d8.' },
      { name: 'Divine Health', level: 3, description: 'Immune to disease.' },
      { name: 'Channel Divinity', level: 3, description: 'Use one of your Channel Divinity options. Once per short rest.', uses: { max: '1', reset: 'short' } },
      { name: 'Extra Attack', level: 5, description: 'Attack twice per Attack action.' },
      { name: 'Aura of Protection', level: 6, description: 'Allies within 10 ft (20 ft at level 18) add your CHA mod (min +1) to all saving throws.' },
      { name: 'Aura of Courage', level: 10, description: 'Allies within 10 ft (20 ft at level 18) cannot be frightened.' },
      { name: 'Improved Divine Smite', level: 11, description: 'All melee weapon hits deal +1d8 radiant damage.' },
      { name: 'Cleansing Touch', level: 14, description: 'Action: end one spell affecting you or a willing creature. CHA mod uses/long rest.', uses: { max: 'CHA mod', reset: 'long' } },
    ],
    subclasses: {
      'Oath of Devotion': [
        { name: 'Sacred Weapon', level: 3, description: 'Channel Divinity: weapon gains +CHA mod to attack rolls and emits bright light (20 ft) for 1 minute.' },
        { name: 'Turn the Unholy', level: 3, description: 'Channel Divinity: fiends and undead within 30 ft must flee for 1 minute (WIS save).' },
        { name: 'Aura of Devotion', level: 7, description: 'Allies within 10 ft (20 ft at level 18) cannot be charmed.' },
        { name: 'Purity of Spirit', level: 15, description: 'Always under the effects of Protection from Evil and Good.' },
        { name: 'Holy Nimbus', level: 20, description: 'Action: emit a sunlight aura (30 ft) for 1 minute. Enemies starting their turn inside take 10 radiant. Advantage on saves vs fiend/undead spells. Once/long rest.', uses: { max: '1', reset: 'long' } },
      ],
    },
  },

  Ranger: {
    base: [
      { name: 'Favored Enemy', level: 1, description: 'Choose a creature type: advantage on Survival to track them and INT checks about them. +1 type at levels 6 and 14.' },
      { name: 'Natural Explorer', level: 1, description: 'Choose a favored terrain: double proficiency on INT/WIS checks there, navigation and travel benefits.' },
      { name: 'Primeval Awareness', level: 3, description: 'Expend a spell slot: sense favored enemy types within 1 mile (6 miles in favored terrain) for 1 min/slot level.' },
      { name: 'Extra Attack', level: 5, description: 'Attack twice per Attack action.' },
      { name: "Land's Stride", level: 8, description: 'Nonmagical difficult terrain costs no extra movement. Advantage on saves vs magically created plants.' },
      { name: 'Hide in Plain Sight', level: 10, description: 'Spend 1 minute camouflaging yourself: +10 to DEX (Stealth) while you remain still.' },
      { name: 'Vanish', level: 14, description: 'Hide as a bonus action. Cannot be tracked by nonmagical means.' },
    ],
    subclasses: {
      Hunter: [
        { name: 'Hunter\'s Prey', level: 3, description: 'Choose one: Colossus Slayer (+1d8 damage once/turn vs bloodied targets), Giant Killer (reaction attack vs Large+ on miss), or Horde Breaker (one extra attack vs adjacent target).' },
        { name: 'Defensive Tactics', level: 7, description: 'Choose one: Escape the Horde (OAs against you at disadvantage), Multiattack Defense (+4 AC vs rest of attacker\'s attacks), or Steel Will (advantage on fright saves).' },
        { name: 'Multiattack', level: 11, description: 'Choose one: Volley (ranged attack all creatures in 10 ft radius) or Whirlwind Attack (melee attack all creatures within 5 ft).' },
        { name: 'Superior Hunter\'s Defense', level: 15, description: 'Choose one: Evasion (AOE DEX save: success = 0 dmg, fail = half), Stand Against the Tide (redirect attacker\'s miss to another creature), or Uncanny Dodge (reaction: halve one attack\'s damage).' },
      ],
    },
  },

  Rogue: {
    base: [
      { name: 'Sneak Attack', level: 1, description: 'Once per turn, +1d6 damage when you have advantage or an ally is adjacent to target (scales to 10d6 at level 19).' },
      { name: 'Cunning Action', level: 2, description: 'Bonus action: Dash, Disengage, or Hide.' },
      { name: 'Uncanny Dodge', level: 5, description: 'Reaction: halve the damage of one attack that hits you.' },
      { name: 'Evasion', level: 7, description: 'DEX save against AOE: success = no damage, failure = half damage.' },
      { name: 'Reliable Talent', level: 11, description: 'Any proficient ability check roll of 9 or lower is treated as a 10.' },
      { name: 'Slippery Mind', level: 15, description: 'Gain proficiency in WIS saving throws.' },
      { name: 'Elusive', level: 18, description: 'Attackers never have advantage on attack rolls against you unless you are incapacitated.' },
    ],
    subclasses: {
      Thief: [
        { name: 'Fast Hands', level: 3, description: 'Cunning Action can also be used to make a Sleight of Hand check, use thieves\' tools, or take the Use an Object action.' },
        { name: 'Second-Story Work', level: 3, description: 'Climb at full movement speed. Running jump distance increases by DEX mod (ft).' },
        { name: 'Supreme Sneak', level: 9, description: 'Advantage on DEX (Stealth) checks when moving at half speed or less.' },
        { name: 'Use Magic Device', level: 13, description: 'Ignore class, race, and level requirements on magic items.' },
        { name: "Thief's Reflexes", level: 17, description: 'Take two full turns in the first round of combat: first at your initiative, second at initiative − 10.' },
      ],
    },
  },

  Sorcerer: {
    base: [
      { name: 'Sorcery Points', level: 2, description: 'Pool = sorcerer level. Flexible Casting: convert to/from spell slots. Powers Metamagic options.', uses: { max: 'sorcerer level', reset: 'long' } },
      { name: 'Metamagic', level: 3, description: 'Choose 2 Metamagic options (more at higher levels): Careful, Distant, Empowered, Extended, Heightened, Quickened, Subtle, Twinned.' },
    ],
    subclasses: {
      'Draconic Bloodline': [
        { name: 'Dragon Ancestor', level: 1, description: 'Choose a dragon type (acid, cold, fire, lightning, poison). You speak Draconic and gain its damage affinity for other features.' },
        { name: 'Draconic Resilience', level: 1, description: '+1 HP per sorcerer level. When not wearing armor, AC = 13 + DEX mod.' },
        { name: 'Elemental Affinity', level: 6, description: 'Spells matching your ancestor\'s damage type add CHA mod to damage. Spend 1 sorcery point for resistance to that type for 1 hour.' },
        { name: 'Dragon Wings', level: 14, description: 'Sprout draconic wings, gaining a fly speed equal to your walking speed.' },
        { name: 'Draconic Presence', level: 18, description: 'Spend 5 sorcery points: awe or fear aura (60 ft, WIS save) for 1 minute. Concentration.', uses: { max: '5 sorcery points', reset: 'short' } },
      ],
    },
  },

  Warlock: {
    base: [
      { name: 'Pact Magic', level: 1, description: 'Short-rest spell slots (1 slot at level 1, 2 slots at level 2, up to 5th-level slots by level 9). Recharge on short rest.', uses: { max: '1–4 slots', reset: 'short' } },
      { name: 'Eldritch Invocations', level: 2, description: 'Learn 2 Eldritch Invocations that modify or expand your abilities (gain more at higher levels).' },
      { name: 'Mystic Arcanum', level: 11, description: 'Once per long rest, cast one 6th-level spell without a slot (7th at 13, 8th at 15, 9th at 17).', uses: { max: '1', reset: 'long' } },
    ],
    subclasses: {
      'The Fiend': [
        { name: "Dark One's Blessing", level: 1, description: 'When you reduce a creature to 0 HP, gain CHA mod + warlock level temporary HP.' },
        { name: "Dark One's Own Luck", level: 6, description: 'Add 1d10 to one ability check or saving throw after rolling. Once per short rest.', uses: { max: '1', reset: 'short' } },
        { name: 'Fiendish Resilience', level: 10, description: 'After a short rest, choose one damage type to be resistant to until your next rest.' },
        { name: 'Hurl Through Hell', level: 14, description: 'On a hit, banish target through the lower planes (returns at end of next turn). Non-fiends take 10d10 psychic on return. Once per long rest.', uses: { max: '1', reset: 'long' } },
      ],
    },
  },

  Wizard: {
    base: [
      { name: 'Arcane Recovery', level: 1, description: 'Once per long rest (after a short rest): recover spell slots with combined level ≤ half wizard level (max 5th level each).', uses: { max: '1', reset: 'long' } },
      { name: 'Spell Mastery', level: 18, description: 'Choose one 1st-level and one 2nd-level spell: cast them at lowest level without a spell slot (after a long rest to switch).' },
      { name: 'Signature Spells', level: 20, description: 'Two 3rd-level wizard spells are always prepared; cast each once per short rest without a slot.', uses: { max: '1 each', reset: 'short' } },
    ],
    subclasses: {
      'School of Evocation': [
        { name: 'Evocation Savant', level: 2, description: 'Copy evocation spells into your spellbook at half the normal gold cost and time.' },
        { name: 'Sculpt Spells', level: 2, description: 'Choose up to 1 + spell level creatures to automatically succeed their save and take no damage from your evocation spells.' },
        { name: 'Potent Cantrip', level: 6, description: 'Targets that succeed their save against your damage cantrips still take half damage.' },
        { name: 'Empowered Evocation', level: 10, description: 'Add your INT modifier to the damage of any wizard evocation spell you cast.' },
        { name: 'Overchannel', level: 14, description: 'Deal maximum damage with a wizard evocation spell of 5th level or lower. Reusing before a long rest deals 2d12 necrotic per spell level (escalates each reuse).' },
      ],
      'School of Divination': [
        { name: 'Divination Savant', level: 2, description: 'Copy divination spells at half gold cost and time.' },
        { name: 'Portent', level: 2, description: 'Roll 2d20 after a long rest; replace any attack, check, or save with one of these rolls (before or after rolling).', uses: { max: '2 dice', reset: 'long' } },
        { name: 'Expert Divination', level: 6, description: 'When you cast a divination spell of 2nd level or higher, regain one expended spell slot of a lower level.' },
        { name: 'The Third Eye', level: 10, description: 'Action: gain one of: Darkvision 60 ft, ethereal sight (see the Ethereal Plane), greater comprehension (read any language), or see invisibility. Until your next rest.', uses: { max: '1', reset: 'short' } },
        { name: 'Greater Portent', level: 14, description: 'Portent now gives 3 dice per long rest instead of 2.', uses: { max: '3 dice', reset: 'long' } },
      ],
      'School of Abjuration': [
        { name: 'Abjuration Savant', level: 2, description: 'Copy abjuration spells at half gold cost and time.' },
        { name: 'Arcane Ward', level: 2, description: 'When you cast an abjuration spell of 1st level or higher, create a ward with HP = 2× wizard level + INT mod. Absorbs damage before you. Recharge by casting abjuration spells.' },
        { name: 'Projected Ward', level: 6, description: 'When a creature within 30 ft takes damage, use your reaction to have your Arcane Ward absorb it instead.' },
        { name: 'Improved Abjuration', level: 10, description: 'Add your proficiency bonus to the result of your ability checks made to Counterspell or Dispel Magic.' },
        { name: 'Spell Resistance', level: 14, description: 'Advantage on saving throws against spells. Resistance to spell damage.' },
      ],
    },
  },
};

export function getClassDef(className: string): ClassDef | undefined {
  const key = Object.keys(CLASS_DATA).find(
    (k) => k.toLowerCase() === className.toLowerCase().trim(),
  );
  return key ? CLASS_DATA[key] : undefined;
}

export function getSubclassFeatures(className: string, subclassName: string): SubclassFeature[] {
  const cls = getClassDef(className);
  if (!cls) return [];
  const key = Object.keys(cls.subclasses).find(
    (k) => k.toLowerCase() === subclassName.toLowerCase().trim(),
  );
  return key ? cls.subclasses[key] : [];
}

export function getBaseFeatures(className: string): SubclassFeature[] {
  return getClassDef(className)?.base ?? [];
}

export function listSubclasses(className: string): string[] {
  const cls = getClassDef(className);
  if (!cls) return [];
  return Object.keys(cls.subclasses);
}

export const ALL_CLASS_NAMES = Object.keys(CLASS_DATA);
