/**
 * Single source of truth for D&D 5e class names used app-wide:
 *  - datalist autocomplete on the character sheet
 *  - spell-book class filter dropdowns
 *  - caster-progression classification
 *
 * Adding/removing entries here propagates to every UI consumer.
 */

export const DND_CLASSES = [
  'Artificer',
  'Barbarian',
  'Bard',
  'Blood Hunter',
  'Cleric',
  'Druid',
  'Fighter',
  'Monk',
  'Paladin',
  'Ranger',
  'Rogue',
  'Sorcerer',
  'Warlock',
  'Wizard',
] as const;

export type DndClass = typeof DND_CLASSES[number];

/** Full spellcasting classes (1→9 spell levels on full-caster table). */
export const FULL_CASTER_CLASSES = ['Bard','Cleric','Druid','Sorcerer','Wizard'] as const;

/** Half-casters (Paladin, Ranger). */
export const HALF_CASTER_CLASSES = ['Paladin','Ranger'] as const;

/** Unique Warlock pact magic progression. */
export const WARLOCK_CLASS = 'Warlock' as const;

/** Classes whose magical subclass makes them third-casters. */
export const THIRD_CASTER_PARENT_CLASSES = ['Fighter','Rogue'] as const;

/** Classes that have a standard 5e spell list (used by spell-book class filter). */
export const SPELLCASTER_CLASSES = [
  ...FULL_CASTER_CLASSES,
  ...HALF_CASTER_CLASSES,
  WARLOCK_CLASS,
] as const;

/** Canonical lowercase set — used for `isCustomClass()` lookups. */
export const STANDARD_CLASS_NAMES_LC: ReadonlySet<string> = new Set(
  DND_CLASSES.map((c) => c.toLowerCase()),
);

export function isCustomClass(name: string): boolean {
  return !STANDARD_CLASS_NAMES_LC.has(name.trim().toLowerCase());
}

export const HIT_DIE: Readonly<Record<string, string>> = {
  Artificer: 'd8',
  Barbarian: 'd12',
  Bard: 'd8',
  'Blood Hunter': 'd10',
  Cleric: 'd8',
  Druid: 'd8',
  Fighter: 'd10',
  Monk: 'd8',
  Paladin: 'd10',
  Ranger: 'd10',
  Rogue: 'd8',
  Sorcerer: 'd6',
  Warlock: 'd8',
  Wizard: 'd6',
};

export function hitDieFor(className: string): string {
  const key = Object.keys(HIT_DIE).find(
    (k) => k.toLowerCase() === className.trim().toLowerCase(),
  );
  return key ? HIT_DIE[key] : 'd8';
}

/** A subclass option with its source sourcebook (for display only). */
export type SubclassOption = { name: string; source?: string };

/**
 * Master subclass list — compiled from PHB (2014 + 2024), XGtE, TCoE, SCAG,
 * MToF, VRGtR, EGtW, ERLW, EFotA, FToD, Mythic Odysseys, Bigby Presents,
 * Valda's Spire of Secrets, GHPG, CBT, TSCSR, Plane Shift, plus UA drops.
 * Free-form entries still allowed via the autocomplete's "use X" option.
 */
export const DND_SUBCLASSES: Record<DndClass, readonly SubclassOption[]> = {
  Artificer: [
    { name: 'Alchemist',     source: 'ERLW, 2019' },
    { name: 'Armorer',       source: 'TCoE, 2020' },
    { name: 'Artillerist',   source: 'ERLW, 2019' },
    { name: 'Battle Smith',  source: 'ERLW, 2019' },
    { name: 'Cartographer',  source: 'EFotA, 2025' },
    { name: 'Reanimator',    source: 'UA: Horror, 2025' },
    { name: 'Gunsmith',      source: "Valda's Spire of Secrets, 2022" },
  ],
  Barbarian: [
    { name: 'Berserker',                 source: 'PHB, 2014/2024' },
    { name: 'Wild Heart',                source: 'TCoE, 2020' },
    { name: 'Zealot',                    source: 'XGtE, 2017' },
    { name: 'World Tree',                source: 'UA, 2023' },
    { name: 'Ancestral Guardian',        source: 'XGtE, 2017' },
    { name: 'Totem Warrior',             source: 'PHB, 2014' },
    { name: 'Storm Herald',              source: 'XGtE, 2017' },
    { name: 'Battlerager',               source: 'SCAG/XGtE, 2015' },
    { name: 'Path of the Beast',         source: 'TCoE, 2020' },
    { name: 'Path of Wild Magic',        source: 'TCoE, 2020' },
    { name: 'Path of the Giant',         source: 'Bigby Presents, 2023' },
    { name: 'Path of the Carrion Raven', source: 'GHPG, 2024' },
    { name: 'Path of the Spell Scorned', source: 'CBT, 2025' },
    { name: 'Juggernaut',                source: 'TSCSR, 2020' },
  ],
  Bard: [
    { name: 'College of Lore',      source: 'PHB, 2014' },
    { name: 'College of Valor',     source: 'PHB, 2014' },
    { name: 'College of Swords',    source: 'XGtE, 2017' },
    { name: 'College of Glamour',   source: 'XGtE, 2017' },
    { name: 'College of Dance',     source: 'PHB, 2024' },
    { name: 'College of Whispers',  source: 'XGtE, 2017' },
    { name: 'College of Creation',  source: 'TCoE, 2020' },
    { name: 'College of Eloquence', source: 'Mythic Odysseys, 2023' },
    { name: 'College of Spirits',   source: 'UA, 2017' },
    { name: 'College of the Moon',  source: 'UA' },
    { name: 'College of Requiems',  source: 'GHPG, 2024' },
    { name: 'College of Drama',     source: 'CBT, 2025' },
    { name: 'College of Tragedy',   source: 'TSCSR, 2020' },
  ],
  'Blood Hunter': [
    { name: 'Order of the Ghostslayer', source: 'Critical Role, 2020' },
    { name: 'Order of the Lycan',       source: 'Critical Role, 2020' },
    { name: 'Order of the Mutant',      source: 'Critical Role, 2020' },
    { name: 'Order of the Profane Soul', source: 'Critical Role, 2020' },
  ],
  Cleric: [
    { name: 'Life',        source: 'PHB, 2014' },
    { name: 'Light',       source: 'PHB, 2014' },
    { name: 'Trickery',    source: 'PHB, 2014' },
    { name: 'War',         source: 'PHB, 2014' },
    { name: 'Arcana',      source: 'SCAG, 2015' },
    { name: 'Tempest',     source: 'PHB, 2014' },
    { name: 'Nature',      source: 'PHB, 2014' },
    { name: 'Grave',       source: 'XGtE, 2017' },
    { name: 'Forge',       source: 'XGtE, 2017' },
    { name: 'Order',       source: 'TCoE, 2020' },
    { name: 'Solidarity',  source: 'Plane Shift, 2016' },
    { name: 'Unity',       source: 'UA, 2020' },
    { name: 'City',        source: 'Plane Shift, 2016' },
    { name: 'Peace',       source: 'TCoE, 2020' },
    { name: 'Knowledge',   source: 'PHB, 2014' },
    { name: 'Protection',  source: 'UA, 2016' },
    { name: 'Twilight',    source: 'TCoE, 2020' },
    { name: 'Death',       source: 'DMG, 2014' },
    { name: 'Mind',        source: 'EE, 2019' },
    { name: 'Strength',    source: 'Plane Shift, 2017' },
    { name: 'Inquisition Domain', source: 'GHPG, 2024' },
    { name: 'Apocalypse Domain',  source: 'CBT, 2025' },
    { name: 'Blood Domain',       source: 'TSCSR, 2020' },
    { name: 'Moon Domain',        source: 'TSCSR, 2020' },
  ],
  Druid: [
    { name: 'Circle of the Land',      source: 'PHB, 2014' },
    { name: 'Circle of the Moon',      source: 'PHB, 2014' },
    { name: 'Circle of the Sea',       source: 'UA, 2017' },
    { name: 'Circle of the Stars',     source: 'TCoE, 2020' },
    { name: 'Circle of Spores',        source: 'UA, 2017' },
    { name: 'Circle of Dreams',        source: 'TCoE, 2020' },
    { name: 'Circle of Wildfire',      source: 'TCoE, 2020' },
    { name: 'Circle of the Shepherd',  source: 'UA, 2017' },
    { name: 'Circle of the Primeval',  source: 'UA' },
    { name: 'Circle of Preservation',  source: 'UA, 2017' },
    { name: 'Circle of Twilight',      source: 'UA' },
    { name: 'Circle of the Symbiote',  source: 'CBT, 2025' },
    { name: 'Circle of the Blighted',  source: 'TSCSR, 2020' },
  ],
  Fighter: [
    { name: 'Champion',             source: 'PHB, 2014' },
    { name: 'Battle Master',        source: 'PHB, 2014' },
    { name: 'Eldritch Knight',      source: 'PHB, 2014' },
    { name: 'Purple Dragon Knight', source: 'SCAG/XGtE, 2015' },
    { name: 'Cavalier',             source: 'XGtE, 2017' },
    { name: 'Rune Knight',          source: 'TCoE, 2020' },
    { name: 'Samurai',              source: 'XGtE, 2017' },
    { name: 'Arcane Archer',        source: 'UA, 2017' },
    { name: 'Echo Knight',          source: 'EGtW, 2020' },
    { name: 'Gladiator',            source: 'UA, 2016' },
    { name: 'Brawler',              source: 'UA, 2016' },
    { name: 'Brute',                source: 'UA, 2018' },
    { name: 'Psi Warrior',          source: 'TCoE, 2020' },
    { name: 'Banneret',             source: 'TCoE, 2020' },
    { name: 'Gunslinger',           source: "Valda's Spire of Secrets, 2022" },
    { name: 'Blade Breaker',        source: 'GHPG, 2024' },
    { name: 'Hero',                 source: 'CBT, 2025' },
    { name: 'Myrmidon',             source: 'Homebrew' },
  ],
  Monk: [
    { name: 'Way of the Open Hand',                  source: 'PHB, 2014' },
    { name: 'Way of Shadow',                         source: 'PHB, 2014' },
    { name: 'Way of the Four Elements',              source: 'PHB, 2014' },
    { name: 'Way of the Drunken Master',             source: 'XGtE, 2017' },
    { name: 'Way of the Kensei',                     source: 'XGtE, 2017' },
    { name: 'Way of the Long Death',                 source: 'XGtE, 2017' },
    { name: 'Way of the Sun Soul',                   source: 'XGtE, 2017' },
    { name: 'Way of the Tattooed Warrior',           source: 'UA, 2017' },
    { name: 'Way of the Astral Self',                source: 'TCoE, 2020' },
    { name: 'Way of the Living Weapon',              source: 'EE, 2025' },
    { name: 'Way of the Ascended Dragon',            source: 'FToD, 2021' },
    { name: 'Way of the Warrior of Cosmic Balance',  source: 'CBT, 2025' },
    { name: 'Way of the Cobalt Soul',                source: 'TSCSR, 2020' },
    { name: 'Way of Mercy',                          source: 'UA, 2017' },
    { name: 'Way of Tranquility',                    source: 'UA, 2016' },
  ],
  Paladin: [
    { name: 'Oath of Devotion',     source: 'PHB, 2014' },
    { name: 'Oath of the Ancients', source: 'PHB, 2014' },
    { name: 'Oath of Vengeance',    source: 'PHB, 2014' },
    { name: 'Oath of Conquest',     source: 'XGtE, 2017' },
    { name: 'Oath of Redemption',   source: 'XGtE, 2017' },
    { name: 'Oath of the Crown',    source: 'SCAG, 2015' },
    { name: 'Oath of the Watchers', source: 'UA, 2017' },
    { name: 'Oath of Glory',        source: 'Mythic Odysseys, 2023' },
    { name: 'Oath of Treachery',    source: 'UA, 2016' },
    { name: 'Oath of Heroism',      source: 'Mythic Odysseys, 2023' },
    { name: 'Oathbreaker',          source: 'DMG, 2014' },
    { name: 'Oath of Zeal',         source: 'GHPG, 2024' },
    { name: 'Oath of the Guardian', source: 'CBT, 2025' },
    { name: 'Oath of the Open Sea', source: 'TSCSR, 2020' },
  ],
  Ranger: [
    { name: 'Hunter',              source: 'PHB, 2014' },
    { name: 'Beast Master',        source: 'PHB, 2014' },
    { name: 'Gloom Stalker',       source: 'XGtE, 2017' },
    { name: 'Fey Wanderer',        source: 'TCoE, 2020' },
    { name: 'Horizon Walker',      source: 'XGtE, 2017' },
    { name: 'Swarmkeeper',         source: 'TCoE, 2020' },
    { name: 'Monster Slayer',      source: 'XGtE, 2017' },
    { name: 'Primeval Guardian',   source: 'UA, 2017' },
    { name: 'Drakewarden',         source: 'FToD, 2021' },
    { name: 'Hollow Warden',       source: 'UA, 2017' },
    { name: 'Winter Walker',       source: 'UA, 2017' },
    { name: 'Scout',               source: 'PHB, 2014' },
    { name: 'Trail Warden',        source: 'CBT, 2025' },
  ],
  Rogue: [
    { name: 'Thief',              source: 'PHB, 2014' },
    { name: 'Assassin',           source: 'PHB, 2014' },
    { name: 'Arcane Trickster',   source: 'PHB, 2014' },
    { name: 'Swashbuckler',       source: 'SCAG/XGtE, 2015' },
    { name: 'Phantom',            source: 'UA, 2017' },
    { name: 'Soulknife',          source: 'TCoE, 2020' },
    { name: 'Scout',              source: 'XGtE, 2017' },
    { name: 'Mastermind',         source: 'XGtE, 2017' },
    { name: 'Inquisitive',        source: 'XGtE, 2017' },
    { name: 'Scion of the Three', source: 'UA, 2016' },
    { name: 'Gambler',            source: 'UA, 2010' },
    { name: 'Acrobat',            source: 'UA, 2016' },
    { name: 'Spy',                source: 'UA, 2015' },
    { name: 'Ruffian',            source: 'UA, 2010' },
    { name: 'Cutpurse',           source: 'UA, 2010' },
    { name: 'Sapper',             source: 'UA, 2010' },
    { name: 'Fixer',              source: 'UA, 2010' },
    { name: 'Misfortune Bringer', source: 'GHPG, 2024' },
    { name: 'Shadow Stalker',     source: 'CBT, 2025' },
  ],
  Sorcerer: [
    { name: 'Aberrant Mind',     source: 'TCoE, 2020' },
    { name: 'Blood Sorcery',     source: 'UA, 2016' },
    { name: 'Clockwork Soul',    source: 'TCoE, 2020' },
    { name: 'Defiled Sorcerer',  source: 'UA: Apocalyptic, 2025' },
    { name: 'Divine Soul',       source: 'XGtE, 2017' },
    { name: 'Draconic',          source: 'PHB, 2014' },
    { name: 'Elementalist',      source: 'UA, 2016' },
    { name: 'Hungering Dark',    source: 'CBT, 2025' },
    { name: 'Lunar Sorcery',     source: 'UA, 2016' },
    { name: 'Phoenix',           source: 'UA, 2016' },
    { name: 'Protean',           source: 'UA, 2016' },
    { name: 'Runechild',         source: 'UA, 2016' },
    { name: 'Seer',              source: 'UA, 2016' },
    { name: 'Shadow',            source: 'UA, 2015' },
    { name: 'Shadow Magic',      source: 'UA, 2015' },
    { name: 'Spellfire Sorcery', source: 'UA: Forgotten Realms, 2025' },
    { name: 'Stone Sorcery',     source: 'UA, 2016' },
    { name: 'Storm Sorcery',     source: 'XGtE, 2017' },
    { name: 'Wild Magic',        source: 'PHB, 2014' },
  ],
  Warlock: [
    { name: 'Fiend',                                source: 'PHB, 2014' },
    { name: 'Archfey',                              source: 'PHB, 2014' },
    { name: 'Great Old One',                        source: 'PHB, 2014' },
    { name: 'Celestial',                            source: 'XGtE, 2017' },
    { name: 'Hexblade',                             source: 'XGtE, 2017' },
    { name: 'Undying',                              source: 'SCAG, 2015' },
    { name: 'Undead',                               source: "Van Richten's Guide, 2021" },
    { name: 'The Noble Genie',                      source: 'TCoE, 2020' },
    { name: 'The Fathomless',                       source: 'TCoE, 2020' },
    { name: 'Sorcerer-King',                        source: 'UA: Apocalyptic, 2025' },
    { name: 'The Raven Queen',                      source: 'UA, 2017' },
    { name: 'Sea Lord',                             source: 'UA, 2017' },
    { name: "Winter's Night",                       source: 'UA, 2017' },
    { name: 'Pale Master',                          source: 'UA: Horror, 2025' },
    { name: 'Dreamer',                              source: 'UA, 2017' },
    { name: 'Lurker',                               source: 'UA, 2017' },
    { name: 'Witch',                                source: 'UA, 2017' },
    { name: 'Exalted Assembly of the Feline Court', source: 'CBT, 2025' },
  ],
  Wizard: [
    { name: 'Abjurer',         source: 'PHB, 2014' },
    { name: 'Artificer',       source: 'UA Eberron, 2015' },
    { name: 'Bibliomancy',     source: 'CBT, 2025' },
    { name: 'Bladesinger',     source: 'SCAG/XGtE, 2015' },
    { name: 'Blood Magic',     source: 'TSCSR, 2020' },
    { name: 'Chronurgy',       source: 'EGtW, 2020' },
    { name: 'Conjurer',        source: 'PHB, 2014' },
    { name: 'Diviner',         source: 'PHB, 2014' },
    { name: 'Enchanter',       source: 'PHB, 2014' },
    { name: 'Evoker',          source: 'PHB, 2014' },
    { name: 'Elementalist',    source: 'UA, 2016' },
    { name: 'Geomancer',       source: 'UA, 2017' },
    { name: 'Graviturgy',      source: 'EGtW, 2020' },
    { name: 'Illusionist',     source: 'PHB, 2014' },
    { name: 'Mentalist',       source: 'UA, 2017' },
    { name: 'Necromancer',     source: 'PHB, 2014' },
    { name: 'Order of Scribes', source: 'TCoE, 2020' },
    { name: 'Pyromancer',      source: 'UA, 2017' },
    { name: 'School of Lore',  source: 'UA, 2015' },
    { name: 'Shadowmage',      source: 'UA, 2015' },
    { name: 'Thaumaturge',     source: 'UA, 2017' },
    { name: 'Transmuter',      source: 'PHB, 2014' },
    { name: 'War Magic',       source: 'XGtE, 2017' },
    { name: 'Witchcraft',      source: 'UA, 2017' },
  ],
};

/** Options for the given class name (case-insensitive match); empty if custom. */
export function subclassesFor(className: string | undefined | null): readonly SubclassOption[] {
  if (!className) return [];
  const key = className.trim();
  if (!key) return [];
  const match = DND_CLASSES.find((c) => c.toLowerCase() === key.toLowerCase());
  return match ? DND_SUBCLASSES[match] : [];
}
