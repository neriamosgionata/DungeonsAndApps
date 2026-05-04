/**
 * Class-default trackable resources (ki, sorcery points, rages, etc.).
 * When a class is added to a character the character page auto-seeds the
 * matching rows here (scaled by level). Rows the user created manually are
 * preserved; a class template is applied at most once per (class, resource).
 */

import type { DndClass } from './classes';

export type ResetKind = 'short' | 'long' | 'none';
export type ResourceTemplate = {
  /** Display name written into sheet.resources[].name */
  name: string;
  /** Refresh trigger. */
  reset: ResetKind;
  /** max at 0 = "don't create yet". */
  maxFor: (level: number) => number;
  /** Minimum class level at which the resource is relevant. */
  minLevel?: number;
};

/** "Rage" count by Barbarian level (PHB). Unlimited at 20 → display ∞ as 99. */
function barbarianRages(L: number): number {
  if (L >= 20) return 99;
  if (L >= 17) return 6;
  if (L >= 12) return 5;
  if (L >= 6)  return 4;
  if (L >= 3)  return 3;
  if (L >= 1)  return 2;
  return 0;
}

/** Sorcery Points = sorcerer level (min 2). */
function sorceryPoints(L: number): number {
  return L >= 2 ? L : 0;
}

/** Superiority Dice (Battle Master) — subclass-gated; best-effort table. */
function superiorityDice(L: number): number {
  if (L >= 15) return 6;
  if (L >= 7)  return 5;
  if (L >= 3)  return 4;
  return 0;
}

/** Bardic Inspiration uses by bard level. */
function bardicInspiration(charismaMod: number, _L: number): number {
  return Math.max(1, charismaMod);
}

/** Channel Divinity (Cleric & Paladin shared template). */
function channelDivinity(L: number): number {
  if (L >= 18) return 3;
  if (L >= 6)  return 2;
  if (L >= 2)  return 1;
  return 0;
}

/** Mystic Arcanum: warlock single-use spells per day (11+, 13+, 15+, 17+). */

export const RESOURCES_BY_CLASS: Record<DndClass, ResourceTemplate[]> = {
  Artificer: [
    { name: 'Infusions Known',    reset: 'long',  maxFor: (L) => (L >= 18 ? 6 : L >= 14 ? 5 : L >= 10 ? 4 : L >= 6 ? 3 : L >= 2 ? 2 : 0), minLevel: 2 },
    { name: 'Infused Items',      reset: 'long',  maxFor: (L) => (L >= 18 ? 6 : L >= 14 ? 5 : L >= 10 ? 4 : L >= 6 ? 3 : L >= 2 ? 2 : 0), minLevel: 2 },
  ],
  Barbarian: [
    { name: 'Rages',              reset: 'long',  maxFor: barbarianRages },
  ],
  Bard: [
    { name: 'Bardic Inspiration', reset: 'short', maxFor: (L) => bardicInspiration(0, L) },
  ],
  'Blood Hunter': [
    { name: 'Crimson Rite',       reset: 'short', maxFor: () => 1 },
  ],
  Cleric: [
    { name: 'Channel Divinity',   reset: 'short', maxFor: channelDivinity, minLevel: 2 },
    { name: 'Divine Intervention',reset: 'long',  maxFor: (L) => (L >= 10 ? 1 : 0), minLevel: 10 },
    { name: 'War Priest',         reset: 'short', maxFor: (L) => (L >= 1 ? 1 : 0) },
  ],
  Druid: [
    { name: 'Wild Shape',         reset: 'short', maxFor: (L) => (L >= 20 ? 99 : L >= 2 ? 2 : 0), minLevel: 2 },
    { name: 'Natural Recovery',   reset: 'long',  maxFor: (L) => (L >= 2 ? 1 : 0), minLevel: 2 },
  ],
  Fighter: [
    { name: 'Second Wind',        reset: 'short', maxFor: (L) => (L >= 1 ? 1 : 0) },
    { name: 'Action Surge',       reset: 'short', maxFor: (L) => (L >= 17 ? 2 : L >= 2 ? 1 : 0), minLevel: 2 },
    { name: 'Indomitable',        reset: 'long',  maxFor: (L) => (L >= 17 ? 3 : L >= 13 ? 2 : L >= 9 ? 1 : 0), minLevel: 9 },
    { name: 'Superiority Dice',   reset: 'short', maxFor: superiorityDice, minLevel: 3 },
  ],
  Monk: [
    { name: 'Ki',                 reset: 'short', maxFor: (L) => (L >= 2 ? L : 0), minLevel: 2 },
    { name: 'Perfect Self',       reset: 'long',  maxFor: (L) => (L >= 20 ? 4 : 0), minLevel: 20 },
  ],
  Paladin: [
    { name: 'Channel Divinity',   reset: 'short', maxFor: channelDivinity, minLevel: 3 },
    { name: 'Lay on Hands Pool',  reset: 'long',  maxFor: (L) => L * 5 },
    { name: 'Cleansing Touch',    reset: 'long',  maxFor: (L) => (L >= 6 ? 1 : 0), minLevel: 6 },
  ],
  Ranger: [
    // Handled by subclass; seed only favored foe if Gloom Stalker etc. — skip for now.
  ],
  Rogue: [
    { name: 'Stroke of Luck',     reset: 'long',  maxFor: (L) => (L >= 20 ? 1 : 0), minLevel: 20 },
  ],
  Sorcerer: [
    { name: 'Sorcery Points',         reset: 'long',  maxFor: sorceryPoints, minLevel: 2 },
    { name: 'Sorcerous Restoration',  reset: 'short', maxFor: (L) => (L >= 20 ? 4 : 0), minLevel: 20 },
  ],
  Warlock: [
    { name: 'Eldritch Invocations', reset: 'none', maxFor: (L) => (L >= 18 ? 8 : L >= 15 ? 7 : L >= 12 ? 6 : L >= 9 ? 5 : L >= 7 ? 4 : L >= 5 ? 3 : L >= 2 ? 2 : 0), minLevel: 2 },
    { name: 'Mystic Arcanum (6th)', reset: 'long', maxFor: (L) => (L >= 11 ? 1 : 0), minLevel: 11 },
    { name: 'Mystic Arcanum (7th)', reset: 'long', maxFor: (L) => (L >= 13 ? 1 : 0), minLevel: 13 },
    { name: 'Mystic Arcanum (8th)', reset: 'long', maxFor: (L) => (L >= 15 ? 1 : 0), minLevel: 15 },
    { name: 'Mystic Arcanum (9th)', reset: 'long', maxFor: (L) => (L >= 17 ? 1 : 0), minLevel: 17 },
  ],
  Wizard: [
    { name: 'Arcane Recovery',    reset: 'long',  maxFor: (L) => (L >= 1 ? 1 : 0) },
    { name: 'Spell Mastery',      reset: 'long',  maxFor: (L) => (L >= 18 ? 1 : 0), minLevel: 18 },
  ],
};

/** Case-insensitive match against the standard class list. */
export function templatesForClass(name: string): readonly ResourceTemplate[] {
  const key = name.trim().toLowerCase();
  for (const [cls, list] of Object.entries(RESOURCES_BY_CLASS)) {
    if (cls.toLowerCase() === key) return list;
  }
  return [];
}
