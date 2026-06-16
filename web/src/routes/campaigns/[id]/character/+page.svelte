<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { Characters, Campaigns, Spells, Dice } from '$lib/api/resources';
  import type { Spell } from '$lib/types';
  import { campaignSocket } from '$lib/ws.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  const campaign = useCampaign();
  import Stepper from '$lib/components/Stepper.svelte';
  import SlotTrack from '$lib/components/SlotTrack.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import CoinPurse from '$lib/components/CoinPurse.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import CharacterOnboarding from '$lib/components/CharacterOnboarding.svelte';
  import { _ } from 'svelte-i18n';
  import { Trash2, Sparkles, Star, ChevronLeft, ChevronRight, BookOpen, Plus, Zap, Search, Swords, Skull, Heart, Bed, Moon, Brain, X } from '@lucide/svelte';
  import { DND_CLASSES, SPELLCASTER_CLASSES, isCustomClass as isCustomClassShared, hitDieFor } from '$lib/dnd/classes';
  import { templatesForClass } from '$lib/dnd/resources';
  import { FEATS, featByKey, featPrereqsMet, type Feat, type Ability } from '$lib/feats';
  import { randomUUID } from '$lib/uuid';
  import { getBaseFeatures, getSubclassFeatures, listSubclasses, ALL_CLASS_NAMES } from '$lib/dnd/subclasses';
  import { ITEMS, itemsByCategory as itemsByCat } from '$lib/dnd/items';

  /** Names of class-default resources for a character (case-insensitive). */
  function classResourceNames(c: Character): Set<string> {
    const out = new Set<string>();
    for (const cl of c.sheet?.classes ?? []) {
      if (!cl.name?.trim()) continue;
      for (const tpl of templatesForClass(cl.name)) out.add(tpl.name.toLowerCase());
    }
    return out;
  }
  import ClassAutocomplete from '$lib/components/ClassAutocomplete.svelte';
  import SubclassAutocomplete from '$lib/components/SubclassAutocomplete.svelte';

  type CharSpell = {
    // one of slug (book) or custom name
    slug?: string;
    name: string;
    level: number;          // 0..9 (0 = cantrip)
    school?: string;
    prepared?: boolean;
    custom?: boolean;
    description?: string;
    classes?: string[];
    ritual?: boolean;
    concentration?: boolean;
    casting_time?: string | null;
    range_text?: string | null;
    components?: string | null;
    duration?: string | null;
    higher_levels?: string | null;
    source?: string | null;
  };

  type Ability = 'str' | 'dex' | 'con' | 'int' | 'wis' | 'cha';
  const ABILITIES: Ability[] = ['str','dex','con','int','wis','cha'];
  const CONDITIONS_LIST = ['blinded','charmed','deafened','exhaustion','frightened','grappled','incapacitated','invisible','paralyzed','petrified','poisoned','prone','restrained','stunned','unconscious'] as const;
  const DAMAGE_TYPES = ['acid','bludgeoning','cold','fire','force','lightning','necrotic','piercing','poison','psychic','radiant','slashing','thunder'] as const;
  type DamageType = typeof DAMAGE_TYPES[number];
  const DAMAGE_CATEGORY_KEYS = ['resistances','vulnerabilities','immunities'] as const;
  type Skill = { key: string; label: string; ability: Ability };
  const SKILLS: Skill[] = [
    { key: 'acrobatics',      label: 'Acrobatics',      ability: 'dex' },
    { key: 'animal_handling', label: 'Animal Handling', ability: 'wis' },
    { key: 'arcana',          label: 'Arcana',          ability: 'int' },
    { key: 'athletics',       label: 'Athletics',       ability: 'str' },
    { key: 'deception',       label: 'Deception',       ability: 'cha' },
    { key: 'history',         label: 'History',         ability: 'int' },
    { key: 'insight',         label: 'Insight',         ability: 'wis' },
    { key: 'intimidation',    label: 'Intimidation',    ability: 'cha' },
    { key: 'investigation',   label: 'Investigation',   ability: 'int' },
    { key: 'medicine',        label: 'Medicine',        ability: 'wis' },
    { key: 'nature',          label: 'Nature',          ability: 'int' },
    { key: 'perception',      label: 'Perception',      ability: 'wis' },
    { key: 'performance',     label: 'Performance',     ability: 'cha' },
    { key: 'persuasion',      label: 'Persuasion',      ability: 'cha' },
    { key: 'religion',        label: 'Religion',        ability: 'int' },
    { key: 'sleight_of_hand', label: 'Sleight of Hand', ability: 'dex' },
    { key: 'stealth',         label: 'Stealth',         ability: 'dex' },
    { key: 'survival',        label: 'Survival',        ability: 'wis' },
  ];

  type ArmorType = 'light' | 'medium' | 'heavy' | 'unarmored_barbarian' | 'unarmored_monk' | 'mage_armor' | 'natural' | 'draconic';
  type ToolProf = { name: string; ability?: Ability; proficient?: boolean; expert?: boolean };

  type Sheet = {
    hp?: { current?: number; max?: number; temp?: number };
    hit_dice?: { current?: number; max?: number; die?: string; pools?: Array<{ name: string; die: string; current: number; max: number }> };
    ac?: number;
    initiative?: number;
    speed?: number;
    alive?: boolean;
    death_saves?: { successes?: number; failures?: number };
    abilities?: { str?: number; dex?: number; con?: number; int?: number; wis?: number; cha?: number };
    /** Saving-throw proficiencies — keyed by ability. */
    saves?: Partial<Record<Ability, boolean>>;
    saves_override?: Partial<Record<Ability, number>>;
    abilities_override?: Partial<Record<Ability, number>>;
    /** Skill proficiencies: key → 'none' (default) | 'prof' | 'expert'. */
    skills?: Record<string, 'prof' | 'expert'>;
    senses?: { darkvision?: number; blindsight?: number; truesight?: number; tremorsense?: number; passive_perception_bonus?: number };
    languages?: string;
    proficiencies?: { armor?: string; weapons?: string; tools?: string };
    tool_proficiencies?: ToolProf[];
    features?: Array<{ id: string; name: string; source?: string; description?: string; uses?: { current: number; max: number; reset?: 'short' | 'long' | 'none' } }>;
    classes?: Array<{ id: string; name: string; level: number; subclass?: string; spellcasting_ability?: Ability; hit_die?: string }>;
    resources?: Array<{ id: string; name: string; current: number; max: number; reset?: 'short' | 'long' | 'none' }>;
    attunement?: Array<{
      id: string; name: string; notes?: string;
      description?: string;
      bonuses?: {
        ac?: number; speed?: number; initiative?: number;
        attack?: number; damage?: number; spell_dc?: number;
        str?: number; dex?: number; con?: number; int?: number; wis?: number; cha?: number;
      };
      charges?: { current: number; max: number; reset: 'dawn'|'dusk'|'long'|'short'|'none'; recharge_die?: string };
      spell_slots?: Record<string, { current: number; max: number }>;
    }>;
    feats?: Array<{ id: string; key: string; config?: { ability?: string; class_name?: string; damage_type?: string } }>;
    concentration?: { spell?: string; since?: string } | null;
    active_effects?: Array<{ id: string; spell: string; duration?: string | null; since?: string }>;
    xp?: number;
    slots?: Record<string, { current: number; max: number }>;
    inspiration?: boolean;
    exhaustion?: number;
    coin?: { cp?: number; sp?: number; ep?: number; gp?: number; pp?: number };
    avatar_url?: string | null;
    casting?: { ability?: string; spell_attack?: number; save_dc?: number };
    spells?: CharSpell[];
    appearance?: {
      age?: string;
      height?: string;
      weight?: string;
      eyes?: string;
      skin?: string;
      hair?: string;
      distinguishing_marks?: string;
    };
    background?: {
      backstory?: string;
      personality?: string;
      ideals?: string;
      bonds?: string;
      flaws?: string;
      notes?: string;
    };
    alignment?: string;
    armor?: { type?: ArmorType; ac_base?: number; max_dex?: number; stealth_disadvantage?: boolean };
    shield?: boolean;
    equipment?: Array<{
      id: string;
      name: string;
      qty: number;
      weight?: number;
      equipped?: boolean;
      notes?: string;
    }>;
    weapons?: Array<{
      id: string;
      name: string;
      attack_bonus?: number;
      damage?: string;        // e.g. "1d8+3"
      damage_die?: string;    // e.g. "1d8" (base die without mod)
      versatile_die?: string; // e.g. "1d10" for two-handed use
      damage_type?: string;   // e.g. "slashing"
      range?: string;         // "melee" / "60/120 ft"
      properties?: string;    // "finesse, light"
      description?: string;   // freeform notes / flavor / effects
      equipped?: boolean;
    }>;
    resistances?: string[];
    vulnerabilities?: string[];
    immunities?: string[];
    potions?: Array<{ id: string; name: string; qty: number; heal_dice: string }>;
    fighting_styles?: string[];
  };
  type Character = {
    id: string;
    owner_id: string;
    name: string;
    race?: string | null;
    level_total: number;
    sheet: Sheet;
    portrait_url?: string | null;
  };

  const cid = $derived(page.params.id!);
  let list = $state<Character[]>([]);
  let idx = $state(0);
  let limit = $state(1);
  let busy = $state(false);
  let error = $state('');
  let loading = $state(true);

  let newName = $state('');
  let newRace = $state('');
  let newLevel = $state(1);
  let newAlignment = $state('');

  async function load() {
    try {
      list = (await Characters.list(cid)) as unknown as Character[];
      if (idx >= list.length) idx = Math.max(0, list.length - 1);
      // fetch member cap for current user
      try {
        const members = await Campaigns.members(cid);
        const me = members.find((m) => m.user_id === auth.user?.id);
        limit = me?.character_limit ?? 1;
      } catch { /* not a member → default */ }
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t === 'character_updated' || t === 'combatant_updated' || t === 'character_created' || t === 'character_deleted') load();
    });
  });
  onDestroy(() => {
    offWs?.();
    clearTimeout(bookTimer);
    clearTimeout(spellbookSearchTimer);
  });

  /**
   * Auto-seed class-default resources AND spell slots when classes change.
   * Runs for the currently-viewed character (owner only).
   * New rows/slots are added at most once per (class, resource/level).
   * Existing user-edited rows are NEVER overwritten — players can manually
   * bump `max`, delete a slot level, or add custom rows; this effect only
   * fills in what's missing.
   */
  // Per-character last-seeded signature. Keyed by c.id so switching
  // between characters or incoming WS reloads don't retrigger seeding
  // that's already been applied. Fixed: bounded size + cleanup.
  const seededSigs = new Map<string, string>();
  const MAX_SIG_CACHE = 100; // Prevent unbounded memory growth
  
  function cleanupOldSigs() {
    // Keep only most recent entries when over limit
    if (seededSigs.size > MAX_SIG_CACHE) {
      const entries = Array.from(seededSigs.entries());
      seededSigs.clear();
      // Keep last 50 entries (LRU-ish: assumes iteration order = insertion order)
      entries.slice(-50).forEach(([k, v]) => seededSigs.set(k, v));
    }
  }
  
  // Track pending patches to avoid re-entrant updates
  let pendingPatch: { c: Character; patchFn: (s: Sheet) => Sheet } | null = null;
  
  $effect(() => {
    const c = list[idx];
    if (!c || !canEdit(c)) return;
    const classes = (c.sheet?.classes ?? []).filter((cl) => cl.name?.trim());
    const sig = classes.map((cl) => `${cl.name}@${cl.level}`).join('|');
    if (seededSigs.get(c.id) === sig) return;
    seededSigs.set(c.id, sig);
    cleanupOldSigs();
    
    const existing = new Set((c.sheet?.resources ?? []).map((r) => r.name.trim().toLowerCase()));
    const toAdd: Array<{ id: string; name: string; current: number; max: number; reset: 'short' | 'long' | 'none' }> = [];
    for (const cl of classes) {
      for (const tpl of templatesForClass(cl.name)) {
        if (tpl.minLevel && cl.level < tpl.minLevel) continue;
        const max = tpl.maxFor(cl.level);
        if (max <= 0) continue;
        if (existing.has(tpl.name.toLowerCase())) continue;
        existing.add(tpl.name.toLowerCase());
        toAdd.push({ id: randomUUID(), name: tpl.name, current: max, max, reset: tpl.reset });
      }
    }
    // compute expected baseline slots and add any that are missing. Existing
    // slot rows are left unchanged so user manual edits survive. When the
    // class-derived max is HIGHER than the stored max (e.g. the player just
    // levelled up), bump the max upward but keep `current` clamped to the
    // new max so we don't erase a resource the player already spent.
    const baseline = computeBaselineSlots(c);
    const curSlots = c.sheet?.slots ?? {};
    const nextSlots: Record<string, { current: number; max: number }> = { ...curSlots };
    let slotsChanged = false;
    // Add new / bump upward
    for (const [lvl, mx] of Object.entries(baseline)) {
      const cur = curSlots[lvl];
      if (!cur) {
        nextSlots[lvl] = { current: mx, max: mx };
        slotsChanged = true;
      } else if (cur.max < mx) {
        nextSlots[lvl] = { current: Math.min(cur.current + (mx - cur.max), mx), max: mx };
        slotsChanged = true;
      } else if (cur.max > mx) {
        // max reduced: clamp current too
        nextSlots[lvl] = { current: Math.min(cur.current, mx), max: mx };
        slotsChanged = true;
      }
    }
    // Remove levels no longer in the baseline
    for (const lvl of Object.keys(curSlots)) {
      if (!(lvl in baseline)) {
        delete nextSlots[lvl];
        slotsChanged = true;
      }
    }

    // Slippery Mind (Rogue 15+): WIS save proficiency
    // Diamond Soul (Monk 14+): all save proficiencies
    // PHB class base save proficiencies
    const CLASS_SAVES: Record<string, Ability[]> = {
      barbarian: ['str','con'],
      bard: ['dex','cha'],
      cleric: ['wis','cha'],
      druid: ['int','wis'],
      fighter: ['str','con'],
      monk: ['str','dex'],
      paladin: ['wis','cha'],
      ranger: ['str','dex'],
      rogue: ['dex','int'],
      sorcerer: ['con','cha'],
      warlock: ['wis','cha'],
      wizard: ['int','wis'],
      artificer: ['con','int'],
      'blood hunter': ['str','wis'],
    };
    const savesToGrant: Ability[] = [];
    const ALL_SAVES: Ability[] = ['str','dex','con','int','wis','cha'];
    for (const cl of classes) {
      const n = cl.name?.trim().toLowerCase() ?? '';
      // Base class save proficiencies
      const baseSaves = CLASS_SAVES[n] ?? [];
      for (const ab of baseSaves) {
        if (!c.sheet?.saves?.[ab] && !savesToGrant.includes(ab)) savesToGrant.push(ab);
      }
      // Class feature save grants
      if (n === 'rogue' && (cl.level ?? 1) >= 15 && !c.sheet?.saves?.wis) savesToGrant.push('wis');
      if (n === 'monk' && (cl.level ?? 1) >= 14) {
        for (const ab of ALL_SAVES) {
          if (!c.sheet?.saves?.[ab] && !savesToGrant.includes(ab)) savesToGrant.push(ab);
        }
      }
    }
    const savesChanged = savesToGrant.length > 0;

    // Champion Fighter level 3+: auto-set crit_range to 19 if still default 20
    const isChampion = classes.some((cl) =>
      cl.name?.toLowerCase() === 'fighter' &&
      (cl.subclass ?? '').toLowerCase().includes('champion') &&
      cl.level >= 3
    );
    const currentCritRange = (c.sheet as Record<string, unknown>)?.crit_range as number ?? 20;
    const critRangeChanged = isChampion && currentCritRange > 19;

    // Draconic Bloodline Sorcerer: auto-set armor to draconic if no armor set
    const isDraconic = classes.some((cl) =>
      cl.name?.toLowerCase() === 'sorcerer' &&
      (cl.subclass ?? '').toLowerCase().includes('draconic')
    );
    const draconicArmorNeeded = isDraconic && !c.sheet?.armor;

    // Auto-sync max HP upward when class levels increase
    const computedHp = computedMaxHP(c);
    const currentMaxHp = c.sheet?.hp?.max ?? 0;
    const hpChanged = computedHp > currentMaxHp;

    // Bardic Inspiration max = CHA mod (min 1). Update if changed.
    const chaMod = abilityMod(c.sheet?.abilities?.cha);
    const biMax = Math.max(1, chaMod);
    let biChanged = false;
    const nextResources = toAdd.length ? [ ...(c.sheet?.resources ?? []), ...toAdd ] : (c.sheet?.resources ?? []).map((r) => {
      if (r.name.trim().toLowerCase() === 'bardic inspiration' && r.max !== biMax) {
        biChanged = true;
        return { ...r, max: biMax, current: Math.min(r.current, biMax) };
      }
      return r;
    });

    const hdPools: Array<{ name: string; die: string; current: number; max: number }> = c.sheet?.hit_dice?.pools ?? [];
    const poolsMap = new Map(hdPools.map((p) => [p.name.toLowerCase(), { ...p }]));
    let poolsChanged = false;
    for (const cl of classes) {
      const key = cl.name?.trim().toLowerCase();
      if (!key) continue;
      const die = cl.hit_die ?? hitDieFor(cl.name ?? '');
      const level = cl.level ?? 1;
      const existing = poolsMap.get(key);
      if (existing) {
        if (existing.max !== level) {
          existing.current = Math.max(0, existing.current + level - existing.max);
          existing.max = level;
          poolsChanged = true;
        }
      } else {
        poolsMap.set(key, { name: cl.name!, die: die, current: level, max: level });
        poolsChanged = true;
      }
    }
    for (const key of poolsMap.keys()) {
      if (!classes.some((c: { name?: string }) => c.name?.trim().toLowerCase() === key)) {
        const pool = poolsMap.get(key)!;
        pool.current = 0;
        poolsChanged = true;
      }
    }

    if (!toAdd.length && !slotsChanged && !savesChanged && !critRangeChanged && !draconicArmorNeeded && !hpChanged && !poolsChanged && !biChanged) return;

    // Fix: queue patch but guard against re-entrancy by checking pending
    if (pendingPatch) return; // Already have pending patch
    pendingPatch = { c, patchFn: (s) => ({
      ...s,
      resources: nextResources,
      slots: slotsChanged ? nextSlots : s.slots,
      saves: savesChanged ? { ...(s.saves ?? {}), ...Object.fromEntries(savesToGrant.map((a) => [a, true])) } : s.saves,
      ...(critRangeChanged ? { crit_range: 19 } : {}),
      ...(draconicArmorNeeded ? { armor: { type: 'draconic' as ArmorType, ac_base: 13, max_dex: 99 } } : {}),
      hp: hpChanged ? { ...(s.hp ?? {}), max: computedHp, current: Math.min(s.hp?.current ?? 0, computedHp) } : s.hp,
      hit_dice: poolsChanged ? { pools: Array.from(poolsMap.values()) } : s.hit_dice,
    })};
    
    queueMicrotask(() => {
      if (!pendingPatch) return;
      const { c: char, patchFn } = pendingPatch;
      pendingPatch = null;
      patchSheet(char, patchFn);
    });
  });

  const raceSeedSigs = new Map<string, string>();
  $effect(() => {
    const c = list[idx];
    if (!c || !canEdit(c)) return;
    const sig = c.race ?? '';
    if (raceSeedSigs.get(c.id) === sig) return;
    raceSeedSigs.set(c.id, sig);
    const def = racialDefaults(c.race);
    if (!def) return;
    const updates: Partial<Sheet> = {};
    if (def.speed && !c.sheet?.speed) updates.speed = def.speed;
    if (def.darkvision && !(c.sheet?.senses as Record<string, unknown> | undefined)?.darkvision) {
      updates.senses = { ...(c.sheet?.senses ?? {}), darkvision: def.darkvision } as Sheet['senses'];
    }
    // Languages
    if (def.languages && !c.sheet?.languages) updates.languages = def.languages;
    // Special speeds
    const curSheet = c.sheet as Record<string, unknown> | undefined;
    if (def.swim_speed && !curSheet?.swim_speed) (updates as Record<string, unknown>).swim_speed = def.swim_speed;
    if (def.fly_speed && !curSheet?.fly_speed) (updates as Record<string, unknown>).fly_speed = def.fly_speed;
    if (def.climb_speed && !curSheet?.climb_speed) (updates as Record<string, unknown>).climb_speed = def.climb_speed;
    const existing = new Set((c.sheet?.resources ?? []).map((r) => r.name.trim().toLowerCase()));
    const toAdd: Array<{ id: string; name: string; current: number; max: number; reset: 'short' | 'long' | 'none' }> = [];
    for (const res of def.resources ?? []) {
      if (!existing.has(res.name.toLowerCase())) {
        toAdd.push({ id: randomUUID(), name: res.name, current: res.max, max: res.max, reset: res.reset });
      }
    }
    if (toAdd.length) updates.resources = [...(c.sheet?.resources ?? []), ...toAdd];
    if (def.resistances?.length) {
      const existing_res = new Set(((c.sheet as Record<string,unknown>)?.resistances as string[] ?? []));
      const newRes = def.resistances.filter((r) => !existing_res.has(r));
      if (newRes.length) (updates as Record<string,unknown>).resistances = [...existing_res, ...newRes];
    }
    if (def.flags) {
      for (const [k, v] of Object.entries(def.flags)) {
        if (!(c.sheet as Record<string,unknown>)?.[k]) (updates as Record<string,unknown>)[k] = v;
      }
    }
    // Tortle natural armor: AC 17 (no DEX bonus)
    if ((c.race ?? '').toLowerCase().includes('tortle') && !c.sheet?.armor) {
      (updates as Record<string,unknown>).armor = { type: 'natural', ac_base: 17, max_dex: 0 };
    }

    // Auto-set spellcasting ability per class if not already set
    if (!c.sheet?.casting?.ability) {
      const ability = detectSpellcastingAbility(c);
      if (ability) (updates as Record<string,unknown>).casting = { ...(c.sheet?.casting ?? {}), ability };
    }
    if (Object.keys(updates).length) patchSheet(c, (s) => ({ ...s, ...updates }));
  });

  // Tiefling Infernal Legacy: seed spells by level
  const tieflingSpellSigs = new Map<string, string>();
  $effect(() => {
    const c = list[idx];
    if (!c || !canEdit(c)) return;
    if (!c.race?.toLowerCase().includes('tiefling')) return;
    const sig = `${c.id}@${c.level_total}`;
    if (tieflingSpellSigs.get(c.id) === sig) return;
    tieflingSpellSigs.set(c.id, sig);
    const spells = c.sheet?.spells as Array<{ slug: string }> ?? [];
    const known = new Set(spells.map((s) => s.slug));
    const toAdd: Array<Record<string, unknown>> = [];
    if (!known.has('thaumaturgy'))
      toAdd.push({ id: randomUUID(), slug: 'thaumaturgy', name: 'Thaumaturgy', level: 0, school: 'transmutation', classes: ['Tiefling'], prepared: true, custom: false });
    if (c.level_total >= 3 && !known.has('hellish-rebuke'))
      toAdd.push({ id: randomUUID(), slug: 'hellish-rebuke', name: 'Hellish Rebuke', level: 1, school: 'evocation', classes: ['Tiefling'], prepared: true, custom: false });
    if (c.level_total >= 5 && !known.has('darkness'))
      toAdd.push({ id: randomUUID(), slug: 'darkness', name: 'Darkness', level: 2, school: 'evocation', classes: ['Tiefling'], prepared: true, custom: false });
    if (toAdd.length) patchSheet(c, (s) => ({ ...s, spells: [...((s.spells as CharSpell[]) ?? []), ...(toAdd as CharSpell[])] }));
  });

  // Drow: Dancing Lights (cantrip), Faerie Fire (3+), Darkness (5+)
  const drowSpellSigs = new Map<string, string>();
  $effect(() => {
    const c = list[idx];
    if (!c || !canEdit(c)) return;
    if (!c.race?.toLowerCase().includes('drow')) return;
    const sig = `${c.id}@${c.level_total}`;
    if (drowSpellSigs.get(c.id) === sig) return;
    drowSpellSigs.set(c.id, sig);
    const spells = c.sheet?.spells as Array<{ slug: string }> ?? [];
    const known = new Set(spells.map((s) => s.slug));
    const toAdd: Array<Record<string, unknown>> = [];
    if (!known.has('dancing-lights'))
      toAdd.push({ id: randomUUID(), slug: 'dancing-lights', name: 'Dancing Lights', level: 0, school: 'evocation', classes: ['Drow'], prepared: true, custom: false });
    if (c.level_total >= 3 && !known.has('faerie-fire'))
      toAdd.push({ id: randomUUID(), slug: 'faerie-fire', name: 'Faerie Fire', level: 1, school: 'evocation', classes: ['Drow'], prepared: true, custom: false });
    if (c.level_total >= 5 && !known.has('darkness'))
      toAdd.push({ id: randomUUID(), slug: 'darkness', name: 'Darkness', level: 2, school: 'evocation', classes: ['Drow'], prepared: true, custom: false });
    if (toAdd.length) patchSheet(c, (s) => ({ ...s, spells: [...((s.spells as CharSpell[]) ?? []), ...(toAdd as CharSpell[])] }));
  });

  // own characters count for gating
  const owned = $derived(list.filter((c) => c.owner_id === auth.user?.id).length);
  const canCreate = $derived(!campaign().isMaster && owned < limit);

  async function create(close: () => void) {
    busy = true;
    try {
      await Characters.create(cid, { name: newName, race: newRace, level_total: newLevel, sheet: { alignment: newAlignment || undefined } });
      newName = ''; newRace = ''; newLevel = 1; newAlignment = '';
      close();
      await load();
      // jump to the newly-added character (last)
      idx = list.length - 1;
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  async function patchField(id: string, field: string, value: unknown) {
    const c = list.find((x) => x.id === id);
    if (c && !canEdit(c)) return;
    await Characters.update(id, { [field]: value });
    await load();
  }

  async function setDeathSave(c: Character, kind: 'successes' | 'failures', i: number) {
    // click toggles i+1 / i: clicking current bubble clears it, else sets count to i+1
    const cur = (c.sheet?.death_saves?.[kind] ?? 0) as number;
    const next = cur === i + 1 ? i : i + 1;
    await patchSheet(c, (s) => {
      const ds = { ...(s.death_saves ?? {}), [kind]: next };
      const sheet: Sheet = { ...s, death_saves: ds };
      if (kind === 'failures' && next >= 3) sheet.alive = false;
      if (kind === 'successes' && next >= 3) {
        sheet.death_saves = { successes: 0, failures: 0 };
        sheet.alive = true;
        sheet.hp = { ...(s.hp ?? {}), current: 0 };
      }
      return sheet;
    });
  }

  async function patchSheet(c: Character, mutator: (s: Sheet) => Sheet) {
    if (!canEdit(c)) return;
    const next = mutator({ ...(c.sheet ?? {}) });
    await Characters.update(c.id, { sheet: next });
    await load();
  }

  async function remove(c: Character) {
    if (!confirm($_('character.delete_character_confirm').replace('{{name}}', c.name))) return;
    await Characters.delete(c.id);
    if (idx >= list.length - 1) idx = Math.max(0, list.length - 2);
    await load();
  }

  function slot(c: Character, lvl: string) {
    return c.sheet?.slots?.[lvl] ?? { current: 0, max: 0 };
  }

  // ---- 5e derived values ----
  function abilityMod(score: number | undefined): number {
    return Math.floor(((score ?? 10) - 10) / 2);
  }
  function abilityModForChar(c: Character, ab: Ability): number {
    return Math.floor(((abilityScore(c, ab) ?? 10) - 10) / 2);
  }
  function profBonus(level: number): number {
    // standard 5e proficiency scaling
    return 2 + Math.floor((Math.max(1, level) - 1) / 4);
  }
  function saveMod(c: Character, ab: Ability): number {
    const ov = c.sheet?.saves_override?.[ab];
    if (typeof ov === 'number') return ov;
    const mod = abilityModForChar(c, ab);
    return mod + (c.sheet?.saves?.[ab] ? profBonus(c.level_total) : 0);
  }
  function abilityScore(c: Character, ab: Ability): number {
    const override = c.sheet?.abilities_override?.[ab];
    if (typeof override === 'number') return override;
    return abilityScoreWithRacial(c, ab);
  }
  function hasAbilityOverride(c: Character, ab: Ability): boolean {
    return typeof c.sheet?.abilities_override?.[ab] === 'number';
  }
  function hasSaveOverride(c: Character, ab: Ability): boolean {
    return typeof c.sheet?.saves_override?.[ab] === 'number';
  }
  function classLevel(c: Character, cls: string): number {
    return (c.sheet?.classes ?? []).find((cl) => cl.name?.toLowerCase() === cls.toLowerCase())?.level ?? 0;
  }

  function hasJackOfAllTrades(c: Character): boolean {
    return classLevel(c, 'bard') >= 2;
  }

  function skillMod(c: Character, sk: Skill): number {
    const mod = abilityModForChar(c, sk.ability);
    const lvl = c.sheet?.skills?.[sk.key];
    const pb = profBonus(c.level_total);
    if (lvl === 'expert') return mod + pb * 2;
    if (lvl === 'prof')   return mod + pb;
    if (hasJackOfAllTrades(c)) return mod + Math.floor(pb / 2);
    return mod;
  }

  function cantripDiceMultiplier(totalLevel: number): number {
    if (totalLevel >= 17) return 4;
    if (totalLevel >= 11) return 3;
    if (totalLevel >= 5)  return 2;
    return 1;
  }

  function sneakAttackDice(c: Character): number {
    const rl = classLevel(c, 'rogue');
    return rl >= 1 ? Math.ceil(rl / 2) : 0;
  }

  function martialArtsDie(c: Character): string | null {
    const ml = classLevel(c, 'monk');
    if (ml <= 0) return null;
    if (ml >= 17) return 'd10';
    if (ml >= 11) return 'd8';
    if (ml >= 5)  return 'd6';
    return 'd4';
  }

  function bardicInspirationDie(c: Character): string | null {
    const bl = classLevel(c, 'bard');
    if (bl <= 0) return null;
    if (bl >= 15) return 'd12';
    if (bl >= 10) return 'd10';
    if (bl >= 5)  return 'd8';
    return 'd6';
  }

  function extraAttackCount(c: Character): number {
    const classes = c.sheet?.classes ?? [];
    let best = 0;
    for (const cl of classes) {
      const n = cl.name?.toLowerCase() ?? '';
      const l = cl.level ?? 0;
      let count = 0;
      if (n === 'fighter') {
        count = l >= 20 ? 3 : l >= 11 ? 2 : l >= 5 ? 1 : 0;
      } else if (n === 'paladin' || n === 'ranger' || n === 'barbarian' || n === 'monk') {
        count = l >= 5 ? 1 : 0;
      } else if (n === 'warlock') {
        const sub = (cl.subclass ?? '').toLowerCase();
        count = (sub.includes('blade') && l >= 5) ? 1 : 0;
      }
      if (count > best) best = count;
    }
    return best;
  }

  function spellPrepCount(c: Character): number | null {
    const classes = c.sheet?.classes ?? [];
    let total = 0;
    let hasPrepClass = false;
    for (const cl of classes) {
      const n = cl.name?.toLowerCase() ?? '';
      const l = cl.level ?? 0;
      if (n === 'cleric' || n === 'druid') {
        hasPrepClass = true;
        total += Math.max(1, l + abilityModForChar(c, 'wis'));
      } else if (n === 'paladin') {
        hasPrepClass = true;
        total += Math.max(1, Math.floor(l / 2) + abilityModForChar(c, 'cha'));
      } else if (n === 'wizard' || n === 'artificer') {
        hasPrepClass = true;
        total += Math.max(1, l + abilityModForChar(c, 'int'));
      }
    }
    return hasPrepClass ? total : null;
  }
  function hasReliableTalent(c: Character): boolean {
    return classLevel(c, 'rogue') >= 11;
  }

  function hasEvasion(c: Character): boolean {
    return classLevel(c, 'rogue') >= 7 || classLevel(c, 'monk') >= 7;
  }

  function auraOfProtectionBonus(c: Character): number | null {
    if (classLevel(c, 'paladin') < 6) return null;
    return abilityModForChar(c, 'cha');
  }

  function rageDamageBonus(c: Character): number | null {
    const bl = classLevel(c, 'barbarian');
    if (bl <= 0) return null;
    if (bl >= 16) return 4;
    if (bl >= 9)  return 3;
    return 2;
  }

  function destroyUndeadCR(c: Character): string | null {
    const cl = classLevel(c, 'cleric');
    if (cl < 5) return null;
    if (cl >= 17) return 'CR 4';
    if (cl >= 14) return 'CR 3';
    if (cl >= 11) return 'CR 2';
    if (cl >= 8)  return 'CR 1';
    return 'CR 1/2';
  }

  function wildShapeCR(c: Character): string | null {
    const dl = classLevel(c, 'druid');
    if (dl < 2) return null;
    if (dl >= 8)  return 'CR 1';
    if (dl >= 4)  return 'CR 1/2';
    return 'CR 1/4';
  }

  function isChampionFighter(c: Character): boolean {
    return (c.sheet?.classes ?? []).some((cl) =>
      cl.name?.toLowerCase() === 'fighter' && (cl.subclass ?? '').toLowerCase().includes('champion') && cl.level >= 3
    );
  }

  function passivePerception(c: Character): number {
    const perc = SKILLS.find((s) => s.key === 'perception')!;
    return 10 + skillMod(c, perc) + (c.sheet?.senses?.passive_perception_bonus ?? 0);
  }
  function carryCapacity(c: Character): number {
    // 5e base: STR × 15 (lb)
    return (c.sheet?.abilities?.str ?? 10) * 15;
  }
  function totalWeight(c: Character): number {
    return (c.sheet?.equipment ?? []).reduce((sum, it) => sum + ((it.weight ?? 0) * (it.qty ?? 1)), 0);
  }

  /** Compute AC from raw armor config + abilities (no Character required). */
  function computeAC(sheet: {
    armor?: Sheet['armor']; shield?: boolean; abilities?: Sheet['abilities'];
    ac?: number; ac_bonus?: number; medium_armor_max_dex_override?: number;
  }): number {
    const armor = sheet.armor;
    const shield = sheet.shield ?? false;
    const dexMod = abilityMod(sheet.abilities?.dex);
    const acBonus = sheet.ac_bonus ?? 0;
    if (!armor || !armor.type) return (sheet.ac ?? 10) + acBonus;
    const shieldBonus = shield ? 2 : 0;
    let base: number;
    switch (armor.type) {
      case 'unarmored_barbarian': base = 10 + dexMod + abilityMod(sheet.abilities?.con) + shieldBonus; break;
      case 'unarmored_monk': base = 10 + dexMod + abilityMod(sheet.abilities?.wis) + shieldBonus; break;
      case 'mage_armor': base = 13 + dexMod + shieldBonus; break;
      case 'draconic': base = 13 + dexMod + shieldBonus; break;
      case 'natural': base = (armor.ac_base ?? 10) + shieldBonus; break;
      default: {
        const acBase = armor.ac_base ?? 10;
        const medOverride = sheet.medium_armor_max_dex_override;
        const maxDex = medOverride ?? armor.max_dex ?? 99;
        base = acBase + Math.min(dexMod, maxDex) + shieldBonus;
        break;
      }
    }
    return base + acBonus;
  }

  function computedAC(c: Character): number {
    let base = computeAC(c.sheet ?? {});
    if ((c.sheet?.feats ?? []).some((f: { key: string }) => f.key === 'dual_wielder')) {
      const melee = (c.sheet?.weapons ?? []).filter((w) => w.equipped !== false && (!w.range || w.range.toLowerCase().includes('melee') || !w.range));
      if (melee.length >= 2) base += 1;
    }
    // Attunement AC bonuses
    const attunementAc = ((c.sheet as Record<string,unknown>)?.attunement as Array<Record<string,unknown>> | undefined)
      ?.reduce((sum, a) => sum + (((a.bonuses as Record<string,unknown>)?.ac as number) ?? 0), 0) ?? 0;
    return base + attunementAc;
  }

  function computedMaxHP(c: Character): number {
    const conMod = abilityMod(c.sheet?.abilities?.con);
    const classes = c.sheet?.classes ?? [];
    if (classes.length === 0) return c.sheet?.hp?.max ?? 1;
    let total = 0;
    let firstClass = true;
    for (const cls of classes) {
      const level = cls.level ?? 1;
      const die = cls.hit_die ?? hitDieFor(cls.name ?? '');
      const dieMax = parseInt(die.replace('d', ''), 10) || 8;
      const avg = die === 'd6' ? 4 : die === 'd8' ? 5 : die === 'd10' ? 6 : die === 'd12' ? 7 : 5;
      if (firstClass) {
        total += dieMax + conMod + (level - 1) * (avg + conMod);
        firstClass = false;
      } else {
        total += level * (avg + conMod);
      }
    }
    // Hill dwarf: +1 HP per level
    if (c.race?.toLowerCase().includes('hill dwarf')) {
      total += classes.reduce((sum, cls) => sum + (cls.level ?? 1), 0);
    }
    // Tough feat: +2 HP per level
    if ((c.sheet?.feats ?? []).some((f: { key: string }) => f.key === 'tough')) {
      total += 2 * classes.reduce((sum, cls) => sum + (cls.level ?? 1), 0);
    }
    const reduction = (c.sheet as Record<string,unknown>)?.hp_max_reduction as number ?? 0;
    return Math.max(1, total - reduction);
  }

  function computedSpeed(c: Character): number {
    const baseSpeed = racialDefaults(c.race)?.speed ?? 30;
    const classes = c.sheet?.classes ?? [];
    const armorType = c.sheet?.armor?.type;
    const hasShield = c.sheet?.shield ?? false;
    let bonus = 0;
    for (const cl of classes) {
      const n = cl.name?.toLowerCase() ?? '';
      const l = cl.level ?? 0;
      // Barbarian Fast Movement: +10ft when not wearing heavy armor
      if (n === 'barbarian' && l >= 5 && armorType !== 'heavy') bonus = Math.max(bonus, 10);
      // Monk Unarmored Movement: scales 10-30ft when unarmored and no shield
      if (n === 'monk' && l >= 2) {
        if ((!armorType || armorType === 'unarmored_monk') && !hasShield) {
          const monkBonus = l >= 18 ? 30 : l >= 14 ? 25 : l >= 10 ? 20 : l >= 6 ? 15 : 10;
          bonus = Math.max(bonus, monkBonus);
        }
      }
    }
    // Mobile feat: +10 speed
    if ((c.sheet?.feats ?? []).some((f: { key: string }) => f.key === 'mobile')) bonus += 10;
    // Heavy armor STR requirement: -10 speed if STR < 15 (PHB p.144)
    const strScore = c.sheet?.abilities?.str ?? 10;
    if (armorType === 'heavy' && strScore < 15) bonus -= 10;
    // Encumbrance: -10 light, -20 heavy (PHB p.176)
    const w = totalWeight(c);
    if (w > strScore * 10) bonus -= 20;
    else if (w > strScore * 5) bonus -= 10;
    return baseSpeed + bonus;
  }

  function computedSpellAttack(c: Character): number | null {
    const ability = c.sheet?.casting?.ability?.toLowerCase() as Ability | undefined;
    if (!ability) return null;
    return abilityModForChar(c, ability) + profBonus(c.level_total);
  }

  function computedSpellSaveDC(c: Character): number | null {
    const ability = c.sheet?.casting?.ability?.toLowerCase() as Ability | undefined;
    if (!ability) return null;
    return 8 + abilityModForChar(c, ability) + profBonus(c.level_total);
  }

  /** Detect the expected spellcasting ability from a character's classes. */
  function detectSpellcastingAbility(c: Character): Ability | null {
    const classes = c.sheet?.classes ?? [];
    if (!classes.length) return null;
    const abilityByClass: Record<string, Ability> = {
      wizard: 'int', artificer: 'int',
      cleric: 'wis', druid: 'wis', ranger: 'wis',
      bard: 'cha', paladin: 'cha', sorcerer: 'cha', warlock: 'cha',
    };
    const votes = new Map<Ability, number>();
    for (const cl of classes) {
      const n = cl.name?.trim().toLowerCase() ?? '';
      const sp = cl.spellcasting_ability?.toLowerCase() as Ability | undefined;
      if (sp) {
        votes.set(sp, (votes.get(sp) ?? 0) + (cl.level ?? 1));
        continue;
      }
      if (cl.spellcasting_ability) continue;
      const ab = abilityByClass[n];
      if (ab) votes.set(ab, (votes.get(ab) ?? 0) + (cl.level ?? 1));
    }
    if (!votes.size) return null;
    let best: Ability = 'cha';
    let bestVotes = 0;
    for (const [ab, v] of votes) {
      if (v > bestVotes) { best = ab; bestVotes = v; }
    }
    return best;
  }

  function computedWeaponAttackBonus(c: Character, w: { properties?: string; range?: string }): number {
    const props = (w.properties ?? '').toLowerCase();
    const isFinesse = props.includes('finesse');
    const isRanged = props.includes('ranged') || (w.range && !w.range.toLowerCase().includes('melee') && w.range !== '');
    const strMod = abilityModForChar(c, 'str');
    const dexMod = abilityModForChar(c, 'dex');
    const mod = isFinesse ? Math.max(strMod, dexMod) : isRanged ? dexMod : strMod;
    const styles: string[] = c.sheet?.fighting_styles ?? [];
    const archeryBonus = isRanged && styles.some(s => s.toLowerCase() === 'archery') ? 2 : 0;
    return mod + profBonus(c.level_total) + archeryBonus;
  }

  function racialAbilityBonus(c: Character, ab: Ability): number {
    const race = c.race?.toLowerCase() ?? '';
    const bonuses: Record<string, Record<string, number>> = {
      dragonborn: { str: 2, cha: 1 },
      'hill dwarf': { con: 2, wis: 1 },
      'mountain dwarf': { con: 2, str: 2 },
      'high elf': { dex: 2, int: 1 },
      'wood elf': { dex: 2, wis: 1 },
      drow: { dex: 2, cha: 1 },
      eladrin: { dex: 2, int: 1 },
      'forest gnome': { int: 2, dex: 1 },
      'rock gnome': { int: 2, con: 1 },
      'half-elf': { cha: 2 },
      'half-orc': { str: 2, con: 1 },
      'lightfoot halfling': { dex: 2, cha: 1 },
      'stout halfling': { dex: 2, con: 1 },
      tiefling: { cha: 2, int: 1 },
      aasimar: { cha: 2 },
      'protector aasimar': { cha: 2, wis: 1 },
      'scourge aasimar': { cha: 2, con: 1 },
      'fallen aasimar': { cha: 2, str: 1 },
      bugbear: { str: 2, dex: 1 },
      firbolg: { wis: 2, str: 1 },
      goblin: { dex: 2, con: 1 },
      hobgoblin: { con: 2, int: 1 },
      kenku: { dex: 2, wis: 1 },
      kobold: { dex: 2, str: -2 },
      lizardfolk: { con: 2, wis: 1 },
      orc: { str: 2, con: 1, int: -2 },
      tabaxi: { dex: 2, cha: 1 },
      triton: { str: 1, con: 1, cha: 1 },
      'yuan-ti pureblood': { cha: 2, int: 1 },
      'human': { str: 1, dex: 1, con: 1, int: 1, wis: 1, cha: 1 },
      'variant human': {},
      'deep gnome': { int: 2, dex: 1 },
      'fairy': { dex: 2, cha: 1 },
      'satyr': { cha: 2, dex: 1 },
      'shadar-kai': { dex: 2, con: 1 },
      'githyanki': { str: 2, int: 1 },
      'githzerai': { wis: 2, int: 1 },
      'centaur': { str: 2, wis: 1 },
      'minotaur': { str: 2, con: 1 },
      'changeling': { cha: 2, dex: 1 },
      'warforged': { con: 2, str: 1 },
      'aarakocra': { dex: 2, wis: 1 },
      'tortle': { str: 2, wis: 1 },
      'genasi': {},
      'air genasi': { dex: 2, int: 1 },
      'earth genasi': { con: 2, str: 1 },
      'fire genasi': { int: 2, con: 1 },
      'water genasi': { wis: 2, con: 1 },
    };
    for (const [r, b] of Object.entries(bonuses)) {
      if (race.includes(r) && !race.includes('variant')) return b[ab] ?? 0;
    }
    return 0;
  }

  function abilityScoreWithRacial(c: Character, ab: Ability): number {
    const base = c.sheet?.abilities?.[ab] ?? 10;
    return Math.min(30, Math.max(1, base + racialAbilityBonus(c, ab)));
  }

  const RACIAL_DEFAULTS: Record<string, {
    speed: number; darkvision?: number; resistances?: string[]; languages?: string;
    swim_speed?: number; fly_speed?: number; climb_speed?: number;
    resources?: Array<{ name: string; reset: 'short' | 'long'; max: 1 }>;
    flags?: Record<string, unknown>;
  }> = {
    'dragonborn':        { speed: 30, languages: 'Common, Draconic', resources: [{ name: 'Breath Weapon', reset: 'short', max: 1 }] },
    'hill dwarf':        { speed: 25, darkvision: 60, resistances: ['poison'], languages: 'Common, Dwarvish' },
    'mountain dwarf':    { speed: 25, darkvision: 60, resistances: ['poison'], languages: 'Common, Dwarvish' },
    'high elf':          { speed: 30, darkvision: 60, languages: 'Common, Elvish' },
    'wood elf':          { speed: 35, darkvision: 60, languages: 'Common, Elvish' },
    'drow':              { speed: 30, darkvision: 120, languages: 'Common, Elvish' },
    'eladrin':           { speed: 30, darkvision: 60, languages: 'Common, Elvish' },
    'forest gnome':      { speed: 25, darkvision: 60, flags: { gnome_cunning: true }, languages: 'Common, Gnomish' },
    'rock gnome':        { speed: 25, darkvision: 60, flags: { gnome_cunning: true }, languages: 'Common, Gnomish' },
    'half-elf':          { speed: 30, darkvision: 60, languages: 'Common, Elvish' },
    'half-orc':          { speed: 30, darkvision: 60, resources: [{ name: 'Relentless Endurance', reset: 'long', max: 1 }], flags: { savage_attacks: true }, languages: 'Common, Orc' },
    'lightfoot halfling':{ speed: 25, languages: 'Common, Halfling' },
    'stout halfling':    { speed: 25, resistances: ['poison'], languages: 'Common, Halfling' },
    'human':             { speed: 30, languages: 'Common' },
    'variant human':     { speed: 30, languages: 'Common' },
    'tiefling':          { speed: 30, darkvision: 60, resistances: ['fire'], languages: 'Common, Infernal' },
    'aasimar':           { speed: 30, darkvision: 60, languages: 'Common, Celestial' },
    'protector aasimar': { speed: 30, darkvision: 60, languages: 'Common, Celestial' },
    'scourge aasimar':   { speed: 30, darkvision: 60, languages: 'Common, Celestial' },
    'fallen aasimar':    { speed: 30, darkvision: 60, languages: 'Common, Celestial' },
    'bugbear':           { speed: 30, darkvision: 60, languages: 'Common, Goblin' },
    'firbolg':           { speed: 30, languages: 'Common, Elvish, Giant' },
    'goblin':            { speed: 30, darkvision: 60, languages: 'Common, Goblin' },
    'hobgoblin':         { speed: 30, darkvision: 60, languages: 'Common, Goblin' },
    'kenku':             { speed: 30, languages: 'Common, Auran' },
    'kobold':            { speed: 30, darkvision: 60, languages: 'Common, Draconic' },
    'lizardfolk':        { speed: 30, swim_speed: 30, languages: 'Common, Draconic' },
    'orc':               { speed: 30, darkvision: 60, languages: 'Common, Orc' },
    'tabaxi':            { speed: 30, darkvision: 60, climb_speed: 20, languages: 'Common, Thieves\u2019 Cant' },
    'triton':            { speed: 30, swim_speed: 30, languages: 'Common, Primordial' },
    'yuan-ti pureblood': { speed: 30, darkvision: 60, languages: 'Common, Abyssal, Draconic' },
    'deep gnome':        { speed: 25, darkvision: 120, flags: { gnome_cunning: true }, languages: 'Common, Gnomish, Undercommon' },
    'fairy':             { speed: 30, fly_speed: 30, languages: 'Common, Sylvan' },
    'satyr':             { speed: 35, languages: 'Common, Sylvan' },
    'shadar-kai':        { speed: 30, darkvision: 60, languages: 'Common, Elvish' },
    'githyanki':         { speed: 30, languages: 'Common, Gith' },
    'githzerai':         { speed: 30, languages: 'Common, Gith' },
    'centaur':           { speed: 40, languages: 'Common, Sylvan' },
    'minotaur':          { speed: 30, languages: 'Common, Abyssal' },
    'changeling':        { speed: 30, languages: 'Common, Elvish' },
    'warforged':         { speed: 30, languages: 'Common' },
    'aarakocra':         { speed: 30, fly_speed: 50, languages: 'Common, Auran' },
    'tortle':            { speed: 30, swim_speed: 20, languages: 'Common, Aquan' },
    'genasi':            { speed: 30, darkvision: 60, languages: 'Common, Primordial' },
    'air genasi':        { speed: 30, darkvision: 60, languages: 'Common, Primordial' },
    'earth genasi':      { speed: 30, darkvision: 60, languages: 'Common, Primordial' },
    'fire genasi':       { speed: 30, darkvision: 60, languages: 'Common, Primordial' },
    'water genasi':      { speed: 30, darkvision: 60, swim_speed: 30, languages: 'Common, Primordial' },
  };

  function racialDefaults(race: string | null | undefined) {
    if (!race) return null;
    const r = race.toLowerCase();
    for (const [k, v] of Object.entries(RACIAL_DEFAULTS)) {
      if (r.includes(k)) return v;
    }
    return null;
  }

  async function toggleSave(c: Character, ab: Ability) {
    await patchSheet(c, (s) => ({ ...s, saves: { ...(s.saves ?? {}), [ab]: !(s.saves?.[ab]) } }));
  }
  async function cycleSkill(c: Character, key: string) {
    // none → prof → expert → none
    await patchSheet(c, (s) => {
      const cur = s.skills?.[key];
      const next: Record<string, 'prof' | 'expert'> = { ...(s.skills ?? {}) };
      if (!cur) next[key] = 'prof';
      else if (cur === 'prof') next[key] = 'expert';
      else delete next[key];
      return { ...s, skills: next };
    });
  }

  // ---- rests ----
  /**
   * Warlock pact-magic spell level table by class level (PHB).
   * Short rest refills these slots. For multiclass warlocks we only refill
   * the slots AT their pact-magic level (not the shared multiclass table).
   */
  function warlockPactSlotLevel(warlockLevel: number): number {
    if (warlockLevel >= 9) return 5;
    if (warlockLevel >= 7) return 4;
    if (warlockLevel >= 5) return 3;
    if (warlockLevel >= 3) return 2;
    if (warlockLevel >= 1) return 1;
    return 0;
  }
  async function shortRest(c: Character) {
    if (!confirm($_('character.short_rest_confirm'))) return;
    const hdCurrent = c.sheet?.hit_dice?.current ?? 0;
    const hdSpent = hdCurrent > 0 ? parseInt(prompt(`Hit dice to spend? (max ${hdCurrent})`) || '0') : 0;
    try {
      await Characters.shortRest(c.id as string, hdSpent);
    } catch (e) {
      alert((e as Error).message);
      return;
    }
    await load();
  }
  async function longRest(c: Character) {
    if (!confirm($_('character.long_rest_confirm'))) return;
    try {
      await Characters.longRest(c.id as string);
    } catch (e) {
      alert((e as Error).message);
      return;
    }
    await load();
  }

  // ---- enchantment helpers ----
  function spellKey(s: CharSpell): string { return s.slug ?? `custom:${s.name}`; }
  function hasSpell(c: Character, s: CharSpell): boolean {
    const k = spellKey(s);
    return (c.sheet?.spells ?? []).some((x) => spellKey(x) === k);
  }
  // ---- 5e caster progression (class level → highest spell level known) ----
  type CasterType = 'full' | 'half' | 'third' | 'warlock' | 'custom' | 'none';
  const isCustomClass = isCustomClassShared;
  function casterType(name: string, subclass?: string): CasterType {
    const n = name.trim().toLowerCase();
    const sub = (subclass ?? '').toLowerCase();
    if (['bard','cleric','druid','sorcerer','wizard'].includes(n)) return 'full';
    if (['paladin','ranger','artificer'].includes(n)) return 'half';
    if (n === 'warlock') return 'warlock';
    // third-casters: Fighter/Eldritch Knight, Rogue/Arcane Trickster
    if (n === 'fighter' && sub.includes('eldritch'))   return 'third';
    if (n === 'rogue'   && sub.includes('arcane'))     return 'third';
    // fighter/rogue w/o magical subclass = not a caster
    if (n === 'fighter' || n === 'rogue') return 'none';
    if (n === 'monk' || n === 'barbarian') return 'none';
    if (n === 'blood hunter') return 'none';
    // anything else — custom homebrew class: treat as full-caster blanket.
    return 'custom';
  }
  /**
   * Full-caster spell slots per class level (PHB). Table row 1..20, cols 1..9.
   * Used for single-class full casters AND for the multiclass slot table since
   * 5e uses the same numbers.
   */
  const FULL_CASTER_SLOTS: number[][] = [
    /* 1 */  [2, 0, 0, 0, 0, 0, 0, 0, 0],
    /* 2 */  [3, 0, 0, 0, 0, 0, 0, 0, 0],
    /* 3 */  [4, 2, 0, 0, 0, 0, 0, 0, 0],
    /* 4 */  [4, 3, 0, 0, 0, 0, 0, 0, 0],
    /* 5 */  [4, 3, 2, 0, 0, 0, 0, 0, 0],
    /* 6 */  [4, 3, 3, 0, 0, 0, 0, 0, 0],
    /* 7 */  [4, 3, 3, 1, 0, 0, 0, 0, 0],
    /* 8 */  [4, 3, 3, 2, 0, 0, 0, 0, 0],
    /* 9 */  [4, 3, 3, 3, 1, 0, 0, 0, 0],
    /* 10 */ [4, 3, 3, 3, 2, 0, 0, 0, 0],
    /* 11 */ [4, 3, 3, 3, 2, 1, 0, 0, 0],
    /* 12 */ [4, 3, 3, 3, 2, 1, 0, 0, 0],
    /* 13 */ [4, 3, 3, 3, 2, 1, 1, 0, 0],
    /* 14 */ [4, 3, 3, 3, 2, 1, 1, 0, 0],
    /* 15 */ [4, 3, 3, 3, 2, 1, 1, 1, 0],
    /* 16 */ [4, 3, 3, 3, 2, 1, 1, 1, 0],
    /* 17 */ [4, 3, 3, 3, 2, 1, 1, 1, 1],
    /* 18 */ [4, 3, 3, 3, 3, 1, 1, 1, 1],
    /* 19 */ [4, 3, 3, 3, 3, 2, 1, 1, 1],
    /* 20 */ [4, 3, 3, 3, 3, 2, 2, 1, 1],
  ];

  /** Warlock Pact Magic slots (count at current pact level). */
  function warlockPactSlotCount(L: number): number {
    if (L >= 17) return 4;
    if (L >= 11) return 3;
    if (L >=  2) return 2;
    if (L >=  1) return 1;
    return 0;
  }

  /**
   * Effective caster level for multiclassing (PHB p. 164):
   *  full-casters: full level
   *  half-casters: floor(level / 2) — Paladin/Ranger
   *  third-casters: floor(level / 3) — Fighter EK / Rogue AT
   *  custom (homebrew): treat as full-caster
   */
  function multiclassCasterLevel(c: Character): number {
    let total = 0;
    for (const cl of c.sheet?.classes ?? []) {
      if (!cl.name?.trim()) continue;
      const t = casterType(cl.name, cl.subclass);
      if (t === 'full' || t === 'custom') total += cl.level;
      else if (t === 'half') {
        const n = (cl.name ?? '').trim().toLowerCase();
        total += n === 'artificer' ? Math.ceil(cl.level / 2) : Math.floor(cl.level / 2);
      }
      else if (t === 'third') total += Math.floor(cl.level / 3);
      // warlock + none: contribute 0 to the multiclass table (pact magic separate)
    }
    return total;
  }

  /** Compute baseline spell slots {1..9 → max} given classes + levels. */
  function computeBaselineSlots(c: Character): Record<string, number> {
    const out: Record<string, number> = {};
    const classes = (c.sheet?.classes ?? []).filter((cl) => cl.name?.trim());
    if (!classes.length) return out;

    const warlocks = classes.filter((cl) => cl.name.trim().toLowerCase() === 'warlock');
    const nonWarlocks = classes.filter((cl) => cl.name.trim().toLowerCase() !== 'warlock');

    // Single-class full caster: use its own row (no multiclass math).
    if (nonWarlocks.length === 1 && !warlocks.length) {
      const cl = nonWarlocks[0];
      const t = casterType(cl.name, cl.subclass);
      if (t === 'full' || t === 'custom') {
        const row = FULL_CASTER_SLOTS[Math.min(20, Math.max(1, cl.level)) - 1];
        row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
        return out;
      }
      if (t === 'half') {
        // Artificer rounds UP per TCoE; Paladin/Ranger round DOWN
        const isArtificer = (cl.name ?? '').trim().toLowerCase() === 'artificer';
        const casterLv = isArtificer ? Math.ceil(cl.level / 2) : Math.floor(cl.level / 2);
        if (casterLv >= 1) {
          const row = FULL_CASTER_SLOTS[Math.min(20, casterLv) - 1];
          row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
        }
        return out;
      }
      if (t === 'third') {
        const casterLv = Math.floor(cl.level / 3);
        if (casterLv >= 1) {
          const row = FULL_CASTER_SLOTS[Math.min(20, casterLv) - 1];
          row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
        }
        return out;
      }
    }

    // Multiclass (any combo of non-warlock casters): sum effective caster levels
    // and read the full-caster table.
    if (nonWarlocks.length) {
      const casterLv = multiclassCasterLevel({ ...c, sheet: { ...(c.sheet ?? {}), classes: nonWarlocks } });
      if (casterLv >= 1) {
        const row = FULL_CASTER_SLOTS[Math.min(20, casterLv) - 1];
        row.forEach((n, i) => { if (n > 0) out[String(i + 1)] = n; });
      }
    }

    // Warlock pact magic: override slot count AT the pact level, adding if
    // missing. If the multiclass table already has a row at that level, stack
    // the pact slots on top per PHB errata (take the MAX for display; pact
    // slots replenish on short rest separately).
    for (const w of warlocks) {
      const pactLv = String(warlockPactSlotLevel(w.level));
      if (pactLv === '0') continue;
      const count = warlockPactSlotCount(w.level);
      if (count <= 0) continue;
      out[pactLv] = Math.max(out[pactLv] ?? 0, count);
    }
    return out;
  }

  function maxSpellLevelFor(cls: { name: string; level: number; subclass?: string }): number {
    const t = casterType(cls.name, cls.subclass);
    const L = cls.level;
    if (t === 'full' || t === 'custom') {
      if (L >= 17) return 9; if (L >= 15) return 8; if (L >= 13) return 7;
      if (L >= 11) return 6; if (L >=  9) return 5; if (L >=  7) return 4;
      if (L >=  5) return 3; if (L >=  3) return 2; if (L >=  1) return 1;
      return 0;
    }
    if (t === 'half') {
      if (L >= 17) return 5; if (L >= 13) return 4; if (L >= 9) return 3;
      if (L >=  5) return 2;
      if (cls.name.trim().toLowerCase() === 'artificer') { if (L >= 1) return 1; } else { if (L >= 2) return 1; }
      return 0;
    }
    if (t === 'warlock') {
      if (L >= 9) return 5; if (L >= 7) return 4; if (L >= 5) return 3;
      if (L >= 3) return 2; if (L >= 1) return 1;
      return 0;
    }
    if (t === 'third') {
      if (L >= 19) return 4; if (L >= 13) return 3; if (L >= 7) return 2;
      if (L >=  3) return 1;
      return 0;
    }
    return 0;
  }
  /** Per-class-name → highest spell level accessible to THIS character. */
  function spellAccessByClass(c: Character): Record<string, number> {
    const acc: Record<string, number> = {};
    for (const cl of c.sheet?.classes ?? []) {
      if (!cl.name?.trim()) continue;
      const key = cl.name.trim();
      const lvl = maxSpellLevelFor(cl);
      acc[key] = Math.max(acc[key] ?? -1, lvl);
    }
    return acc;
  }
  /** Spell is allowed if it appears on one of the character's class lists AND
   *  its level is within that class's spell access.  Custom (homebrew) classes
   *  bypass the class-list match and accept all spells up to their level cap.
   *  Characters with no classes yet fall back to the slot-level check. */
  function canLearn(c: Character, spell: { level: number; classes?: string[] | null }): boolean {
    const classes = (c.sheet?.classes ?? []).filter((cl) => cl.name?.trim());
    if (!classes.length) {
      return spell.level === 0 || spell.level <= maxSlotLevel(c);
    }
    // Custom classes grant blanket access up to their computed cap.
    for (const cl of classes) {
      if (isCustomClass(cl.name) && spell.level <= maxSpellLevelFor(cl)) return true;
    }
    // Standard classes require class-list match + level cap.
    const listed = (spell.classes ?? []).map((s) => s.toLowerCase());
    for (const cl of classes) {
      if (isCustomClass(cl.name)) continue;
      if (!listed.includes(cl.name.trim().toLowerCase())) continue;
      if (spell.level === 0 || spell.level <= maxSpellLevelFor(cl)) return true;
    }
    return false;
  }

  function maxSlotLevel(c: Character): number {
    // highest spell-slot level whose max > 0. Cantrips always allowed.
    const slots = c.sheet?.slots ?? {};
    let m = 0;
    for (const [k, v] of Object.entries(slots)) {
      const n = Number(k);
      if (v.max > 0 && n > m) m = n;
    }
    return m;
  }
  async function addSpell(c: Character, s: CharSpell) {
    if (hasSpell(c, s)) return;
    // Custom spells bypass class-list matching; still enforce slot cap.
    if (s.custom) {
      if (s.level > 0 && s.level > maxSlotLevel(c)) {
        alert($_('character.cannot_learn_slot').replace('{{name}}', s.name).replace('{{level}}', String(maxSlotLevel(c))));
        return;
      }
    } else if (!canLearn(c, { level: s.level, classes: s.classes })) {
      alert($_('character.cannot_learn_class').replace('{{name}}', s.name));
      return;
    }
    const list = [...(c.sheet?.spells ?? []), s];
    await patchSheet(c, (sh) => ({ ...sh, spells: list }));
  }
  async function removeSpell(c: Character, s: CharSpell) {
    if (!confirm($_('character.spell_remove_confirm'))) return;
    const k = spellKey(s);
    const list = (c.sheet?.spells ?? []).filter((x) => spellKey(x) !== k);
    await patchSheet(c, (sh) => ({ ...sh, spells: list }));
  }
  async function togglePrepared(c: Character, s: CharSpell) {
    const k = spellKey(s);
    const list = (c.sheet?.spells ?? []).map((x) =>
      spellKey(x) === k ? { ...x, prepared: !x.prepared } : x);
    await patchSheet(c, (sh) => ({ ...sh, spells: list }));
  }
  function isPassiveSpell(s: CharSpell): boolean {
    const d = (s.duration ?? '').toLowerCase();
    if (!d) return false;
    return !d.includes('instantaneous');
  }
  async function castSpell(c: Character, s: CharSpell) {
    if (s.level === 0) {
      if (isPassiveSpell(s) || s.concentration) {
        await patchSheet(c, (sh) => applyCastEffects(sh, s));
      }
      return;
    }
    // Check if higher slots are available — show upcast dialog
    const availableSlots = Object.entries(c.sheet?.slots ?? {})
      .filter(([lvl, sl]) => parseInt(lvl) >= s.level && sl.current > 0)
      .map(([lvl]) => parseInt(lvl))
      .sort((a, b) => a - b);
    if (availableSlots.length === 0) return;
    if (availableSlots.length === 1 && availableSlots[0] === s.level) {
      await castSpellAtLevel(c, s, s.level);
    } else {
      upcastSpell = { spell: s, c };
    }
  }

  async function castSpellAtLevel(c: Character, s: CharSpell, atLevel: number) {
    const key = String(atLevel);
    const sl = slot(c, key);
    if (sl.current <= 0) return;
    await patchSheet(c, (sh) => {
      const updated: Sheet = {
        ...sh,
        slots: { ...(sh.slots ?? {}), [key]: { current: sl.current - 1, max: sl.max } },
      };
      if (isPassiveSpell(s) || s.concentration) return applyCastEffects(updated, s);
      return updated;
    });
    upcastSpell = null;
  }
  function applyCastEffects(sh: Sheet, s: CharSpell): Sheet {
    let next: Sheet = { ...sh };
    if (s.concentration) {
      // starting a new concentration spell drops the previous one
      next = { ...next, concentration: { spell: s.name, since: new Date().toISOString() } };
    }
    if (isPassiveSpell(s)) {
      const list = next.active_effects ?? [];
      const already = list.some((e) => e.spell === s.name);
      if (!already) {
        next = { ...next, active_effects: [...list, { id: randomUUID(), spell: s.name, duration: s.duration ?? null, since: new Date().toISOString() }] };
      }
    }
    return next;
  }
  async function dropEffect(c: Character, id: string) {
    await patchSheet(c, (sh) => ({ ...sh, active_effects: (sh.active_effects ?? []).filter((e) => e.id !== id) }));
  }

  // ---- book search (debounced) ----
  let bookQuery = $state('');
  let bookLevel = $state<number | ''>('');
  let bookClass = $state<string>('');
  let bookExpanded = $state<string | null>(null);
  const CASTER_CLASSES = SPELLCASTER_CLASSES;
  let bookResults = $state<Array<{ slug: string; name: string; level: number; school: string; classes: string[]; ritual: boolean; concentration: boolean; description: string; casting_time?: string | null; range_text?: string | null; components?: string | null; duration?: string | null; higher_levels?: string | null; source?: string | null }>>([]);
  let bookLoading = $state(false);
  let bookTimer: ReturnType<typeof setTimeout> | undefined;
  async function runBookSearch() {
    const q = bookQuery.trim();
    const lv = bookLevel === '' ? undefined : Number(bookLevel);
    const cls = bookClass || undefined;
    if (!q && lv === undefined && !cls) { bookResults = []; return; }
    bookLoading = true;
    try {
      const r = await Spells.list({ q: q || undefined, level: lv, class: cls });
      // sort by level asc then name; return up to 100 so high-level spells
      // are reachable without typing the full name.
      bookResults = r.slice(0, 100);
    } finally { bookLoading = false; }
  }
  function onBookInput() {
    clearTimeout(bookTimer);
    bookTimer = setTimeout(runBookSearch, 250);
  }
  $effect(() => { void bookLevel; void bookClass; runBookSearch(); });

  // ---- custom spell form ----
  let customName = $state('');
  let customLevel = $state(0);
  let customDesc = $state('');

  // Seed features
  let seedOpen = $state(false);
  let seedClass = $state('');
  let seedSubclass = $state('');
  let seedSelected = $state<Set<string>>(new Set());

  const seedBaseFeatures = $derived(seedClass ? getBaseFeatures(seedClass) : []);
  const seedSubclassFeatures = $derived(seedClass && seedSubclass ? getSubclassFeatures(seedClass, seedSubclass) : []);
  const seedSubclasses = $derived(seedClass ? listSubclasses(seedClass) : []);

  function seedClassLevel(c: Character): number {
    if (!seedClass) return 0;
    const cl = (c.sheet?.classes ?? []).find(
      (x) => x.name.trim().toLowerCase() === seedClass.trim().toLowerCase()
    );
    return cl?.level ?? 0;
  }

  function featLevelExceeds(c: Character, featLevel: number): boolean {
    const clsLv = seedClassLevel(c);
    // If class not found in sheet, don't block (custom class)
    if (clsLv === 0) return false;
    return featLevel > clsLv;
  }

  function featureAlreadyExists(c: Character, name: string): boolean {
    const inFeatures = (c.sheet?.features ?? []).some((f) => f.name === name);
    const inResources = (c.sheet?.resources ?? []).some((r) => r.name.toLowerCase() === name.toLowerCase());
    return inFeatures || inResources;
  }

  async function applySeed(c: Character) {
    const allFeatures = [...seedBaseFeatures, ...seedSubclassFeatures];
    const toAdd = allFeatures.filter((f) => seedSelected.has(f.name) && !featureAlreadyExists(c, f.name) && !featLevelExceeds(c, f.level));
    if (toAdd.length === 0) return;

    const newFeatures = [
      ...(c.sheet?.features ?? []),
      ...toAdd.map((f) => ({
        id: randomUUID(),
        name: f.name,
        source: seedSubclass && seedSubclassFeatures.includes(f) ? `${seedClass} — ${seedSubclass}` : seedClass,
        description: f.description,
      })),
    ];

    // Features with uses → also create a Resource so they're trackable with pips.
    const newResources = [...(c.sheet?.resources ?? [])];
    for (const f of toAdd) {
      if (!f.uses || f.uses.reset === 'none') continue;
      const reset = f.uses.reset as 'short' | 'long';
      // Parse max: may be a plain number-like string (e.g. "1", "2") or descriptive.
      // Try to extract a number; fall back to 1.
      const maxNum = parseInt(f.uses.max, 10);
      const max = isFinite(maxNum) && maxNum > 0 ? maxNum : 1;
      newResources.push({ id: randomUUID(), name: f.name, current: max, max, reset });
    }

    await patchSheet(c, (s) => ({ ...s, features: newFeatures, resources: newResources }));
    seedOpen = false;
    seedSelected = new Set();
  }

  function toggleSeedAll(features: typeof seedBaseFeatures, c: Character) {
    const allNames = features
      .filter((f) => !featureAlreadyExists(c, f.name) && !featLevelExceeds(c, f.level))
      .map((f) => f.name);
    const allSelected = allNames.every((n) => seedSelected.has(n));
    const next = new Set(seedSelected);
    if (allSelected) allNames.forEach((n) => next.delete(n));
    else allNames.forEach((n) => next.add(n));
    seedSelected = next;
  }

  // Feats
  let featSearch = $state('');
  let featConfigFeat = $state<Feat | null>(null);
  let featConfigAbility = $state<string>('');
  let featConfigClass = $state<string>('');
  let featConfigDamage = $state<string>('');
  let featConfigSkills = $state<string[]>([]);

  const ABILITY_OPTIONS: { key: Ability; label: string }[] = [
    { key: 'str', label: 'STR' }, { key: 'dex', label: 'DEX' },
    { key: 'con', label: 'CON' }, { key: 'int', label: 'INT' },
    { key: 'wis', label: 'WIS' }, { key: 'cha', label: 'CHA' },
  ];
  const CLASS_OPTIONS = ['Bard', 'Cleric', 'Druid', 'Sorcerer', 'Warlock', 'Wizard'];
  const DAMAGE_OPTIONS = ['Acid', 'Cold', 'Fire', 'Lightning', 'Thunder'];

  function featsSearch(c: Character) {
    const q = featSearch.trim().toLowerCase();
    return FEATS.filter((f) => !q || f.name.toLowerCase().includes(q) || f.mechanics.toLowerCase().includes(q));
  }

  function charHasFeat(c: Character, key: string): boolean {
    return (c.sheet?.feats ?? []).some((f) => f.key === key);
  }

  /** Apply (or reverse) a feat's mechanical effects. Returns a new Sheet;
   *  does NOT mutate `sh` or any of its nested objects. */
  function applyFeatEffects(sh: Sheet, feat: Feat, config: { ability?: string; class_name?: string; damage_type?: string; skills?: string[] }, remove = false): Sheet {
    const mult = remove ? -1 : 1;
    const ab = { ...((sh.abilities ?? {}) as Record<string, number>) };
    const prof = { ...((sh.proficiencies ?? {}) as Record<string, string>) };
    const senses = { ...((sh.senses ?? {}) as Record<string, number>) };
    const saves = { ...((sh.saves ?? {}) as Record<string, boolean>) };
    const next: Sheet = { ...sh };

    if (feat.effects.ability) {
      const key = feat.effects.ability;
      const cur = ab[key] ?? 10;
      ab[key] = Math.max(1, Math.min(20, cur + mult));
    }
    if (feat.effects.ability_choice && config.ability) {
      const key = config.ability;
      const cur = ab[key] ?? 10;
      ab[key] = Math.max(1, Math.min(20, cur + mult));
    }
    if (feat.effects.initiative) {
      const cur = typeof next.initiative === 'number' ? next.initiative : 0;
      next.initiative = cur + mult * feat.effects.initiative;
    }
    if (feat.effects.speed) {
      const cur = typeof next.speed === 'number' ? next.speed : 30;
      next.speed = Math.max(0, cur + mult * feat.effects.speed);
    }
    if (feat.effects.passive_perception) {
      const cur = senses.passive_perception_bonus ?? 0;
      senses.passive_perception_bonus = cur + mult * feat.effects.passive_perception;
    }
    if (feat.effects.save_prof) {
      if (!remove) saves[feat.effects.save_prof] = true;
    }
    if (feat.effects.save_prof_from_config && config.ability) {
      const key = config.ability as Ability;
      if (!remove) saves[key] = true;
    }
    if (feat.effects.passive_investigation) {
      const cur = (senses as Record<string, number>).passive_investigation_bonus ?? 0;
      (senses as Record<string, number>).passive_investigation_bonus = cur + mult * feat.effects.passive_investigation;
    }
    if (feat.effects.ac_bonus) {
      const cur = (next as Record<string, unknown>).ac_bonus as number ?? 0;
      (next as Record<string, unknown>).ac_bonus = cur + mult * feat.effects.ac_bonus;
    }
    if (feat.effects.medium_armor_max_dex) {
      (next as Record<string, unknown>).medium_armor_max_dex_override = remove ? undefined : feat.effects.medium_armor_max_dex;
    }
    if (feat.effects.nonmagical_damage_reduction) {
      const cur = (next as Record<string, unknown>).nonmagical_damage_reduction as number ?? 0;
      (next as Record<string, unknown>).nonmagical_damage_reduction = remove
        ? Math.max(0, cur - feat.effects.nonmagical_damage_reduction)
        : cur + feat.effects.nonmagical_damage_reduction;
    }
    if (feat.effects.armor_prof) {
      const entry = feat.effects.armor_prof;
      if (remove) {
        prof.armor = (prof.armor ?? '').split(', ').filter((p) => p !== entry).join(', ');
        if (!prof.armor) delete prof.armor;
      } else {
        const cur = prof.armor ?? '';
        if (!cur.split(', ').includes(entry)) prof.armor = cur ? `${cur}, ${entry}` : entry;
      }
    }
    if (feat.effects.free_skills && config.skills?.length) {
      const skillMap = { ...(next.skills ?? {}) } as Record<string, 'prof' | 'expert'>;
      for (const sk of config.skills) {
        if (remove) { if (skillMap[sk] === 'prof') delete skillMap[sk]; }
        else if (!skillMap[sk]) skillMap[sk] = 'prof';
      }
      next.skills = skillMap;
    }
    next.abilities = ab as Sheet['abilities'];
    next.proficiencies = prof as Sheet['proficiencies'];
    next.senses = senses as Sheet['senses'];
    next.saves = saves as Sheet['saves'];
    return next;
  }

  async function takeFeat(c: Character, feat: Feat) {
    const config: { ability?: string; class_name?: string; damage_type?: string; skills?: string[] } = {};
    if (feat.effects.config_type === 'ability' || feat.effects.config_type === 'ability_choice') {
      if (!featConfigAbility) return;
      config.ability = featConfigAbility;
    }
    if (feat.effects.config_type === 'class') {
      if (!featConfigClass) return;
      config.class_name = featConfigClass;
    }
    if (feat.effects.config_type === 'damage_type') {
      if (!featConfigDamage) return;
      config.damage_type = featConfigDamage;
    }
    if (feat.effects.config_type === 'skills') {
      const needed = feat.effects.free_skills ?? 1;
      if (featConfigSkills.length !== needed) return;
      config.skills = [...featConfigSkills];
    }
    const newFeat = { id: randomUUID(), key: feat.key, config };
    const newFeats = [...(c.sheet?.feats ?? []), newFeat];
    let next = applyFeatEffects(c.sheet ?? {}, feat, config, false);
    if (feat.effects.resource) {
      const res = feat.effects.resource;
      const resources = [...(next.resources ?? [])];
      resources.push({ id: randomUUID(), name: res.name, current: res.max, max: res.max, reset: res.reset });
      next = { ...next, resources };
    }
    next = { ...next, feats: newFeats };
    await patchSheet(c, () => next);
    featConfigFeat = null;
    featConfigAbility = ''; featConfigClass = ''; featConfigDamage = ''; featConfigSkills = [];
  }

  /**
   * Strip features + resources that were seeded from a given class/subclass.
   * Called when a class entry is removed or its name/subclass changes.
   * oldClass  = previous class name (e.g. "Wizard")
   * oldSub    = previous subclass name (e.g. "School of Divination"), optional
   */
  function pruneClassData(
    s: Sheet,
    oldClass: string,
    oldSub?: string,
  ): Sheet {
    if (!oldClass.trim()) return s;
    const cls = oldClass.trim().toLowerCase();
    const sub = oldSub?.trim().toLowerCase();

    // Feature source patterns: "ClassName" or "ClassName — SubclassName"
    const features = (s.features ?? []).filter((f) => {
      const src = (f.source ?? '').toLowerCase();
      if (!src) return true;
      // Exact class match (base features)
      if (src === cls) return false;
      // Subclass feature: "wizard — school of divination"
      if (src.startsWith(cls + ' —')) return false;
      return true;
    });

    // Resources seeded from this class (by template name match).
    // templatesForClass returns resource names for a given class.
    const classTemplateNames = new Set(
      templatesForClass(oldClass).map((t) => t.name.trim().toLowerCase())
    );
    const resources = (s.resources ?? []).filter((r) => {
      return !classTemplateNames.has(r.name.trim().toLowerCase());
    });

    return { ...s, features, resources };
  }

  async function removeFeat(c: Character, featEntry: { id: string; key: string; config?: { ability?: string; class_name?: string; damage_type?: string; skills?: string[] } }) {
    if (!confirm($_('character.feat_remove') + '?')) return;
    const feat = featByKey(featEntry.key);
    if (!feat) return;
    let next = applyFeatEffects(c.sheet ?? {}, feat, featEntry.config ?? {}, true);
    if (feat.effects.resource) {
      const name = feat.effects.resource.name;
      next = { ...next, resources: (next.resources ?? []).filter((r) => r.name !== name) };
    }
    next = { ...next, feats: (next.feats ?? []).filter((f) => f.id !== featEntry.id) };
    await patchSheet(c, () => next);
  }
  async function addCustom(c: Character) {
    if (!customName.trim()) return;
    await addSpell(c, {
      name: customName.trim(),
      level: customLevel,
      custom: true,
      description: customDesc.trim() || undefined,
    });
    customName = ''; customLevel = 0; customDesc = '';
  }

  // group known spells by level for display
  function grouped(c: Character): Array<[number, CharSpell[]]> {
    const buckets: Record<number, CharSpell[]> = {};
    for (const s of c.sheet?.spells ?? []) {
      (buckets[s.level] ??= []).push(s);
    }
    return Object.keys(buckets)
      .map(Number)
      .sort((a, b) => a - b)
      .map((lv) => [lv, buckets[lv].slice().sort((a, b) => a.name.localeCompare(b.name))] as [number, CharSpell[]]);
  }

  let selectedSpell = $state<CharSpell | null>(null);
  let upcastSpell = $state<{ spell: CharSpell; c: Character } | null>(null);

  function canEdit(c: Character): boolean {
    // Only owners can modify their own character sheet. Master/admin observe
    // but cannot edit — use combat or NPC tools for their roles.
    return c.owner_id === auth.user?.id;
  }

  function canViewSpellbook(c: Character): boolean {
    return canEdit(c) || campaign().isMaster;
  }

  // ---- backend spellbook ----
  type SpellbookEntry = {
    spell_id: string;
    name: string;
    slug: string;
    level: number;
    prepared: boolean;
    notes: string | null;
  };

  let spellbook = $state<SpellbookEntry[]>([]);
  let spellbookLoading = $state(false);
  let spellbookSearch = $state('');
  let spellbookSearchResults = $state<Spell[]>([]);
  let spellbookSearchLoading = $state(false);
  let spellbookSearchTimer: ReturnType<typeof setTimeout> | undefined;

  async function loadSpellbook(c: Character) {
    spellbookLoading = true;
    try {
      spellbook = await Characters.spells.list(c.id);
    } catch (e) {
      console.error('Failed to load spellbook', e);
    } finally {
      spellbookLoading = false;
    }
  }

  async function runSpellbookSearch() {
    const q = spellbookSearch.trim();
    if (!q) { spellbookSearchResults = []; return; }
    spellbookSearchLoading = true;
    try {
      const r = await Spells.list({ q: q || undefined });
      spellbookSearchResults = r.slice(0, 50);
    } finally { spellbookSearchLoading = false; }
  }

  function onSpellbookSearchInput() {
    clearTimeout(spellbookSearchTimer);
    spellbookSearchTimer = setTimeout(runSpellbookSearch, 250);
  }

  async function addSpellbookSpell(c: Character, spell: Spell) {
    if (spellbook.some((s) => s.slug === spell.slug)) return;
    await Characters.spells.add(c.id, { spell_id: spell.slug, prepared: false, notes: '' });
    await loadSpellbook(c);
  }

  async function toggleSpellbookPrepared(c: Character, entry: SpellbookEntry) {
    await Characters.spells.update(c.id, entry.spell_id, { prepared: !entry.prepared });
    await loadSpellbook(c);
  }

  async function updateSpellbookNotes(c: Character, entry: SpellbookEntry, notes: string) {
    await Characters.spells.update(c.id, entry.spell_id, { notes: notes || null });
    await loadSpellbook(c);
  }

  async function removeSpellbookSpell(c: Character, entry: SpellbookEntry) {
    if (!confirm($_('character.spellbook_remove_confirm'))) return;
    await Characters.spells.remove(c.id, entry.spell_id);
    await loadSpellbook(c);
  }

  function groupedSpellbook(entries: SpellbookEntry[]): Array<[number, SpellbookEntry[]]> {
    const buckets: Record<number, SpellbookEntry[]> = {};
    for (const s of entries) {
      (buckets[s.level] ??= []).push(s);
    }
    return Object.keys(buckets)
      .map(Number)
      .sort((a, b) => a - b)
      .map((lv) => [lv, buckets[lv].slice().sort((a, b) => a.name.localeCompare(b.name))] as [number, SpellbookEntry[]]);
  }

  type Tab = 'vitals' | 'combat' | 'magic' | 'loot' | 'features' | 'story' | 'spellbook';
  let tab = $state<Tab>('vitals');

  // ---- equipment helpers ----
  let newEqName = $state('');
  let newEqQty = $state(1);
  let newEqWeight = $state<number | ''>('');
  async function addEq(c: Character) {
    if (!newEqName.trim()) return;
    const item = {
      id: randomUUID(),
      name: newEqName.trim(),
      qty: newEqQty,
      weight: newEqWeight === '' ? undefined : Number(newEqWeight),
      equipped: false,
    };
    await patchSheet(c, (s) => ({ ...s, equipment: [ ...(s.equipment ?? []), item ] }));
    newEqName = ''; newEqQty = 1; newEqWeight = '';
  }
  async function patchEq(c: Character, id: string, patch: Record<string, unknown>) {
    const next = (c.sheet?.equipment ?? []).map((it) => it.id === id ? { ...it, ...patch } : it);
    await patchSheet(c, (s) => ({ ...s, equipment: next }));
  }
  async function removeEq(c: Character, id: string) {
    if (!confirm($_('character.equipment_remove_confirm'))) return;
    const next = (c.sheet?.equipment ?? []).filter((it) => it.id !== id);
    await patchSheet(c, (s) => ({ ...s, equipment: next }));
  }
  async function addFromCatalog(c: Character, item: any) {
    await patchSheet(c, (s) => ({ ...s, equipment: [...(s.equipment ?? []), { id: randomUUID(), name: item.name, qty: 1, weight: item.weight_lb, equipped: true }] }));
    if (item.category === 'armor' && item.armor_type) {
      const at = item.armor_type;
      await patchSheet(c, (s) => ({ ...s, armor: { type: at, ac_base: item.ac_base ?? 10, max_dex: item.max_dex ?? 99, stealth_disadvantage: item.stealth_disadvantage ?? false }, ac: computeAC({ ...s, armor: { type: at, ac_base: item.ac_base ?? 10, max_dex: item.max_dex ?? 99 } }) }));
    }
    if (item.category === 'shield') {
      await patchSheet(c, (s) => ({ ...s, shield: true, ac: computeAC({ ...s, shield: true }) }));
    }
    if (item.category === 'weapon' && item.damage_die) {
      await patchSheet(c, (s) => ({ ...s, weapons: [...(s.weapons ?? []), { id: randomUUID(), name: item.name, damage: item.damage_die, damage_die: item.damage_die, versatile_die: item.versatile_die, damage_type: item.damage_type || 'bludgeoning', range: item.range_normal ? String(item.range_normal) + '/' + String(item.range_long || item.range_normal * 4) : 'melee', properties: (item.properties || []).join(', '), equipped: true }] }));
    }
  }

  // ---- potion helpers ----
  const POTION_PRESETS = [
    { name: $_('character.potion_healing'),         heal_dice: '2d4+2'  },
    { name: $_('character.potion_healing_greater'), heal_dice: '4d4+4'  },
    { name: $_('character.potion_healing_superior'), heal_dice: '8d4+8' },
    { name: $_('character.potion_healing_supreme'),  heal_dice: '10d4+20' },
  ];
  let newPotionName = $state('');
  let newPotionHealDice = $state('2d4+2');
  let newPotionQty = $state(1);
  let drinkResult = $state<{ name: string; rolled: number; hp_before: number; hp_after: number } | null>(null);
  let rollResult = $state<{ label: string; total: number; expr: string } | null>(null);
  let rollResultTimer: ReturnType<typeof setTimeout> | null = null;

  async function rollCheck(cid: string, expr: string, label: string, characterId: string) {
    const res = await Dice.roll(cid, expr, label, false, characterId);
    if (rollResultTimer) clearTimeout(rollResultTimer);
    rollResult = { label, total: res.total, expr };
    rollResultTimer = setTimeout(() => { rollResult = null; }, 5000);
  }

  async function addPotion(c: Character) {
    if (!newPotionName.trim()) return;
    const potion = { id: randomUUID(), name: newPotionName.trim(), qty: newPotionQty, heal_dice: newPotionHealDice };
    await patchSheet(c, (s) => ({ ...s, potions: [...(s.potions ?? []), potion] }));
    newPotionName = ''; newPotionQty = 1; newPotionHealDice = '2d4+2';
  }

  async function drinkPotion(c: Character, potion: { id: string; name: string; qty: number; heal_dice: string }) {
    if (potion.qty <= 0) return;
    const rollRes = await Dice.roll(cid, potion.heal_dice, potion.name, false, c.id);
    const healed = rollRes.total;
    const hpBefore = c.sheet?.hp?.current ?? 0;
    const hpMax = c.sheet?.hp?.max ?? 999;
    const hpAfter = Math.min(hpMax, hpBefore + healed);
    // Decrement qty and update HP atomically via patchSheet
    await patchSheet(c, (s) => ({
      ...s,
      hp: { ...s.hp, current: hpAfter },
      potions: (s.potions ?? []).map((p) =>
        p.id === potion.id ? { ...p, qty: p.qty - 1 } : p
      ).filter((p) => p.qty > 0),
    }));
    drinkResult = { name: potion.name, rolled: healed, hp_before: hpBefore, hp_after: hpAfter };
    setTimeout(() => drinkResult = null, 5000);
  }

  async function removePotion(c: Character, id: string) {
    await patchSheet(c, (s) => ({ ...s, potions: (s.potions ?? []).filter((p) => p.id !== id) }));
  }

  // ---- weapon helpers ----
  let newWpName = $state('');
  let newWpAtk = $state<number>(0);
  let newWpDmg = $state('');
  let newWpDamageDie = $state('');
  let newWpVersatileDie = $state('');
  let newWpDmgType = $state('');
  let newWpRange = $state('');
  let newWpProps = $state('');
  let newWpDesc = $state('');
  async function addWeapon(c: Character) {
    if (!newWpName.trim()) return;
    const w = {
      id: randomUUID(),
      name: newWpName.trim(),
      attack_bonus: newWpAtk,
      damage: newWpDmg.trim() || undefined,
      damage_die: newWpDamageDie.trim() || undefined,
      versatile_die: newWpVersatileDie.trim() || undefined,
      damage_type: newWpDmgType.trim() || undefined,
      range: newWpRange.trim() || undefined,
      properties: newWpProps.trim() || undefined,
      description: newWpDesc.trim() || undefined,
      equipped: false,
    };
    await patchSheet(c, (s) => ({ ...s, weapons: [ ...(s.weapons ?? []), w ] }));
    newWpName = ''; newWpAtk = 0; newWpDmg = ''; newWpDamageDie = ''; newWpVersatileDie = ''; newWpDmgType = ''; newWpRange = ''; newWpProps = ''; newWpDesc = '';
  }
  async function patchWeapon(c: Character, id: string, patch: Record<string, unknown>) {
    const next = (c.sheet?.weapons ?? []).map((it) => it.id === id ? { ...it, ...patch } : it);
    await patchSheet(c, (s) => ({ ...s, weapons: next }));
  }
  async function removeWeapon(c: Character, id: string) {
    if (!confirm($_('character.weapon_remove_confirm'))) return;
    const next = (c.sheet?.weapons ?? []).filter((it) => it.id !== id);
    await patchSheet(c, (s) => ({ ...s, weapons: next }));
  }

  const current = $derived(list[idx]);

  $effect(() => {
    const c = current;
    if (c && canViewSpellbook(c)) {
      loadSpellbook(c);
    }
  });
</script>

<section class="mx-auto max-w-6xl px-3 sm:px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-xl font-semibold">{$_('character.title')}</h2>
    {#if canCreate}
      <CollapsibleAdd label={$_('character.new')} title={$_('character.new')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2 sm:grid-cols-2">
            <input required placeholder={$_('character.name')} bind:value={newName}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <select bind:value={newRace} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value="">{$_('character.race')}</option>
              <optgroup label="PHB Races">
                <option value="Dragonborn">Dragonborn</option>
                <option value="Hill Dwarf">Hill Dwarf</option>
                <option value="Mountain Dwarf">Mountain Dwarf</option>
                <option value="High Elf">High Elf</option>
                <option value="Wood Elf">Wood Elf</option>
                <option value="Drow">Drow</option>
                <option value="Forest Gnome">Forest Gnome</option>
                <option value="Rock Gnome">Rock Gnome</option>
                <option value="Half-Elf">Half-Elf</option>
                <option value="Half-Orc">Half-Orc</option>
                <option value="Lightfoot Halfling">Lightfoot Halfling</option>
                <option value="Stout Halfling">Stout Halfling</option>
                <option value="Human">Human</option>
                <option value="Variant Human">Variant Human</option>
                <option value="Tiefling">Tiefling</option>
              </optgroup>
              <optgroup label="Additional Races">
                <option value="Aasimar">Aasimar</option>
                <option value="Protector Aasimar">Protector Aasimar</option>
                <option value="Scourge Aasimar">Scourge Aasimar</option>
                <option value="Fallen Aasimar">Fallen Aasimar</option>
                <option value="Bugbear">Bugbear</option>
                <option value="Centaur">Centaur</option>
                <option value="Changeling">Changeling</option>
                <option value="Deep Gnome">Deep Gnome</option>
                <option value="Eladrin">Eladrin</option>
                <option value="Fairy">Fairy</option>
                <option value="Firbolg">Firbolg</option>
                <option value="Githyanki">Githyanki</option>
                <option value="Githzerai">Githzerai</option>
                <option value="Goblin">Goblin</option>
                <option value="Hobgoblin">Hobgoblin</option>
                <option value="Kenku">Kenku</option>
                <option value="Kobold">Kobold</option>
                <option value="Lizardfolk">Lizardfolk</option>
                <option value="Minotaur">Minotaur</option>
                <option value="Orc">Orc</option>
                <option value="Satyr">Satyr</option>
                <option value="Shadar-kai">Shadar-kai</option>
                <option value="Tabaxi">Tabaxi</option>
                <option value="Tortle">Tortle</option>
                <option value="Triton">Triton</option>
                <option value="Warforged">Warforged</option>
                <option value="Yuan-ti Pureblood">Yuan-ti Pureblood</option>
              </optgroup>
              <optgroup label="Genasi">
                <option value="Air Genasi">Air Genasi</option>
                <option value="Earth Genasi">Earth Genasi</option>
                <option value="Fire Genasi">Fire Genasi</option>
                <option value="Water Genasi">Water Genasi</option>
              </optgroup>
            </select>
            <input type="number" min="1" max="20" placeholder={$_('character.level')} bind:value={newLevel}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <select bind:value={newAlignment} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value="">Alignment</option>
              <option value="Lawful Good">Lawful Good</option>
              <option value="Neutral Good">Neutral Good</option>
              <option value="Chaotic Good">Chaotic Good</option>
              <option value="Lawful Neutral">Lawful Neutral</option>
              <option value="True Neutral">True Neutral</option>
              <option value="Chaotic Neutral">Chaotic Neutral</option>
              <option value="Lawful Evil">Lawful Evil</option>
              <option value="Neutral Evil">Neutral Evil</option>
              <option value="Chaotic Evil">Chaotic Evil</option>
            </select>
            <div class="sm:col-span-2 flex justify-end">
              <button disabled={busy} class="rounded-md bg-violet-600 px-6 py-2 text-white disabled:opacity-50">
                {$_('common.create')}
              </button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>

  {#if !campaign().isMaster && limit > 1}
    <p class="mt-2 text-xs" style="color:#8b6914;">{$_('character.limit_info').replace('{{used}}', String(owned)).replace('{{max}}', String(limit))}</p>
  {:else if !campaign().isMaster && !canCreate && list.length > 0}
    <p class="mt-2 text-xs italic" style="color:#8b6914;">{$_('character.limit_reached')}</p>
  {/if}

  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if list.length === 0}
    <p class="mt-8 text-center italic" style="color:#8b6355;">{$_('character.empty')}</p>
  {:else}
    <!-- nav strip -->
    <div class="mt-5 flex items-center justify-between gap-3 rounded-md border border-neutral-800 bg-neutral-900 px-3 py-2">
      <button onclick={() => idx = (idx - 1 + list.length) % list.length}
        disabled={list.length < 2}
        class="inline-flex items-center gap-1 text-sm disabled:opacity-40"><ChevronLeft size={16} /> prev</button>
      <div class="text-sm font-display tracking-wider">
        {idx + 1} / {list.length} · {current.name}
      </div>
      <button onclick={() => idx = (idx + 1) % list.length}
        disabled={list.length < 2}
        class="inline-flex items-center gap-1 text-sm disabled:opacity-40">next <ChevronRight size={16} /></button>
    </div>

    {#if current}
      {@const c = current}
      {@const hp = c.sheet?.hp ?? {}}
      {@const hd = c.sheet?.hit_dice ?? {}}
      <article id="ob-sheet" class="mt-4 rounded-lg border border-neutral-800 bg-neutral-900 p-3 sm:p-6 lg:p-10 space-y-8 {canEdit(c) ? '' : 'readonly-sheet'}" style="position:relative;">
        <CharacterOnboarding character={{ id: c.id, name: c.name, race: c.race, level_total: c.level_total, sheet: c.sheet as Record<string,unknown> }} canEdit={canEdit(c)} onSwitchTab={(t) => tab = t as Tab} />
        <!-- identity -->
        <header class="flex justify-between items-start gap-4">
          <div class="flex items-start gap-4 min-w-0">
            {#if canEdit(c)}
              <div class="shrink-0">
                <ImageUpload value={(c.portrait_url as string | null) ?? null} kind="avatar" size={88}
                  onchange={async (url) => {
                    try {
                      // Also strip any legacy sheet.avatar_url so clearing truly removes the portrait.
                      const sheet = c.sheet?.avatar_url
                        ? { ...(c.sheet ?? {}), avatar_url: undefined }
                        : undefined;
                      await Characters.update(c.id, { portrait_url: url, clear_portrait: url == null, ...(sheet ? { sheet } : {}) });
                      await load();
                    }
                    catch (e) { error = (e as Error).message; }
                  }} />
              </div>
            {:else if c.portrait_url}
              <img src={c.portrait_url as string} alt="" class="h-22 w-22 rounded-full object-cover border border-amber-900 shrink-0" />
            {/if}
            <div class="min-w-0 pt-1">
              <div class="flex items-center gap-3 flex-wrap">
                <h3 class="text-2xl font-display font-bold leading-tight">{c.name}</h3>
                {#if !canEdit(c)}
                  <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:rgba(47,96,88,0.25);color:#6fa39a;border:1px solid #2f6058;">
                    {$_('character.read_only')}
                  </span>
                {/if}
                <span id="ob-level" class="lvl-badge" title={$_('character.level')}>
                  <span class="lvl-label">{$_('character.lv_short')}</span>
                  {#if canEdit(c)}
                    <input type="number" min="1" max="20" value={c.level_total}
                      onchange={(e) => patchField(c.id, 'level_total', +(e.currentTarget as HTMLInputElement).value)} />
                  {:else}
                    <span class="lvl-value">{c.level_total}</span>
                  {/if}
                </span>
                <!-- life + inspiration quick status -->
                <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                  style={c.sheet?.alive === false
                    ? 'background:#8b1a1a;color:#f4e4c1;'
                    : 'background:rgba(79,109,54,0.3);color:#8aa86f;border:1px solid #6b8a4f;'}>
                  {#if c.sheet?.alive === false}
                  <Skull size={12} />
                  {#if (c.sheet?.death_saves?.successes ?? 0) >= 3}
                    {$_('character.stabilized')}
                  {:else}
                    {$_('character.dead')}
                  {/if}
                {:else}<Heart size={12} fill="currentColor" /> {$_('character.alive')}{/if}
                </span>
                {#each ((c.sheet as Record<string,unknown>)?.conditions as string[] ?? []) as cond (cond)}
                  <span class="inline-flex items-center rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:rgba(139,26,26,0.2);color:#a93535;border:1px solid #8b1a1a;">
                    {cond}
                  </span>
                {/each}
                {#if canEdit(c)}
                  <button type="button" title={$_('character.inspiration')}
                    onclick={() => patchSheet(c, (s) => ({ ...s, inspiration: !s.inspiration }))}
                    class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style={c.sheet?.inspiration
                      ? 'background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;'
                      : 'background:rgba(139,105,20,0.1);color:#8b6914;border:1px solid rgba(139,105,20,0.4);'}>
                    <Star size={12} fill={c.sheet?.inspiration ? 'currentColor' : 'none'} />
                    {c.sheet?.inspiration ? $_('character.inspiration_active') : $_('character.inspiration_none')}
                  </button>
                  {#if c.sheet?.inspiration}
                    <button type="button" title={$_('character.inspiration_use')}
                      onclick={async () => {
                        await rollCheck(cid, '2d20kh1', $_('character.inspiration_roll'), c.id);
                        await patchSheet(c, (s) => ({ ...s, inspiration: false }));
                      }}
                      class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold"
                      style="background:rgba(139,105,20,0.15);color:#c9a84c;border:1px solid #c9a84c;">
                      <Zap size={10} /> {$_('character.inspiration_use')}
                    </button>
                  {/if}
                {:else if c.sheet?.inspiration}
                  <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;">
                    <Star size={12} fill="currentColor" /> {$_('character.inspiration_active')}
                  </span>
                {/if}
              </div>
              <div class="mt-1 flex items-center gap-2 flex-wrap text-sm text-neutral-400">
                {#if canEdit(c)}
                  <select id="ob-race" value={c.race ?? ''}
                    onchange={(e) => patchField(c.id, 'race', (e.currentTarget as HTMLSelectElement).value)}
                    class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    <option value="">Race</option>
                    <option value="Dragonborn">Dragonborn</option>
                    <option value="Hill Dwarf">Hill Dwarf</option>
                    <option value="Mountain Dwarf">Mountain Dwarf</option>
                    <option value="High Elf">High Elf</option>
                    <option value="Wood Elf">Wood Elf</option>
                    <option value="Drow">Drow</option>
                    <option value="Eladrin">Eladrin</option>
                    <option value="Forest Gnome">Forest Gnome</option>
                    <option value="Rock Gnome">Rock Gnome</option>
                    <option value="Half-Elf">Half-Elf</option>
                    <option value="Half-Orc">Half-Orc</option>
                    <option value="Lightfoot Halfling">Lightfoot Halfling</option>
                    <option value="Stout Halfling">Stout Halfling</option>
                    <option value="Human">Human</option>
                    <option value="Variant Human">Variant Human</option>
                    <option value="Tiefling">Tiefling</option>
                    <option value="Aasimar">Aasimar</option>
                    <option value="Bugbear">Bugbear</option>
                    <option value="Firbolg">Firbolg</option>
                    <option value="Goblin">Goblin</option>
                    <option value="Hobgoblin">Hobgoblin</option>
                    <option value="Kenku">Kenku</option>
                    <option value="Kobold">Kobold</option>
                    <option value="Lizardfolk">Lizardfolk</option>
                    <option value="Orc">Orc</option>
                    <option value="Tabaxi">Tabaxi</option>
                    <option value="Triton">Triton</option>
                    <option value="Yuan-ti Pureblood">Yuan-ti Pureblood</option>
                  </select>
                  <select value={c.sheet?.alignment ?? ''}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, alignment: (e.currentTarget as HTMLSelectElement).value || undefined }))}
                    class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    <option value="">Alignment</option>
                    <option value="Lawful Good">LG</option>
                    <option value="Neutral Good">NG</option>
                    <option value="Chaotic Good">CG</option>
                    <option value="Lawful Neutral">LN</option>
                    <option value="True Neutral">TN</option>
                    <option value="Chaotic Neutral">CN</option>
                    <option value="Lawful Evil">LE</option>
                    <option value="Neutral Evil">NE</option>
                    <option value="Chaotic Evil">CE</option>
                  </select>
                {:else}
                  <span>{c.race ?? '—'}</span>
                  {#if c.sheet?.alignment}<span>· {c.sheet.alignment}</span>{/if}
                {/if}
                {#if (c.sheet?.classes ?? []).some((cl) => cl.name?.trim())}
                  <span>· {(c.sheet?.classes ?? []).filter((cl) => cl.name?.trim()).map((cl) => `${cl.name}${cl.subclass ? ` (${cl.subclass})` : ''} ${cl.level}`).join(' / ')}</span>
                {/if}
              </div>

              {#if c.sheet?.concentration?.spell || (c.sheet?.active_effects ?? []).length}
                <div class="mt-2 flex flex-wrap items-center gap-1.5">
                  {#if c.sheet?.concentration?.spell}
                    <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                      style="background:rgba(74,127,118,0.25); color:#2f6058; border:1px solid #2f6058;"
                      title={$_('character.concentration')}>
                      <Brain size={12} /> {$_('character.concentrating').replace('{{name}}', c.sheet.concentration.spell)}
                      {#if canEdit(c)}
                        <button class="ml-1" title={$_('character.drop_concentration')}
                          onclick={() => patchSheet(c, (s) => ({ ...s, concentration: null }))}>
                          <X size={10} />
                        </button>
                      {/if}
                    </span>
                  {/if}
                  {#each c.sheet?.active_effects ?? [] as eff (eff.id)}
                    <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                      style="background:rgba(201,168,76,0.2); color:#6d510f; border:1px solid rgba(139,105,20,0.5);"
                      title={eff.duration ?? ''}>
                      <Sparkles size={12} /> {$_('character.active_spell').replace('{{name}}', eff.spell)}
                      {#if canEdit(c)}
                        <button class="ml-1" title={$_('common.end')} onclick={() => dropEffect(c, eff.id)}>
                          <X size={10} />
                        </button>
                      {/if}
                    </span>
                  {/each}
                </div>
              {/if}
              <div class="mt-2 flex flex-wrap gap-3 text-xs" style="color:#8b6914;">
                <span>{$_('character.prof')} <b style="color:#2c1810;">+{profBonus(c.level_total)}</b></span>
                <span>{$_('character.passive_perception')} <b style="color:#2c1810;">{passivePerception(c)}</b></span>
                {#if extraAttackCount(c) > 0}
                  <span>{$_('character.extra_attack')} <b style="color:#2c1810;">×{extraAttackCount(c) + 1}</b></span>
                {/if}
                {#if sneakAttackDice(c) > 0}
                  <span>{$_('character.sneak_attack')} <b style="color:#2c1810;">{sneakAttackDice(c)}d6</b></span>
                {/if}
                {#if martialArtsDie(c)}
                  <span>{$_('character.martial_arts')} <b style="color:#2c1810;">{martialArtsDie(c)}</b></span>
                {/if}
                {#if bardicInspirationDie(c)}
                  <span>{$_('character.bardic_inspiration_die')} <b style="color:#2c1810;">{bardicInspirationDie(c)}</b></span>
                {/if}
                {#if hasJackOfAllTrades(c)}
                  <span class="italic">{$_('character.jack_of_all_trades')}</span>
                {/if}
                {#if classLevel(c, 'barbarian') >= 7}
                  <span class="italic">{$_('character.feral_instinct')}</span>
                {/if}
                {#if rageDamageBonus(c) !== null}
                  <span>{$_('character.rage_damage')} <b style="color:#2c1810;">+{rageDamageBonus(c)}</b></span>
                {/if}
                {#if classLevel(c, 'barbarian') >= 9}
                  <span class="italic">{$_('character.brutal_critical')}</span>
                {/if}
                {#if classLevel(c, 'barbarian') >= 15}
                  <span class="italic">{$_('character.persistent_rage')}</span>
                {/if}
                {#if classLevel(c, 'barbarian') >= 20}
                  <span class="italic">{$_('character.primal_champion')}</span>
                {/if}
                {#if destroyUndeadCR(c)}
                  <span>{$_('character.destroy_undead')} <b style="color:#2c1810;">{destroyUndeadCR(c)}</b></span>
                {/if}
                {#if wildShapeCR(c)}
                  <span>{$_('character.wild_shape_cr')} <b style="color:#2c1810;">{wildShapeCR(c)}</b></span>
                {/if}
                {#if classLevel(c, 'monk') >= 5}
                  <span class="italic">{$_('character.stunning_strike')}</span>
                {/if}
                {#if classLevel(c, 'rogue') >= 5}
                  <span class="italic">{$_('character.uncanny_dodge')}</span>
                {/if}
                {#if classLevel(c, 'rogue') >= 14}
                  <span class="italic">{$_('character.blindsense')}</span>
                {/if}
                {#if classLevel(c, 'paladin') >= 3}
                  <span class="italic">{$_('character.divine_health')}</span>
                {/if}
                {#if classLevel(c, 'paladin') >= 10}
                  <span class="italic">{$_('character.aura_of_courage')}</span>
                {/if}
                {#if isChampionFighter(c)}
                  <span class="italic">{$_('character.remarkable_athlete')}</span>
                {/if}
                {#if charHasFeat(c, 'alert')}
                  <span class="italic">{$_('character.alert_surprise')}</span>
                {/if}
                {#if charHasFeat(c, 'heavy_armor_master')}
                  <span class="italic">{$_('character.heavy_armor_master_dr')}</span>
                {/if}
                {#if charHasFeat(c, 'tavern_brawler')}
                  <span class="italic">{$_('character.tavern_brawler_d4')}</span>
                {/if}
                {#if charHasFeat(c, 'charger')}
                  <span class="italic">{$_('character.feat_charger')}</span>
                {/if}
                {#if charHasFeat(c, 'crossbow_expert')}
                  <span class="italic">{$_('character.feat_crossbow_expert')}</span>
                {/if}
                {#if charHasFeat(c, 'defensive_duelist')}
                  <span class="italic">{$_('character.feat_defensive_duelist')}</span>
                {/if}
                {#if charHasFeat(c, 'great_weapon_master')}
                  <span class="italic">{$_('character.feat_gwm')}</span>
                {/if}
                {#if charHasFeat(c, 'healer')}
                  <span class="italic">{$_('character.feat_healer')}</span>
                {/if}
                {#if charHasFeat(c, 'mage_slayer')}
                  <span class="italic">{$_('character.feat_mage_slayer')}</span>
                {/if}
                {#if charHasFeat(c, 'mounted_combatant')}
                  <span class="italic">{$_('character.feat_mounted_combatant')}</span>
                {/if}
                {#if charHasFeat(c, 'polearm_master')}
                  <span class="italic">{$_('character.feat_polearm_master')}</span>
                {/if}
                {#if charHasFeat(c, 'savage_attacker')}
                  <span class="italic">{$_('character.feat_savage_attacker')}</span>
                {/if}
                {#if charHasFeat(c, 'sentinel')}
                  <span class="italic">{$_('character.feat_sentinel')}</span>
                {/if}
                {#if charHasFeat(c, 'sharpshooter')}
                  <span class="italic">{$_('character.feat_sharpshooter')}</span>
                {/if}
                {#if charHasFeat(c, 'shield_master')}
                  <span class="italic">{$_('character.feat_shield_master')}</span>
                {/if}
                {#if charHasFeat(c, 'skulker')}
                  <span class="italic">{$_('character.feat_skulker')}</span>
                {/if}
                {#if charHasFeat(c, 'spell_sniper')}
                  <span class="italic">{$_('character.feat_spell_sniper')}</span>
                {/if}
                {#if (c.race?.toLowerCase() ?? '').includes('half-orc')}
                  <span class="italic">{$_('character.savage_attacks')}</span>
                {/if}
                {#if (c.sheet as Record<string,unknown>)?.swim_speed}
                  <span>{$_('character.swim_speed')} <b style="color:#2c1810;">{(c.sheet as Record<string,unknown>).swim_speed as number} ft</b></span>
                {/if}
                {#if (c.sheet as Record<string,unknown>)?.climb_speed}
                  <span>{$_('character.climb_speed')} <b style="color:#2c1810;">{(c.sheet as Record<string,unknown>).climb_speed as number} ft</b></span>
                {/if}
                {#if (c.sheet as Record<string,unknown>)?.fly_speed}
                  <span>{$_('character.fly_speed')} <b style="color:#2c1810;">{(c.sheet as Record<string,unknown>).fly_speed as number} ft</b></span>
                {/if}
                {#if (c.race?.toLowerCase() ?? '').includes('wood elf')}
                  <span class="italic">{$_('character.mask_of_wild')}</span>
                {/if}
                {#if c.sheet?.armor?.type === 'heavy' || c.sheet?.armor?.stealth_disadvantage}
                  <span class="italic" style="color:#a93535;">{$_('character.stealth_disadvantage')}</span>
                {/if}
                {#if (c.race?.toLowerCase() ?? '').includes('drow') && (c.sheet as Record<string,unknown>)?.sunlight_sensitivity}
                  <span class="italic" style="color:#a93535;">{$_('character.sunlight_sensitivity')}</span>
                {/if}
                {#if campaign().leveling === 'xp'}
                  <span>{$_('character.xp')} <b style="color:#2c1810;">{c.sheet?.xp ?? 0}</b></span>
                {:else}
                  <span class="italic">{$_('character.milestone')}</span>
                {/if}
              </div>

              {#if (c.sheet?.resources ?? []).some((r) => classResourceNames(c).has(r.name.trim().toLowerCase()))}
                <div class="mt-2 flex flex-wrap gap-1.5">
                  {#each (c.sheet?.resources ?? []).filter((r) => classResourceNames(c).has(r.name.trim().toLowerCase())) as r (r.id)}
                    <span class="inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                      style={r.current <= 0
                        ? 'background:rgba(139,26,26,0.2);color:#a93535;border:1px solid #8b1a1a;'
                        : 'background:rgba(201,168,76,0.22);color:#6d510f;border:1px solid rgba(139,105,20,0.5);'}
                      title="{r.reset ?? 'manual'} rest reset">
                      {#if canEdit(c)}
                        <button type="button" class="px-1 hover:opacity-70" aria-label="-1"
                          disabled={r.current <= 0}
                          onclick={() => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, current: Math.max(0, x.current - 1) } : x) }))}>−</button>
                      {/if}
                      {r.name}: <b style="color:#2c1810;">{r.current}/{r.max}</b>
                      {#if canEdit(c)}
                        <button type="button" class="px-1 hover:opacity-70" aria-label="+1"
                          disabled={r.current >= r.max}
                          onclick={() => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, current: Math.min(x.max, x.current + 1) } : x) }))}>+</button>
                      {/if}
                    </span>
                  {/each}
                </div>
              {/if}
            </div>
          </div>
          <div class="flex items-center gap-2">
            {#if canEdit(c)}
              <button class="inline-flex items-center gap-1 rounded px-2.5 py-1 text-xs"
                style="background:rgba(111,163,154,0.25);color:#2f6058;border:1px solid #2f6058;"
                onclick={() => shortRest(c)} title={$_('character.refresh_short')}>
                <Bed size={12} /> {$_('character.short_rest')}
              </button>
              <button class="inline-flex items-center gap-1 rounded px-2.5 py-1 text-xs"
                style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;"
                onclick={() => longRest(c)} title={$_('character.refresh_long')}>
                <Moon size={12} /> {$_('character.long_rest')}
              </button>
              <button class="inline-flex items-center gap-1 text-xs text-red-400" onclick={() => remove(c)}>
                <Trash2 size={12} /> {$_('common.delete')}
              </button>
            {/if}
          </div>
        </header>

        <!-- tab bar -->
        <div class="sheet-tabs">
          <button id="ob-tab-vitals" class="sheet-tab {tab === 'vitals' ? 'active' : ''}" onclick={() => tab = 'vitals'}>{$_('character.tab_vitals')}</button>
          <button id="ob-tab-combat" class="sheet-tab {tab === 'combat' ? 'active' : ''}" onclick={() => tab = 'combat'}>{$_('character.tab_combat')}</button>
          <button id="ob-tab-magic" class="sheet-tab {tab === 'magic'  ? 'active' : ''}" onclick={() => tab = 'magic'}>{$_('character.tab_magic')}</button>
          <button id="ob-tab-loot" class="sheet-tab {tab === 'loot'   ? 'active' : ''}" onclick={() => tab = 'loot'}>{$_('character.tab_loot')}</button>
          <button id="ob-tab-features" class="sheet-tab {tab === 'features' ? 'active' : ''}" onclick={() => tab = 'features'}>{$_('character.tab_features')}</button>
          <button id="ob-tab-story" class="sheet-tab {tab === 'story'  ? 'active' : ''}" onclick={() => tab = 'story'}>{$_('character.tab_story')}</button>
          {#if canViewSpellbook(c)}
            <button class="sheet-tab {tab === 'spellbook' ? 'active' : ''}" onclick={() => tab = 'spellbook'}>{$_('character.tab_spellbook')}</button>
          {/if}
        </div>

        {#if tab === 'vitals'}
        <!-- vitals block -->
        <section class="sheet-block">
          <h4 class="sheet-h">{$_('character.vitals')}</h4>
          <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
            <Stepper label={$_('character.hp_current')} value={hp.current ?? 0} min={0} max={hp.max ?? 999}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, current: v } }))} />
            <div>
              <Stepper label={$_('character.hp_max')} value={hp.max ?? 0} min={0}
                onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, max: v, current: Math.min(s.hp?.current ?? 0, v) } }))} />
              {#if canEdit(c)}
                <div class="text-[10px] mt-1" style="color:#8b6355;">
                  Computed: <b style="color:#2c1810;">{computedMaxHP(c)}</b>
                  <button type="button" class="underline ml-1" style="color:#8b6914;"
                    onclick={() => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, max: computedMaxHP(c), current: Math.min(s.hp?.current ?? 0, computedMaxHP(c)) } }))}>
                    apply
                  </button>
                </div>
              {/if}
            </div>
            <Stepper label={$_('character.temp_hp')} value={hp.temp ?? 0} min={0}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, temp: v } }))} />
            <Stepper label={$_('character.hp_max_reduction')} value={(c.sheet as Record<string,unknown>)?.hp_max_reduction as number ?? 0} min={0} max={999}
              onchange={(v) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), hp_max_reduction: v > 0 ? v : undefined } as Sheet))} />
          </div>
          {#if (hp.max ?? 0) > 0}
            {@const cur = hp.current ?? 0}
            {@const mx  = hp.max ?? 1}
            {@const tmp = hp.temp ?? 0}
            {@const reduction = (c.sheet as Record<string,unknown>)?.hp_max_reduction as number ?? 0}
            {@const effMax = Math.max(1, mx - reduction)}
            {@const denom = Math.max(mx, cur + tmp, 1)}
            {@const pct = Math.max(0, Math.min(100, (cur / denom) * 100))}
            {@const tmpPct = Math.max(0, Math.min(100 - pct, (tmp / denom) * 100))}
            {@const redPct = Math.max(0, Math.min(100, (reduction / denom) * 100))}
            <div class="mt-3 h-3 rounded-full overflow-hidden relative"
              style="background:#2c1810; border:1px solid rgba(139,105,20,0.55);">
              <div class="absolute inset-y-0 left-0 transition-[width] duration-200"
                style={`width:${pct}%; background:linear-gradient(180deg,#8aa86f,#4f6d36);`}></div>
              {#if tmp > 0}
                <div class="absolute inset-y-0 transition-[width] duration-200"
                  style={`left:${pct}%; width:${tmpPct}%; background:linear-gradient(180deg,#a8d4cb,#4a7f76); box-shadow:inset 0 1px 0 rgba(255,248,220,0.35);`}
                  title={$_('character.temporary_hp')}></div>
              {/if}
              {#if reduction > 0}
                <div class="absolute inset-y-0 right-0 transition-[width] duration-200"
                  style={`width:${redPct}%; background:repeating-linear-gradient(45deg,#6b1a1a,#6b1a1a 2px,#3a0e0e 2px,#3a0e0e 5px); opacity:0.85;`}
                  title="{$_('character.hp_max_reduction')}: -{reduction}"></div>
              {/if}
            </div>
            <div class="mt-1 text-xs flex items-center gap-2" style="color:#8b6355;">
              <span>{cur}/{effMax}{reduction > 0 ? ` (${mx})` : ''}</span>
              {#if tmp > 0}
                <span class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-bold"
                  style="background:rgba(74,127,118,0.25); color:#2f6058; border:1px solid #2f6058;">
                  +{tmp} {$_('character.temp_short')}
                </span>
                <span>→ {cur + tmp} {$_('character.effective')}</span>
              {/if}
              {#if reduction > 0}
                <span class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-bold"
                  style="background:rgba(107,26,26,0.2); color:#8b1a1a; border:1px solid #8b1a1a;">
                  -{reduction} {$_('character.hp_max_reduction')}
                </span>
              {/if}
            </div>
          {/if}
        </section>

        <div class="space-y-8">
          <!-- LEFT → now full-width under vitals tab -->
          <div class="space-y-8">
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.hit_dice')}</h4>
              {#if c.sheet?.hit_dice?.pools}
                {#each c.sheet.hit_dice.pools as pool (pool.name)}
                  <div class="flex items-center gap-3 mb-1.5">
                    <span class="text-xs font-bold w-20 truncate" style="color:#8b6914;">{pool.name}</span>
                    <Stepper compact label={pool.die} value={pool.current} min={0} max={pool.max}
                      onchange={(v) => patchSheet(c, (s) => {
                        const p = (s.hit_dice?.pools ?? []).map((x: any) => x.name === pool.name ? { ...x, current: v } : x);
                        return { ...s, hit_dice: { ...s.hit_dice, pools: p } };
                      })} />
                    <span class="text-[10px]" style="color:#8b6355;">/{pool.max}</span>
                  </div>
                {/each}
              {:else}
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-4">
                <Stepper label={$_('character.hit_dice_current')} value={hd.current ?? 0} min={0} max={hd.max ?? 20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, current: v } }))} />
                <Stepper label={$_('character.hit_dice_max')} value={hd.max ?? 0} min={0} max={20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, max: v } }))} />
                <label class="flex flex-col">
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{$_('character.hit_dice_type')}</span>
                  <select value={hd.die ?? 'd8'}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, die: (e.currentTarget as HTMLSelectElement).value } }))}
                    class="mt-0.5 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    {#each ['d6','d8','d10','d12'] as d (d)}<option>{d}</option>{/each}
                  </select>
                </label>
              </div>
              {/if}
            </section>

            <!-- potions section -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5">⚗ {$_('character.potions')}</h4>

              {#if drinkResult}
                <div class="mb-2 rounded px-3 py-2 text-sm font-bold"
                  style="background:rgba(74,127,118,0.2);border:1px solid #2f6058;color:#2f6058;">
                  🧪 {drinkResult.name}: +{drinkResult.rolled} PF
                  ({drinkResult.hp_before} → {drinkResult.hp_after})
                </div>
              {/if}

              <!-- existing potions list -->
              {#if (c.sheet?.potions ?? []).length > 0}
                <div class="space-y-1.5 mb-3">
                  {#each c.sheet?.potions ?? [] as pot (pot.id)}
                    <div class="flex items-center gap-2 rounded px-2 py-1.5"
                      style="background:rgba(44,24,16,0.5);border:1px solid rgba(139,105,20,0.3);">
                      <!-- drink button -->
                      <button type="button"
                        onclick={() => drinkPotion(c, pot)}
                        disabled={pot.qty <= 0 || !canEdit(c)}
                        title={$_('character.potion_drink')}
                        class="shrink-0 rounded px-2 py-0.5 text-xs font-bold disabled:opacity-40"
                        style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;">
                        🍶 {$_('character.potion_drink')}
                      </button>
                      <!-- name -->
                      <span class="flex-1 text-sm font-semibold" style="color:#f4e4c1;">{pot.name}</span>
                      <!-- dice -->
                      <span class="text-xs font-mono px-1.5 py-0.5 rounded"
                        style="background:rgba(139,105,20,0.25);color:#f4e4c1;border:1px solid rgba(139,105,20,0.4);">{pot.heal_dice}</span>
                      <!-- qty controls -->
                      <div class="flex items-center gap-0.5 shrink-0">
                        <button type="button"
                          onclick={() => patchSheet(c, (s) => ({ ...s, potions: (s.potions ?? []).map(p => p.id === pot.id ? { ...p, qty: Math.max(0, p.qty - 1) } : p) }))}
                          class="w-5 h-5 rounded-full text-xs font-bold flex items-center justify-center"
                          style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;">−</button>
                        <span class="w-6 text-center text-sm font-bold tabular-nums" style="color:#f4e4c1;">{pot.qty}</span>
                        <button type="button"
                          onclick={() => patchSheet(c, (s) => ({ ...s, potions: (s.potions ?? []).map(p => p.id === pot.id ? { ...p, qty: p.qty + 1 } : p) }))}
                          class="w-5 h-5 rounded-full text-xs font-bold flex items-center justify-center"
                          style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;">+</button>
                      </div>
                      <!-- remove -->
                      <button type="button" onclick={() => removePotion(c, pot.id)}
                        class="shrink-0 text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic mb-3" style="color:#8b6355;">{$_('character.potions_empty')}</p>
              {/if}

              <!-- add potion form -->
              {#if canEdit(c)}
                <div class="flex flex-wrap gap-2 items-end">
                  <!-- preset selector -->
                  <select
                    onchange={(e) => {
                      const preset = POTION_PRESETS.find(p => p.name === (e.currentTarget as HTMLSelectElement).value);
                      if (preset) { newPotionName = preset.name; newPotionHealDice = preset.heal_dice; }
                      (e.currentTarget as HTMLSelectElement).value = '';
                    }}
                    class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"
                    style="color:#f4e4c1;">
                    <option value="">{$_('character.potion_preset')}</option>
                    {#each POTION_PRESETS as p (p.name)}
                      <option value={p.name}>{p.name} ({p.heal_dice})</option>
                    {/each}
                  </select>
                  <input placeholder={$_('character.potion_name_ph')} bind:value={newPotionName}
                    class="flex-1 min-w-32 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                  <input placeholder={$_('character.potion_dice_ph')} bind:value={newPotionHealDice}
                    class="w-20 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm font-mono" />
                  <div class="w-16">
                    <Stepper compact label="Qty" value={newPotionQty} min={1} onchange={(v) => newPotionQty = v} />
                  </div>
                  <button type="button" onclick={() => addPotion(c)}
                    class="rounded px-3 py-1 text-sm inline-flex items-center gap-1"
                    style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;font-weight:700;">
                    <Plus size={14} /> {$_('common.add')}
                  </button>
                </div>
              {/if}
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.status')}</h4>
              <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <Stepper label={$_('character.exhaustion')} value={c.sheet?.exhaustion ?? 0} min={0} max={6}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, exhaustion: v }))} />
                <label class="flex items-center gap-2 cursor-pointer">
                  <input type="checkbox" checked={c.sheet?.inspiration ?? false}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, inspiration: (e.currentTarget as HTMLInputElement).checked }))}
                    class="w-4 h-4 accent-amber-600" />
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">Inspiration ⭐</span>
                </label>
                {#if (c.race?.toLowerCase() ?? '').includes('drow')}
                  <label class="flex items-center gap-2 cursor-pointer">
                    <input type="checkbox" checked={!!(c.sheet as Record<string,unknown>)?.sunlight_sensitivity}
                      onchange={(e) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), sunlight_sensitivity: (e.currentTarget as HTMLInputElement).checked || undefined } as Sheet))}
                      class="w-4 h-4 accent-red-600" />
                    <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#a93535;">{$_('character.sunlight_sensitivity')}</span>
                  </label>
                {/if}
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-2" style="color:#8b6914;">{$_('character.death_saves')}</div>
                  <div class="flex items-center gap-3">
                    <span class="inline-flex items-center gap-1.5" title={$_('character.death_save_success_title')}>
                      <span class="text-[10px] uppercase font-display font-bold" style="color:#6b8a4f;">{$_('character.death_save_success_letter')}</span>
                      {#each [0,1,2] as i (i)}
                        <button type="button" aria-label={$_('character.death_save_success_aria').replace('{{n}}', String(i+1))}
                          onclick={() => setDeathSave(c, 'successes', i)}
                          class="ds-dot"
                          style={`border-color:#6b8a4f; background:${i < (c.sheet?.death_saves?.successes ?? 0) ? 'radial-gradient(circle at 35% 30%, #a8c88f 0%, #6b8a4f 60%, #3a5226 100%)' : 'transparent'};`}></button>
                      {/each}
                    </span>
                    <span class="inline-flex items-center gap-1.5" title={$_('character.death_save_failure_title')}>
                      <span class="text-[10px] uppercase font-display font-bold" style="color:#a93535;">{$_('character.death_save_failure_letter')}</span>
                      {#each [0,1,2] as i (i)}
                        <button type="button" aria-label={$_('character.death_save_failure_aria').replace('{{n}}', String(i+1))}
                          onclick={() => setDeathSave(c, 'failures', i)}
                          class="ds-dot"
                          style={`border-color:#a93535; background:${i < (c.sheet?.death_saves?.failures ?? 0) ? 'radial-gradient(circle at 35% 30%, #d47a7a 0%, #a93535 60%, #4e0a0a 100%)' : 'transparent'};`}></button>
                      {/each}
                    </span>
                    {#if (c.sheet?.death_saves?.successes ?? 0) > 0 || (c.sheet?.death_saves?.failures ?? 0) > 0}
                      <button type="button" class="text-[11px] underline ml-auto" style="color:#8b6914;"
                        onclick={() => patchSheet(c, (s) => ({ ...s, death_saves: { successes: 0, failures: 0 } }))}>{$_('character.death_saves_reset')}</button>
                    {/if}
                  </div>
                </div>
              </div>
            </section>

            <!-- senses + languages + proficiencies -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.senses')}</h4>
              <div class="grid sm:grid-cols-2 gap-3">
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">{$_('character.senses_ft')}</div>
                  <div class="grid grid-cols-2 gap-2 text-sm">
                    {#each ['darkvision','blindsight','truesight','tremorsense'] as k (k)}
                      <label class="flex items-center justify-between gap-2">
                        <span class="text-xs" style="color:#8b6914;">{$_(`character.${k}`)}</span>
                        <input type="number" min="0" step="5"
                          value={(c.sheet?.senses as Record<string, number | undefined> | undefined)?.[k] ?? 0}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, senses: { ...(s.senses ?? {}), [k]: +(e.currentTarget as HTMLInputElement).value } }))}
                          class="w-16 text-center text-sm" />
                      </label>
                    {/each}
                    <label class="flex items-center justify-between gap-2 col-span-2">
                      <span class="text-xs" style="color:#8b6914;">{$_('character.passive_perc_bonus')}</span>
                      <input type="number" value={c.sheet?.senses?.passive_perception_bonus ?? 0}
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, senses: { ...(s.senses ?? {}), passive_perception_bonus: +(e.currentTarget as HTMLInputElement).value } }))}
                        class="w-16 text-center text-sm" />
                    </label>
                  </div>
                  <div class="mt-2 text-xs" style="color:#8b6355;">
                    {$_('character.passive_perception_total')}: <b style="color:#2c1810;">{passivePerception(c)}</b>
                  </div>
                </div>
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">{$_('character.languages')}</div>
                  {#if canEdit(c)}
                    <input type="text" value={c.sheet?.languages ?? ''} placeholder={$_('character.languages_ph')}
                      onchange={(e) => patchSheet(c, (s) => ({ ...s, languages: (e.currentTarget as HTMLInputElement).value }))}
                      class="w-full text-sm" />
                  {/if}
                  {#if c.sheet?.languages}
                    <div class="flex flex-wrap gap-1 mt-1">
                      {#each c.sheet.languages.split(',').map((t) => t.trim()).filter(Boolean) as lang (lang)}
                        <span class="rounded px-1.5 py-0.5 text-[10px]" style="background:rgba(139,105,20,0.15);color:#6d510f;border:1px solid rgba(139,105,20,0.3);">{lang}</span>
                      {/each}
                    </div>
                  {/if}
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mt-3 mb-1" style="color:#8b6914;">{$_('character.proficiencies')}</div>
                  {#each ['armor','weapons','tools'] as k (k)}
                    {@const profVal = (c.sheet?.proficiencies as Record<string, string | undefined> | undefined)?.[k] ?? ''}
                    <div class="mb-2">
                      <div class="flex items-center gap-2 mb-1">
                        <span class="w-16 text-xs shrink-0" style="color:#8b6914;">{$_(`character.prof_${k}`)}</span>
                        {#if canEdit(c)}
                          <input type="text" value={profVal}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, proficiencies: { ...(s.proficiencies ?? {}), [k]: (e.currentTarget as HTMLInputElement).value } }))}
                            class="flex-1 text-sm" />
                        {/if}
                      </div>
                      {#if profVal}
                        <div class="flex flex-wrap gap-1 ml-16">
                          {#each profVal.split(',').map((t) => t.trim()).filter(Boolean) as tag (tag)}
                            <span class="rounded px-1.5 py-0.5 text-[10px]" style="background:rgba(139,105,20,0.15);color:#6d510f;border:1px solid rgba(139,105,20,0.3);">{tag}</span>
                          {/each}
                        </div>
                      {/if}
                    </div>
                  {/each}
                </div>
                <!-- tool proficiencies -->
                <div class="mt-3">
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">Tool Proficiencies</div>
                  {#each c.sheet?.tool_proficiencies ?? [] as tp, i}
                    {@const toolMod = abilityModForChar(c, tp.ability ?? 'dex') + (tp.proficient ? profBonus(c.level_total) : 0) + (tp.expert ? profBonus(c.level_total) : 0)}
                    <div class="flex items-center gap-2 mb-1">
                      <input type="text" value={tp.name} placeholder="Tool name"
                        onchange={(e) => patchSheet(c, (s) => {
                          const list = [...(s.tool_proficiencies ?? [])];
                          list[i] = { ...tp, name: (e.currentTarget as HTMLInputElement).value };
                          return { ...s, tool_proficiencies: list };
                        })}
                        class="flex-1 text-sm" />
                      <select value={tp.ability ?? 'dex'}
                        onchange={(e) => patchSheet(c, (s) => {
                          const list = [...(s.tool_proficiencies ?? [])];
                          list[i] = { ...tp, ability: (e.currentTarget as HTMLSelectElement).value as Ability };
                          return { ...s, tool_proficiencies: list };
                        })}
                        class="text-xs rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5">
                        {#each ABILITIES as ab}<option value={ab}>{ab.toUpperCase()}</option>{/each}
                      </select>
                      <span class="text-xs font-bold font-mono w-6 text-right" style="color:{toolMod >= 0 ? '#4f6d36' : '#8b1a1a'};">{toolMod >= 0 ? `+${toolMod}` : toolMod}</span>
                      <label class="flex items-center gap-1 text-xs">
                        <input type="checkbox" checked={tp.proficient ?? false}
                          onchange={(e) => patchSheet(c, (s) => {
                            const list = [...(s.tool_proficiencies ?? [])];
                            list[i] = { ...tp, proficient: (e.currentTarget as HTMLInputElement).checked };
                            return { ...s, tool_proficiencies: list };
                          })} />
                        Prof
                      </label>
                      <label class="flex items-center gap-1 text-xs">
                        <input type="checkbox" checked={tp.expert ?? false}
                          onchange={(e) => patchSheet(c, (s) => {
                            const list = [...(s.tool_proficiencies ?? [])];
                            list[i] = { ...tp, expert: (e.currentTarget as HTMLInputElement).checked };
                            return { ...s, tool_proficiencies: list };
                          })} />
                        Expert
                      </label>
                      <button type="button" class="text-red-400"
                        onclick={() => patchSheet(c, (s) => ({ ...s, tool_proficiencies: (s.tool_proficiencies ?? []).filter((_, j) => j !== i) }))}>
                        <Trash2 size={12} />
                      </button>
                    </div>
                  {/each}
                  <button type="button"
                    onclick={() => patchSheet(c, (s) => ({ ...s, tool_proficiencies: [...(s.tool_proficiencies ?? []), { name: '', ability: 'dex', proficient: true }] }))}
                    class="mt-1 inline-flex items-center gap-1 rounded px-2 py-0.5 text-xs"
                    style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;">
                    <Plus size={12} /> Add tool
                  </button>
                </div>
              </div>
            </section>

            <!-- conditions -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.conditions')}</h4>
              <div class="flex flex-wrap gap-1.5">
                {#each CONDITIONS_LIST as cond (cond)}
                  {@const active = ((c.sheet as Record<string,unknown>)?.conditions as string[] ?? []).includes(cond)}
                  <button type="button"
                    onclick={() => canEdit(c) && patchSheet(c, (s) => {
                      const cur = (s as Record<string,unknown>).conditions as string[] ?? [];
                      const next = active ? cur.filter((x) => x !== cond) : [...cur, cond];
                      return { ...(s as Record<string,unknown>), conditions: next.length ? next : undefined } as Sheet;
                    })}
                    class="rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest transition-colors"
                    style={active
                      ? 'background:rgba(139,26,26,0.3);color:#a93535;border:1px solid #8b1a1a;'
                      : 'background:rgba(139,105,20,0.08);color:#6d510f;border:1px solid rgba(139,105,20,0.3);'}>
                    {cond}
                  </button>
                {/each}
              </div>
            </section>

            <!-- resistances / vulnerabilities / immunities -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.damage_categories')}</h4>
              <div class="space-y-4">
                {#each DAMAGE_CATEGORY_KEYS as cat (cat)}
                  {@const active = (c.sheet?.[cat] ?? []) as string[]}
                  <div>
                    <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1.5"
                      style="color:{cat === 'vulnerabilities' ? '#a93535' : cat === 'immunities' ? '#4a7a4a' : '#8b6914'};">
                      {$_(`character.${cat}`)}
                    </div>
                    <div class="flex flex-wrap gap-1.5">
                      {#each DAMAGE_TYPES as dt (dt)}
                        {@const on = active.includes(dt)}
                        <button type="button"
                          onclick={() => canEdit(c) && patchSheet(c, (s) => {
                            const cur = (s[cat] ?? []) as string[];
                            const next = on ? cur.filter((x) => x !== dt) : [...cur, dt];
                            return { ...s, [cat]: next.length ? next : undefined };
                          })}
                          class="rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest transition-colors"
                          style={on
                            ? cat === 'vulnerabilities'
                              ? 'background:rgba(139,26,26,0.3);color:#a93535;border:1px solid #8b1a1a;'
                              : cat === 'immunities'
                                ? 'background:rgba(74,122,74,0.25);color:#4a7a4a;border:1px solid #3a6a3a;'
                                : 'background:rgba(201,168,76,0.25);color:#6d510f;border:1px solid #8b6914;'
                            : 'background:rgba(139,105,20,0.06);color:#6d510f;border:1px solid rgba(139,105,20,0.25);'}>
                          {dt}
                        </button>
                      {/each}
                    </div>
                  </div>
                {/each}
              </div>
            </section>

          </div>
        </div>
        {/if}

        {#if tab === 'combat'}
        {@const dexMod = abilityModForChar(c, 'dex')}
        {@const initBonus = c.sheet?.initiative ?? dexMod}
        <div class="space-y-8">
          <div class="space-y-8">
            <!-- abilities -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.abilities')}</h4>
              <div class="grid grid-cols-3 sm:grid-cols-6 gap-2 text-center">
                {#each ABILITIES as k (k)}
                  {@const val = abilityScore(c, k)}
                  {@const mod = Math.floor((val - 10) / 2)}
                  {@const isOv = hasAbilityOverride(c, k)}
                  <div class="rounded-md p-2" style="background:rgba(139,105,20,0.08); border:1px solid {isOv ? '#c9a84c' : 'rgba(139,105,20,0.3)'};">
                    <div class="text-[10px] font-display tracking-widest" style="color:#8b6914;">{$_(`character.ability_${k}`)}</div>
                    <input type="number" min="1" max="30" value={val}
                      onchange={(e) => {
                        const v = +(e.currentTarget as HTMLInputElement).value;
                        patchSheet(c, (s) => ({ ...s, abilities_override: { ...(s.abilities_override ?? {}), [k]: v } }));
                      }}
                      class="w-full text-center text-lg font-bold bg-transparent border-0 p-0" />
                    <button type="button"
                      onclick={() => rollCheck(cid, `1d20${mod >= 0 ? '+' : ''}${mod}`, $_('character.ability_check').replace('{{ability}}', $_(`character.ability_${k}`)), c.id)}
                      class="text-xs w-full tabular-nums font-semibold hover:underline"
                      style="color:#8b6355;" title={$_('character.ability_check').replace('{{ability}}', $_(`character.ability_${k}`))}>
                      {mod >= 0 ? '+' : ''}{mod}
                    </button>
                    {#if isOv && canEdit(c)}
                      <button type="button" class="text-[9px] underline w-full text-center" style="color:#8b6914;"
                        onclick={() => patchSheet(c, (s) => {
                          const { [k]: _removed, ...rest } = s.abilities_override ?? {};
                          return { ...s, abilities_override: rest };
                        })}>{$_('character.override_reset')}</button>
                    {/if}
                  </div>
                {/each}
              </div>
            </section>

            <!-- defense / initiative -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.defense')}</h4>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
                <div>
                  <Stepper label={$_('character.ac')} value={c.sheet?.ac ?? 10} min={0} max={40}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, ac: v }))} />
                  <div class="text-[10px] mt-1" style="color:#8b6355;">
                    Computed: <b style="color:#2c1810;">{computedAC(c)}</b>
                    <button type="button" class="underline ml-1" style="color:#8b6914;"
                      onclick={() => patchSheet(c, (s) => ({ ...s, ac: computedAC(c) }))}>
                      apply
                    </button>
                  </div>
                </div>
                <div>
                  <Stepper label={$_('character.initiative_bonus')} value={initBonus} min={-10} max={20}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, initiative: v }))} />
                  {#if c.sheet?.initiative === undefined}
                    <div class="text-[10px] mt-1" style="color:#8b6355;">{$_('character.initiative_from_dex').replace('{{mod}}', (dexMod >= 0 ? '+' : '') + String(dexMod))}</div>
                  {:else}
                    <button type="button" class="text-[10px] underline mt-1" style="color:#8b6914;"
                      onclick={() => patchSheet(c, (s) => { const { initiative: _i, ...rest } = s; return rest; })}>
                      {$_('character.initiative_reset_dex')}
                    </button>
                  {/if}
                </div>
                <div>
                  <Stepper label={$_('character.speed')} value={c.sheet?.speed ?? 30} min={0} max={120} step={5}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, speed: v }))} />
                  {#if computedSpeed(c) !== (c.sheet?.speed ?? 30)}
                    <div class="text-[10px] mt-1" style="color:#8b6355;">
                      Computed: <b style="color:#2c1810;">{computedSpeed(c)}</b>
                      <button type="button" class="underline ml-1" style="color:#8b6914;"
                        onclick={() => patchSheet(c, (s) => ({ ...s, speed: computedSpeed(c) }))}>
                        apply
                      </button>
                    </div>
                  {/if}
                </div>
                <Stepper label={$_('character.crit_range')} value={(c.sheet as Record<string,unknown>)?.crit_range as number ?? 20} min={18} max={20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), crit_range: v } as Sheet))} />
                <Stepper label={$_('character.swim_speed')} value={(c.sheet as Record<string,unknown>)?.swim_speed as number ?? 0} min={0} max={120} step={5}
                  onchange={(v) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), swim_speed: v > 0 ? v : undefined } as Sheet))} />
                <Stepper label={$_('character.climb_speed')} value={(c.sheet as Record<string,unknown>)?.climb_speed as number ?? 0} min={0} max={120} step={5}
                  onchange={(v) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), climb_speed: v > 0 ? v : undefined } as Sheet))} />
                <Stepper label={$_('character.fly_speed')} value={(c.sheet as Record<string,unknown>)?.fly_speed as number ?? 0} min={0} max={120} step={5}
                  onchange={(v) => patchSheet(c, (s) => ({ ...(s as Record<string,unknown>), fly_speed: v > 0 ? v : undefined } as Sheet))} />
              </div>
              {#if (c.sheet as Record<string,unknown>)?.nonmagical_damage_reduction as number}
                {@const dr = (c.sheet as Record<string,unknown>).nonmagical_damage_reduction as number}
                <div class="mt-2 text-[10px]" style="color:#4f6d36;">
                  Non-magical B/P/S damage reduction: <b>{dr}</b>
                </div>
              {/if}
              {#if canEdit(c)}
                <div class="mt-3 grid grid-cols-2 sm:grid-cols-4 gap-2 text-xs">
                  <select value={c.sheet?.armor?.type ?? ''}
                    onchange={(e) => {
                      const type = (e.currentTarget as HTMLSelectElement).value as ArmorType | '';
                      if (!type) {
                        patchSheet(c, (s) => {
                          const { armor: _a, ...rest } = s;
                          return { ...rest, ac: computeAC(rest) };
                        });
                        return;
                      }
                      const defaults: Record<string, { ac_base: number; max_dex: number }> = {
                        light: { ac_base: 11, max_dex: 99 },
                        medium: { ac_base: 13, max_dex: 2 },
                        heavy: { ac_base: 16, max_dex: 0 },
                        unarmored_barbarian: { ac_base: 10, max_dex: 99 },
                        unarmored_monk: { ac_base: 10, max_dex: 99 },
                        mage_armor: { ac_base: 13, max_dex: 99 },
                        draconic: { ac_base: 13, max_dex: 99 },
                        natural: { ac_base: 13, max_dex: 99 },
                      };
                      const d = defaults[type] ?? { ac_base: 10, max_dex: 99 };
                      patchSheet(c, (s) => {
                        const newArmor = { type, ac_base: d.ac_base, max_dex: d.max_dex };
                        const ac = computeAC({ ...s, armor: newArmor });
                        return { ...s, armor: newArmor, ac };
                      });
                    }}
                    class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1">
                    <option value="">No armor</option>
                    <option value="light">Light armor</option>
                    <option value="medium">Medium armor</option>
                    <option value="heavy">Heavy armor</option>
                    <option value="unarmored_barbarian">Unarmored (Barb)</option>
                    <option value="unarmored_monk">Unarmored (Monk)</option>
                    <option value="mage_armor">Mage Armor</option>
                    <option value="draconic">Draconic Resilience</option>
                    <option value="natural">Natural armor</option>
                  </select>
                  {#if c.sheet?.armor?.type && c.sheet.armor.type !== 'unarmored_barbarian' && c.sheet.armor.type !== 'unarmored_monk' && c.sheet.armor.type !== 'mage_armor' && c.sheet.armor.type !== 'draconic'}
                    <input type="number" min="0" max="30" placeholder="AC base"
                      value={c.sheet.armor.ac_base ?? 10}
                      onchange={(e) => patchSheet(c, (s) => {
                        const ac_base = +(e.currentTarget as HTMLInputElement).value;
                        const armor = { ...s.armor, ac_base };
                        return { ...s, armor, ac: computeAC({ ...s, armor }) };
                      })}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-center" />
                    <input type="number" min="0" max="10" placeholder="Max DEX"
                      value={c.sheet.armor.max_dex ?? 99}
                      onchange={(e) => patchSheet(c, (s) => {
                        const max_dex = +(e.currentTarget as HTMLInputElement).value;
                        const armor = { ...s.armor, max_dex };
                        return { ...s, armor, ac: computeAC({ ...s, armor }) };
                      })}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-center" />
                  {/if}
                  <label class="flex items-center gap-2">
                    <input type="checkbox" checked={c.sheet?.shield ?? false}
                      onchange={(e) => patchSheet(c, (s) => {
                        const shield = (e.currentTarget as HTMLInputElement).checked;
                        return { ...s, shield, ac: computeAC({ ...s, shield }) };
                      })} />
                    <span>Shield (+2)</span>
                  </label>
                  {#if c.sheet?.armor?.type}
                    <label class="flex items-center gap-2">
                      <input type="checkbox" checked={c.sheet?.armor?.stealth_disadvantage ?? false}
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, armor: { ...s.armor, stealth_disadvantage: (e.currentTarget as HTMLInputElement).checked } }))} />
                      <span>Stealth disadv.</span>
                    </label>
                  {/if}
                </div>
              {/if}
            </section>

            <!-- fighting styles -->
            {#if canEdit(c) || (c.sheet?.fighting_styles ?? []).length > 0}
            {@const ALL_STYLES = ['archery','dueling','great_weapon_fighting','two-weapon_fighting','defense','blind_fighting','interception','thrown_weapon_fighting','unarmed_fighting']}
            {@const active = c.sheet?.fighting_styles ?? []}
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.fighting_styles')}</h4>
              <div class="flex flex-wrap gap-1.5">
                {#each ALL_STYLES as style (style)}
                  {@const on = active.some(s => s.toLowerCase() === style)}
                  <button type="button" onclick={() => {
                      const next = on
                        ? active.filter(s => s.toLowerCase() !== style)
                        : [...active, style];
                      patchSheet(c, (s) => ({ ...s, fighting_styles: next.length ? next : undefined }));
                    }}
                    class="rounded-full px-2.5 py-0.5 text-xs font-semibold"
                    style={on
                      ? 'background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #f4e4c1;color:#1a0f08;'
                      : 'background:rgba(44,24,16,0.5);border:1px solid #c9a84c;color:#f4e4c1;'}>
                    {$_(`character.style_${style}`)}
                  </button>
                {/each}
              </div>
            </section>
            {/if}

            <!-- saving throws + skills -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex flex-wrap items-center gap-2">
                <span>{$_('character.saving_throws')}</span>
                {#if auraOfProtectionBonus(c) !== null}
                  <span class="text-[10px] rounded px-1.5 py-0.5 font-semibold" style="background:rgba(201,168,76,0.2);color:#6d510f;border:1px solid rgba(139,105,20,0.4);">
                    {$_('character.aura_of_protection').replace('{{bonus}}', String(auraOfProtectionBonus(c)! >= 0 ? '+' + auraOfProtectionBonus(c) : auraOfProtectionBonus(c)))}
                  </span>
                {/if}
              </h4>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                {#each ABILITIES as a (a)}
                  {@const sm = saveMod(c, a)}
                  {@const isOv = hasSaveOverride(c, a)}
                  <div class="rounded px-2 py-1 flex flex-col gap-0.5"
                    style={`background:${c.sheet?.saves?.[a] ? 'rgba(201,168,76,0.25)' : 'rgba(139,105,20,0.06)'}; border:1px solid ${isOv ? '#c9a84c' : 'rgba(139,105,20,0.35)'};`}>
                    <div class="flex items-center justify-between gap-2">
                      <button type="button" onclick={() => toggleSave(c, a)}
                        class="inline-flex items-center gap-1.5 text-xs">
                        <span class="h-3 w-3 rounded-full border flex-none" style={`border-color:#8b6914; background:${c.sheet?.saves?.[a] ? '#8b6914' : 'transparent'};`}></span>
                        <span class="uppercase tracking-widest font-display" style="color:#8b6914;">{$_(`character.ability_${a}`)}</span>
                      </button>
                      {#if isOv && canEdit(c)}
                        <input type="number" min="-20" max="30"
                          value={c.sheet?.saves_override?.[a]}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, saves_override: { ...(s.saves_override ?? {}), [a]: +(e.currentTarget as HTMLInputElement).value } }))}
                          class="w-12 text-center tabular-nums font-bold bg-transparent border-0 p-0 text-sm"
                          style="color:#2c1810;" />
                      {:else}
                        <button type="button"
                          onclick={() => rollCheck(cid, `1d20${sm >= 0 ? '+' : ''}${sm}`, $_('character.save_check').replace('{{ability}}', $_(`character.ability_${a}`)), c.id)}
                          class="tabular-nums font-bold text-sm hover:underline"
                          style="color:#2c1810;" title={$_('character.save_check').replace('{{ability}}', $_(`character.ability_${a}`))}>
                          {sm >= 0 ? '+' : ''}{sm}
                        </button>
                      {/if}
                    </div>
                    {#if canEdit(c)}
                      {#if isOv}
                        <button type="button" class="text-[9px] underline text-left" style="color:#8b6914;"
                          onclick={() => patchSheet(c, (s) => {
                            const { [a]: _r, ...rest } = s.saves_override ?? {};
                            return { ...s, saves_override: rest };
                          })}>{$_('character.override_reset')}</button>
                      {:else}
                        <button type="button" class="text-[9px] underline text-left" style="color:#8b6355;"
                          onclick={() => patchSheet(c, (s) => ({ ...s, saves_override: { ...(s.saves_override ?? {}), [a]: sm } }))}>
                          {$_('character.override_set')}
                        </button>
                      {/if}
                    {/if}
                  </div>
                {/each}
              </div>
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h inline-flex flex-wrap items-center gap-2">
                <span>{$_('character.skills')} <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.skills_hint')}</span></span>
                {#if hasReliableTalent(c)}<span class="text-[10px] rounded px-1.5 py-0.5 font-semibold" style="background:rgba(201,168,76,0.2);color:#6d510f;border:1px solid rgba(139,105,20,0.4);">{$_('character.reliable_talent')}</span>{/if}
                {#if hasJackOfAllTrades(c)}<span class="text-[10px] rounded px-1.5 py-0.5 font-semibold" style="background:rgba(201,168,76,0.2);color:#6d510f;border:1px solid rgba(139,105,20,0.4);">{$_('character.jack_of_all_trades')}</span>{/if}
                {#if hasEvasion(c)}<span class="text-[10px] rounded px-1.5 py-0.5 font-semibold" style="background:rgba(201,168,76,0.2);color:#6d510f;border:1px solid rgba(139,105,20,0.4);">{$_('character.evasion')}</span>{/if}
              </h4>
              <div class="grid sm:grid-cols-2 gap-1">
                {#each SKILLS as sk (sk.key)}
                  {@const lvl = c.sheet?.skills?.[sk.key]}
                  {@const mod = skillMod(c, sk)}
                  <div class="flex items-center gap-0 rounded overflow-hidden text-sm"
                    style={`background:${lvl ? 'rgba(201,168,76,0.18)' : 'rgba(139,105,20,0.05)'}; border:1px solid rgba(139,105,20,0.25);`}>
                    <button type="button" onclick={() => cycleSkill(c, sk.key)}
                      class="flex-1 flex items-center gap-2 px-2 py-1 text-left">
                      <span class="h-3 w-3 rounded-full border flex items-center justify-center text-[8px] font-bold shrink-0"
                        style={`border-color:#8b6914; background:${lvl === 'expert' ? '#8b6914' : lvl === 'prof' ? '#c9a84c' : 'transparent'}; color:#1a0f08;`}>
                        {lvl === 'expert' ? '★' : ''}
                      </span>
                      <span>{$_(`character.skill_${sk.key}`)}</span>
                      <span class="text-[10px] uppercase" style="color:#8b6914;">{$_(`character.ability_${sk.ability}`)}</span>
                    </button>
                    <button type="button"
                      onclick={() => rollCheck(cid, `1d20${mod >= 0 ? '+' : ''}${mod}`, $_('character.skill_check').replace('{{skill}}', $_(`character.skill_${sk.key}`)), c.id)}
                      class="tabular-nums font-bold px-2 py-1 hover:underline shrink-0"
                      style="color:#2c1810; border-left:1px solid rgba(139,105,20,0.25);"
                      title={$_('character.skill_check').replace('{{skill}}', $_(`character.skill_${sk.key}`))}>
                      {mod >= 0 ? '+' : ''}{mod}
                    </button>
                  </div>
                {/each}
              </div>

              <!-- passive scores -->
              {#if c.sheet}
                <div class="mt-3 grid grid-cols-3 sm:grid-cols-6 gap-2">
                  {#each [
                    { key: 'perception',   ability: 'wis' as Ability, bonusKey: 'passive_perception_bonus' },
                    { key: 'insight',      ability: 'wis' as Ability, bonusKey: null },
                    { key: 'investigation',ability: 'int' as Ability, bonusKey: 'passive_investigation_bonus' },
                  ] as ps}
                    {@const sk = SKILLS.find((s) => s.key === ps.key)!}
                    {@const bonus = ps.bonusKey ? ((c.sheet.senses as Record<string,number> | undefined)?.[ps.bonusKey] ?? 0) : 0}
                    {@const passive = 10 + skillMod(c, sk) + bonus}
                    <div class="rounded px-2 py-1 text-center text-xs" style="background:rgba(139,105,20,0.08); border:1px solid rgba(139,105,20,0.25);">
                      <div class="text-[9px] uppercase tracking-widest" style="color:#8b6914;">Passive {ps.key.replace('_',' ')}</div>
                      <div class="font-bold text-sm" style="color:#2c1810;">{passive}</div>
                    </div>
                  {/each}
                </div>
              {/if}
            </section>

            <!-- weapons -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Swords size={14} /> {$_('character.weapons')}</h4>

              {#if (c.sheet?.weapons ?? []).length}
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">
                      <tr>
                        <th class="text-left py-1">{$_('character.weapon_equip')}</th>
                        <th class="text-left py-1">{$_('character.name')}</th>
                        <th class="py-1">{$_('character.weapon_atk')}</th>
                        <th class="text-left py-1">{$_('character.weapon_damage')}</th>
                        <th class="text-left py-1 text-[10px]">Die</th>
                        <th class="text-left py-1 text-[10px]">Vers.</th>
                        <th class="text-left py-1">{$_('character.weapon_range')}</th>
                        <th class="text-left py-1">{$_('character.weapon_properties')}</th>
                        <th></th>
                      </tr>
                    </thead>
                    <tbody>
                      {#each c.sheet?.weapons ?? [] as w (w.id)}
                        <tr class="border-t align-top" style="border-color:rgba(139,105,20,0.2);">
                          <td class="py-1 pr-2">
                            <button type="button" onclick={() => patchWeapon(c, w.id, { equipped: !w.equipped })}
                              class="rounded px-1.5 py-0.5 text-[10px] font-bold {w.equipped ? 'bg-amber-500 text-black' : 'bg-neutral-700'}"
                              style={w.equipped ? '' : 'color:#f4e4c1;'}>
                              {w.equipped ? $_('character.weapon_equip_yes') : $_('character.weapon_equip_no')}
                            </button>
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.name}
                              onchange={(e) => patchWeapon(c, w.id, { name: (e.currentTarget as HTMLInputElement).value })}
                              class="w-full bg-transparent border-0 px-1 py-0.5" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="number" value={w.attack_bonus ?? 0}
                              onchange={(e) => patchWeapon(c, w.id, { attack_bonus: +(e.currentTarget as HTMLInputElement).value })}
                              class="w-14 bg-transparent border-0 px-1 py-0.5 text-center tabular-nums" />
                            {#if computedWeaponAttackBonus(c, w) !== (w.attack_bonus ?? 0)}
                              <button type="button" class="block text-[9px] underline" style="color:#8b6914;"
                                title={$_('character.sync_computed')}
                                onclick={() => patchWeapon(c, w.id, { attack_bonus: computedWeaponAttackBonus(c, w) })}>
                                ↑{computedWeaponAttackBonus(c, w) >= 0 ? '+' : ''}{computedWeaponAttackBonus(c, w)}
                              </button>
                            {/if}
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.damage ?? ''} placeholder={$_('character.weapon_damage_inline_ph')}
                              onchange={(e) => patchWeapon(c, w.id, { damage: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-16 bg-transparent border-0 px-1 py-0.5" />
                            <input type="text" value={w.damage_type ?? ''} placeholder={$_('character.weapon_damage_type_ph')}
                              onchange={(e) => patchWeapon(c, w.id, { damage_type: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-16 bg-transparent border-0 px-1 py-0.5 text-xs italic" style="color:#8b6914;" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.damage_die ?? ''} placeholder="1d8"
                              onchange={(e) => patchWeapon(c, w.id, { damage_die: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-12 bg-transparent border-0 px-1 py-0.5 text-xs" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.versatile_die ?? ''} placeholder="1d10"
                              onchange={(e) => patchWeapon(c, w.id, { versatile_die: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-12 bg-transparent border-0 px-1 py-0.5 text-xs" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.range ?? ''} placeholder={$_('character.weapon_range')}
                              onchange={(e) => patchWeapon(c, w.id, { range: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-24 bg-transparent border-0 px-1 py-0.5" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.properties ?? ''} placeholder={$_('character.weapon_properties')}
                              onchange={(e) => patchWeapon(c, w.id, { properties: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-full bg-transparent border-0 px-1 py-0.5 text-xs" />
                          </td>
                          <td class="py-1">
                            <button type="button" aria-label={$_('common.remove')} onclick={() => removeWeapon(c, w.id)}
                              class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                          </td>
                        </tr>
                        <tr style="border:0;">
                          <td></td>
                          <td colspan="6" class="pb-2 pr-2">
                            <textarea value={w.description ?? ''} placeholder={$_('character.weapon_description')}
                              onchange={(e) => patchWeapon(c, w.id, { description: (e.currentTarget as HTMLTextAreaElement).value || undefined })}
                              rows="3"
                              class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-xs italic resize-y"
                              style="color:#c9a48c; min-height:4rem;"></textarea>
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {/if}

              <!-- unarmed strike always shown -->
              {#if true}
                {@const unarmedAtk = abilityModForChar(c, 'str') + profBonus(c.level_total)}
                {@const unarmedDmg = martialArtsDie(c) ?? (charHasFeat(c, 'tavern_brawler') ? `1d4+${Math.max(0, abilityModForChar(c, 'str'))}` : `1+${Math.max(0, abilityModForChar(c, 'str'))}`)}
                <div class="mt-2 text-xs" style="color:#8b6914;">
                  {$_('character.unarmed_strike')}: <b style="color:#2c1810;">{unarmedAtk >= 0 ? '+' : ''}{unarmedAtk}</b>
                  · <b style="color:#2c1810;">{unarmedDmg}</b> bludgeoning
                </div>
              {/if}

              <form onsubmit={(e) => { e.preventDefault(); addWeapon(c); }}
                class="mt-3 grid grid-cols-2 md:grid-cols-6 gap-2 items-end">
                <input required placeholder={$_('character.weapon_name_ph')} bind:value={newWpName}
                  class="col-span-2 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <div>
                  <Stepper compact label={$_('character.weapon_atk')} value={newWpAtk} min={-5} max={20}
                    onchange={(v) => newWpAtk = v} />
                </div>
                <input placeholder={$_('character.weapon_damage_ph')} bind:value={newWpDmg}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder={$_('character.weapon_damage_type_ph')} bind:value={newWpDmgType}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder={$_('character.weapon_range')} bind:value={newWpRange}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder={$_('character.weapon_properties_ph')} bind:value={newWpProps}
                  class="col-span-2 md:col-span-4 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <textarea placeholder={$_('character.weapon_description')} bind:value={newWpDesc} rows="4"
                  class="col-span-2 md:col-span-6 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm resize-y" style="min-height:5rem;"></textarea>
                <button class="col-span-2 md:col-span-6 rounded bg-violet-600 px-3 py-1 text-sm text-white inline-flex items-center justify-center gap-1">
                  <Plus size={14} /> {$_('character.weapon_add')}
                </button>
              </form>
            </section>
          </div>
        </div>
        {/if}

        {#if tab === 'magic'}
        <div class="space-y-8">
          <div class="space-y-8">
            <!-- concentration -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Brain size={14} /> {$_('character.concentration')}</h4>
              {#if c.sheet?.concentration?.spell}
                <div class="flex items-center justify-between gap-2">
                  <span class="text-sm"><b>{c.sheet.concentration.spell}</b>
                    {#if c.sheet.concentration.since}
                      <span class="text-xs italic ml-2" style="color:#8b6355;">{$_('character.concentration_since').replace('{{time}}', new Date(c.sheet.concentration.since).toLocaleTimeString())}</span>
                    {/if}
                  </span>
                  <button class="rounded px-3 py-1 text-xs" style="background:#8b1a1a;color:#f4e4c1;"
                    onclick={() => patchSheet(c, (s) => ({ ...s, concentration: null }))}>{$_('character.drop')}</button>
                </div>
              {:else}
                <form onsubmit={(e) => {
                    e.preventDefault();
                    const v = (e.currentTarget.elements.namedItem('sp') as HTMLInputElement)?.value?.trim();
                    if (v) patchSheet(c, (s) => ({ ...s, concentration: { spell: v, since: new Date().toISOString() } }));
                  }}
                  class="flex items-center gap-2">
                  <input name="sp" placeholder={$_('character.book_spell_ph')}
                    class="flex-1 text-sm" />
                  <button class="rounded bg-violet-600 px-3 py-1 text-xs text-white">{$_('character.set')}</button>
                </form>
              {/if}
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Sparkles size={14} /> {$_('character.spell_slots')}</h4>
              <div class="space-y-1.5">
                {#each ['1','2','3','4','5','6','7','8','9'] as lvl (lvl)}
                  {@const s = slot(c, lvl)}
                  {#if s.max > 0}
                    <SlotTrack level={Number(lvl)} current={s.current} max={s.max}
                      onchange={(cur, mx) => patchSheet(c, (sh) => ({ ...sh, slots: { ...(sh.slots ?? {}), [lvl]: { current: cur, max: mx } } }))} />
                  {/if}
                {/each}
              </div>
              <!-- add inactive slot levels -->
              <div class="mt-2 flex flex-wrap gap-1">
                {#each ['1','2','3','4','5','6','7','8','9'].filter(lvl => slot(c, lvl).max === 0) as lvl (lvl)}
                  <button type="button"
                    onclick={() => patchSheet(c, (sh) => ({ ...sh, slots: { ...(sh.slots ?? {}), [lvl]: { current: 1, max: 1 } } }))}
                    class="rounded-full px-2 py-0.5 text-[10px] font-bold"
                    style="background:rgba(44,24,16,0.6); border:1px solid #c9a84c; color:#f4e4c1;">
                    + {$_('spells.level')} {lvl}
                  </button>
                {/each}
              </div>
            </section>

            <!-- spellcasting stats -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Zap size={14} /> {$_('character.spellcasting')}</h4>
              <div class="grid grid-cols-1 sm:grid-cols-3 gap-4">
                <label class="flex flex-col">
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{$_('character.casting_ability')}</span>
                  <div class="flex items-center gap-1">
                    <select value={c.sheet?.casting?.ability ?? ''}
                      onchange={(e) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), ability: (e.currentTarget as HTMLSelectElement).value || undefined } }))}
                      class="mt-0.5 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm flex-1">
                      <option value="">—</option>
                      {#each ['INT','WIS','CHA','STR','DEX','CON'] as a (a)}<option>{a}</option>{/each}
                    </select>
                    {#if canEdit(c)}
                      {@const detected = detectSpellcastingAbility(c)}
                      {#if detected && (c.sheet?.casting?.ability ?? '').toLowerCase() !== detected}
                        <button type="button" class="text-[10px] underline whitespace-nowrap" style="color:#8b6914;"
                          onclick={() => patchSheet(c, (s) => {
                            const ab = detected!;
                            const mod = abilityMod(s.abilities?.[ab]);
                            const pb = profBonus(c.level_total);
                            return { ...s, casting: { ability: ab.toUpperCase(), spell_attack: mod + pb, save_dc: 8 + mod + pb } };
                          })}>
                          ↑ {detected.toUpperCase()} (atk {abilityModForChar(c, detected) + profBonus(c.level_total)}, DC {8 + abilityModForChar(c, detected) + profBonus(c.level_total)})
                        </button>
                      {/if}
                    {/if}
                  </div>
                </label>
                <div class="flex flex-col gap-1">
                  <Stepper label={$_('character.spell_attack_bonus')} value={c.sheet?.casting?.spell_attack ?? 0} min={-5} max={20}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), spell_attack: v } }))} />
                  {#if computedSpellAttack(c) !== null && computedSpellAttack(c) !== (c.sheet?.casting?.spell_attack ?? 0)}
                    <button type="button" class="text-[10px] underline text-left" style="color:#8b6914;"
                      onclick={() => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), spell_attack: computedSpellAttack(c)! } }))}>
                      ↑ {$_('character.sync_computed')} (+{computedSpellAttack(c)})
                    </button>
                  {/if}
                </div>
                <div class="flex flex-col gap-1">
                  <Stepper label={$_('character.spell_save_dc')} value={c.sheet?.casting?.save_dc ?? 8} min={0} max={30}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), save_dc: v } }))} />
                  {#if computedSpellSaveDC(c) !== null && computedSpellSaveDC(c) !== (c.sheet?.casting?.save_dc ?? 8)}
                    <button type="button" class="text-[10px] underline text-left" style="color:#8b6914;"
                      onclick={() => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), save_dc: computedSpellSaveDC(c)! } }))}>
                      ↑ {$_('character.sync_computed')} ({computedSpellSaveDC(c)})
                    </button>
                  {/if}
                </div>
              </div>
            </section>

            <!-- enchantments -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5 flex-wrap">
                <span class="inline-flex items-center gap-1.5"><BookOpen size={14} /> {$_('character.enchantments')}</span>
                {#if spellPrepCount(c) !== null}
                  {@const preparedCount = (c.sheet?.spells ?? []).filter((s) => s.level > 0 && s.prepared).length}
                  <span class="text-[10px] font-normal" style="color:{preparedCount > spellPrepCount(c)! ? '#8b1a1a' : '#8b6914'};">
                    {$_('character.prepared_count').replace('{{n}}', String(preparedCount)).replace('{{max}}', String(spellPrepCount(c)))}
                  </span>
                {/if}
              </h4>

              {#each grouped(c) as [lv, ss] (lv)}
                {@const sl = lv > 0 ? slot(c, String(lv)) : { current: 0, max: 0 }}
                <div class="mb-3">
                  <div class="flex items-center justify-between text-[11px] uppercase tracking-widest font-display" style="color:#8b6914;">
                    <span>{lv === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${lv}`}</span>
                    {#if lv > 0}<span class="opacity-70">{sl.current}/{sl.max}</span>{/if}
                  </div>
                  <ul class="mt-1 space-y-1">
                    {#each ss as s (spellKey(s))}
                      <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1">
                        <button type="button" title={$_('character.toggle_prepared')} onclick={() => togglePrepared(c, s)}
                          class="rounded px-1.5 py-0.5 text-[10px] font-bold {s.prepared ? 'bg-amber-500 text-black' : 'bg-neutral-700'}"
                          style={s.prepared ? '' : 'color:#f4e4c1;'}>
                          {s.prepared ? $_('character.prepared') : $_('character.known')}
                        </button>
                        <button type="button" class="flex-1 text-left text-sm truncate"
                          onclick={() => selectedSpell = s}
                          style="color:#2c1810;" title={$_('character.view_details')}>
                          {s.name}
                          {#if s.custom}<span class="text-[10px] italic" style="color:#8b6914;"> · {$_('spells.custom_tag')}</span>{/if}
                          {#if s.ritual}<span class="text-[10px]" style="color:#8b6914;"> · {$_('spells.ritual')}</span>{/if}
                          {#if s.concentration}<span class="text-[10px]" style="color:#8b6914;"> · {$_('spells.conc_short')}</span>{/if}
                          {#if s.source}
                            <span class="text-[10px] rounded px-1 ml-1"
                              style="background:rgba(139,105,20,0.15); color:#6d510f; border:1px solid rgba(139,105,20,0.35);">
                              {s.source}
                            </span>
                          {/if}
                        </button>
                        {#if lv > 0}
                          <button type="button" onclick={() => castSpell(c, s)}
                            disabled={sl.current <= 0}
                            class="rounded bg-violet-600 px-2 py-0.5 text-[11px] text-white disabled:opacity-40">{$_('spells.cast')}</button>
                        {/if}
                        <button type="button" aria-label={$_('common.remove')} onclick={() => removeSpell(c, s)}
                          class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/each}

              {#if !(c.sheet?.spells ?? []).length}
                <p class="text-sm italic" style="color:#8b6355;">{$_('character.enchantments_empty')}</p>
              {/if}

              <!-- add from book -->
              <details class="mt-4"
                ontoggle={(e) => {
                  if ((e.currentTarget as HTMLDetailsElement).open) {
                    // Pre-populate class filter from character's primary caster class
                    const primaryClass = (c.sheet?.classes ?? [])
                      .map(cl => cl.name?.trim())
                      .find(n => n && CASTER_CLASSES.map(x => x.toLowerCase()).includes(n.toLowerCase()));
                    if (primaryClass && !bookClass) bookClass = primaryClass;
                    runBookSearch();
                  }
                }}>
                <summary class="cursor-pointer inline-flex items-center gap-1.5 text-sm font-display" style="color:#c9a84c;">
                  <Search size={14} /> {$_('common.add')}
                </summary>
                <div class="mt-2 space-y-2">
                  <div class="flex gap-2 flex-wrap">
                    <input type="search" placeholder={$_('character.book_search_ph')}
                      bind:value={bookQuery} oninput={onBookInput}
                      class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                    <select bind:value={bookLevel}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      <option value="">{$_('spells.any_level')}</option>
                      {#each [0,1,2,3,4,5,6,7,8,9] as l (l)}
                        <option value={l}>{l === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${l}`}</option>
                      {/each}
                    </select>
                    <select bind:value={bookClass}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      <option value="">{$_('spells.any_class')}</option>
                      {#each CASTER_CLASSES as cl (cl)}
                        <option value={cl}>{cl}</option>
                      {/each}
                    </select>
                  </div>
                  {#if bookLoading}<p class="text-xs italic" style="color:#8b6355;">{$_('spells.loading')}</p>{/if}
                  {#if bookResults.length}
                    <ul class="max-h-56 overflow-y-auto space-y-1 border rounded"
                      style="border-color:#d4b896;">
                      {#each bookResults as r (r.slug)}
                        {@const already = hasSpell(c, { slug: r.slug, name: r.name, level: r.level })}
                        {@const allowed = canLearn(c, { level: r.level, classes: r.classes })}
                        {@const tooHigh = !allowed}
                        {@const isOpen = bookExpanded === r.slug}
                        <li class="text-sm border-b" style="border-color:rgba(139,105,20,0.15);">
                          <div class="flex items-center gap-2 px-2 py-1 {tooHigh ? 'opacity-50' : ''}">
                            <span class="rounded bg-neutral-800 px-1.5 text-[10px] font-bold" style="color:#f4e4c1;">
                              {r.level === 0 ? 'C' : r.level}
                            </span>
                            <button type="button" class="flex-1 text-left truncate inline-flex items-center gap-1"
                              onclick={() => bookExpanded = isOpen ? null : r.slug}
                              style="color:#2c1810;">
                              <ChevronRight size={12} class="transition-transform {isOpen ? 'rotate-90' : ''}"
                                style="color:#8b6914;" />
                              {r.name}
                              <span class="text-[10px]" style="color:#8b6914;">· {r.school}</span>
                              {#if r.source}
                                <span class="text-[10px] rounded px-1.5 ml-1"
                                  style="background:rgba(139,105,20,0.18); color:#6d510f; border:1px solid rgba(139,105,20,0.4);">
                                  {r.source}
                                </span>
                              {/if}
                            </button>
                            <button type="button" disabled={already || tooHigh}
                              title={tooHigh ? $_('character.spell_too_high') : ''}
                              onclick={() => addSpell(c, { slug: r.slug, name: r.name, level: r.level, school: r.school, classes: r.classes, ritual: r.ritual, concentration: r.concentration, description: r.description, casting_time: r.casting_time, range_text: r.range_text, components: r.components, duration: r.duration, higher_levels: r.higher_levels, source: r.source })}
                              class="rounded bg-violet-600 px-2 py-0.5 text-[11px] text-white disabled:opacity-40">
                              {already ? '✓' : tooHigh ? '🔒' : $_('spells.learn')}
                            </button>
                          </div>
                          {#if isOpen}
                            <div class="px-3 py-2 text-xs space-y-1.5" style="background:rgba(139,105,20,0.06); color:#3a2313;">
                              <div class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">
                                {r.level === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${r.level}`} · {r.school}
                                {#if r.classes?.length} · {r.classes.join(', ')}{/if}
                                {#if r.ritual} · ritual{/if}
                                {#if r.concentration} · concentration{/if}
                              </div>
                              {#if r.source}
                                <div class="text-[10px]" style="color:#6d510f;">
                                  <b style="color:#8b6914;">{$_('spells.source')}:</b> {r.source}
                                </div>
                              {/if}
                              <div class="grid grid-cols-2 sm:grid-cols-4 gap-x-3 gap-y-0.5">
                                {#if r.casting_time}
                                  <div><b style="color:#8b6914;">{$_('spells.cast')}:</b> {r.casting_time}</div>
                                {/if}
                                {#if r.range_text}
                                  <div><b style="color:#8b6914;">{$_('spells.range')}:</b> {r.range_text}</div>
                                {/if}
                                {#if r.components}
                                  <div class="col-span-2"><b style="color:#8b6914;">{$_('spells.components')}:</b> {r.components}</div>
                                {/if}
                                {#if r.duration}
                                  <div><b style="color:#8b6914;">{$_('spells.duration')}:</b> {r.duration}</div>
                                {/if}
                              </div>
                              <p class="whitespace-pre-wrap">{r.description}</p>
                              {#if r.higher_levels}
                                <p class="whitespace-pre-wrap"><b style="color:#8b6914;">{$_('spells.higher')}:</b> {r.higher_levels}</p>
                              {/if}
                            </div>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  {:else if bookQuery && !bookLoading}
                    <p class="text-xs italic" style="color:#8b6355;">{$_('spells.none')}</p>
                  {/if}
                </div>
              </details>

              <!-- add custom -->
              <details class="mt-3">
                <summary class="cursor-pointer inline-flex items-center gap-1.5 text-sm font-display" style="color:#c9a84c;">
                  <Plus size={14} /> {$_('character.custom_spell')}
                </summary>
                <form onsubmit={(e) => { e.preventDefault(); addCustom(c); }}
                  class="mt-2 space-y-2">
                  <input required placeholder={$_('character.custom_name_ph')} bind:value={customName}
                    class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                  <label class="flex items-center gap-2 text-xs" style="color:#8b6914;">
                    {$_('character.custom_level')}
                    <select bind:value={customLevel}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      {#each [0,1,2,3,4,5,6,7,8,9] as l (l)}
                        <option value={l}>{l === 0 ? $_('spells.cantrip') : l}</option>
                      {/each}
                    </select>
                  </label>
                  <textarea rows="2" placeholder={$_('character.custom_desc_ph')} bind:value={customDesc}
                    class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
                  <button class="rounded bg-violet-600 px-3 py-1 text-sm text-white">{$_('common.add')}</button>
                </form>
              </details>
            </section>
          </div>
        </div>
        {/if}

        {#if tab === 'loot'}
        <div class="space-y-8">
          <!-- coin purse first -->
          <section class="sheet-block">
            <h4 class="sheet-h">{$_('character.coin')}</h4>
            <CoinPurse
              values={{
                pp: c.sheet?.coin?.pp ?? 0,
                gp: c.sheet?.coin?.gp ?? 0,
                ep: c.sheet?.coin?.ep ?? 0,
                sp: c.sheet?.coin?.sp ?? 0,
                cp: c.sheet?.coin?.cp ?? 0,
              }}
              onchange={(k, v) => patchSheet(c, (s) => ({ ...s, coin: { ...(s.coin ?? {}), [k]: v } }))} />
            {#if true}
              {@const totalGP = ((c.sheet?.coin?.pp ?? 0) * 10) + (c.sheet?.coin?.gp ?? 0) + ((c.sheet?.coin?.ep ?? 0) * 0.5) + ((c.sheet?.coin?.sp ?? 0) * 0.1) + ((c.sheet?.coin?.cp ?? 0) * 0.01)}
              {#if totalGP > 0}
                <div class="mt-1 text-xs text-right" style="color:#8b6355;">
                  ≈ <b style="color:#2c1810;">{totalGP % 1 === 0 ? totalGP : totalGP.toFixed(1)}</b> gp {$_('character.coin_total')}
                </div>
              {/if}
            {/if}
          </section>

          <!-- equipment list -->
          <section class="sheet-block">
            <h4 class="sheet-h">{$_('character.equipment')}</h4>

            {#if (c.sheet?.equipment ?? []).length}
              {@const strScore = c.sheet?.abilities?.str ?? 10}
              {@const w = totalWeight(c)}
              {@const cap = carryCapacity(c)}
              {@const over = w > cap}
              {@const lightEnc = w > strScore * 5}
              {@const heavyEnc = w > strScore * 10}
              {@const spdEnc = computedSpeed(c)}
              <div class="mb-2 text-xs" style="color:#8b6355;">
                {$_('character.equipment_total')}: <b style={over ? 'color:#8b1a1a;' : 'color:#2c1810;'}>{w.toFixed(1)} lb</b>
                / {$_('character.equipment_capacity')}: <b style="color:#2c1810;">{cap} lb</b> (STR × 15)
                · {$_('character.equipment_push')}: <b style="color:#2c1810;">{cap * 2} lb</b>
                {#if lightEnc}
                  <span class="ml-2 italic" style="color:#8b1a1a;">
                    {heavyEnc ? $_('character.equipment_heavy_encumbered') : $_('character.equipment_encumbered')}
                    — speed {spdEnc} ft
                  </span>
                {/if}
              </div>
              <ul class="space-y-1.5">
                {#each c.sheet?.equipment ?? [] as it (it.id)}
                  <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1">
                    <button type="button"
                      onclick={() => patchEq(c, it.id, { equipped: !it.equipped })}
                      class="rounded px-1.5 py-0.5 text-[10px] font-bold {it.equipped ? 'bg-amber-500 text-black' : 'bg-neutral-700'}"
                      style={it.equipped ? '' : 'color:#f4e4c1;'}>
                      {it.equipped ? $_('character.equip_label_yes') : $_('character.equip_label_no')}
                    </button>
                    <div class="flex items-center gap-0.5 shrink-0">
                      <button type="button" onclick={() => patchEq(c, it.id, { qty: Math.max(0, it.qty - 1) })}
                        class="w-5 h-5 rounded-full text-xs font-bold flex items-center justify-center"
                        style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;">−</button>
                      <span class="w-7 text-center text-sm font-bold tabular-nums" style="color:#f4e4c1;">{it.qty}</span>
                      <button type="button" onclick={() => patchEq(c, it.id, { qty: it.qty + 1 })}
                        class="w-5 h-5 rounded-full text-xs font-bold flex items-center justify-center"
                        style="background:linear-gradient(180deg,#c9a84c,#6d510f);border:1px solid #4e3909;color:#1a0f08;">+</button>
                    </div>
                    <input type="text" value={it.name}
                      onchange={(e) => patchEq(c, it.id, { name: (e.currentTarget as HTMLInputElement).value })}
                      class="flex-1 bg-transparent border-0 px-1 py-0.5 text-sm" />
                    {#if it.weight !== undefined}
                      <span class="text-xs" style="color:#8b6914;">{it.weight} lb</span>
                    {/if}
                    <button type="button" aria-label={$_('common.delete')} onclick={() => removeEq(c, it.id)}
                      class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="text-sm italic" style="color:#8b6355;">{$_('character.equipment_empty')}</p>
            {/if}

            <form onsubmit={(e) => { e.preventDefault(); addEq(c); }}
              class="mt-3 flex flex-wrap gap-2 items-end">
              <input required placeholder={$_('character.item_name_ph')} bind:value={newEqName}
                class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
              <div class="w-20">
                <Stepper compact label={$_('character.equipment_qty')} value={newEqQty} min={1} onchange={(v) => newEqQty = v} />
              </div>
              <label class="flex flex-col">
                <span class="text-[10px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{$_('character.equipment_weight')}</span>
                <input type="number" step="0.1" min="0" placeholder={$_('character.weight_inline_ph')}
                  bind:value={newEqWeight}
                  class="w-20 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
              </label>
              <button class="rounded bg-violet-600 px-3 py-1 text-sm text-white inline-flex items-center gap-1">
                <Plus size={14} /> {$_('common.add')}
              </button>
            </form>
            <details class="mt-2">
              <summary class="text-xs cursor-pointer" style="color:#8b6914;">Add from SRD catalog</summary>
              <div class="mt-1 space-y-0.5">
                {#each itemsByCat('armor') as item (item.slug)}
                  <button type="button" onclick={() => { const i = item; addFromCatalog(c, i); }}
                    class="block w-full text-left text-xs px-2 py-1 rounded hover:bg-neutral-800" style="color:#f4e4c1;">
                    {item.name} — {item.cost_gp} gp ({item.weight_lb} lb)
                  </button>
                {/each}
              </div>
            </details>
          </section>
        </div>
        {/if}

        {#if tab === 'features'}
          <div class="space-y-8">
            <!-- XP + classes -->
            <section class="sheet-block">
              <h4 class="sheet-h">{campaign().leveling === 'xp' ? $_('character.classes_xp') : $_('character.classes_progression')}</h4>
              <div class="grid sm:grid-cols-3 gap-3 mb-3">
                {#if campaign().leveling === 'xp'}
                  <label class="flex flex-col">
                    <span class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">{$_('character.xp')}</span>
                    <input type="number" min="0" value={c.sheet?.xp ?? 0}
                      onchange={(e) => patchSheet(c, (s) => ({ ...s, xp: Math.max(0, +(e.currentTarget as HTMLInputElement).value) }))}
                      class="text-sm" />
                  </label>
                  <div class="sm:col-span-2 text-xs" style="color:#8b6355;">
                    {$_('character.total_level')}: <b style="color:#2c1810;">{c.level_total}</b> · {$_('character.proficiency_bonus')}: <b style="color:#2c1810;">+{profBonus(c.level_total)}</b>
                  </div>
                {:else}
                  <div class="sm:col-span-3 text-xs" style="color:#8b6355;">
                    <span class="italic">{$_('character.classes_milestone_hint')}</span>
                    <div class="mt-1">
                      {$_('character.total_level')}: <b style="color:#2c1810;">{c.level_total}</b> · {$_('character.proficiency_bonus')}: <b style="color:#2c1810;">+{profBonus(c.level_total)}</b>
                    </div>
                  </div>
                {/if}
              </div>

              {#if (c.sheet?.classes ?? []).length}
                <ul class="space-y-1.5">
                  {#each c.sheet?.classes ?? [] as cls (cls.id)}
                    <li class="flex flex-col gap-1 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <div class="flex items-center gap-2">
                        <ClassAutocomplete value={cls.name}
                          onchange={(v) => patchSheet(c, (s) => {
                            const pruned = pruneClassData(s, cls.name, cls.subclass);
                            return { ...pruned, classes: (pruned.classes ?? []).map((x) => x.id === cls.id ? { ...x, name: v, subclass: undefined } : x) };
                          })} />
                        <SubclassAutocomplete value={cls.subclass ?? ''} className={cls.name}
                          onchange={(v) => patchSheet(c, (s) => {
                            const pruned = pruneClassData(s, cls.name, cls.subclass);
                            const features = (s.features ?? []).filter((f: any) => f.source !== 'subclass');
                            if (v) {
                              const seeded = getSubclassFeatures(cls.name, v) ?? [];
                              for (const sf of seeded) {
                                if (!features.some((f: any) => f.name.toLowerCase() === sf.name.toLowerCase())) {
                                  features.push({ id: randomUUID(), name: sf.name, source: 'subclass', description: sf.description || sf.name });
                                }
                              }
                            }
                            return { ...pruned, classes: (pruned.classes ?? []).map((x: any) => x.id === cls.id ? { ...x, subclass: v || undefined } : x), features };
                          })} />
                        <div class="lvl-stepper shrink-0">
                          <Stepper compact label={$_('character.level')} value={cls.level} min={1}
                            max={Math.min(20, c.level_total - ((c.sheet?.classes ?? []).filter((x) => x.id !== cls.id).reduce((s, x) => s + (x.level || 0), 0)))}
                            onchange={(v) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, level: v } : x) }))} />
                        </div>
                        <button aria-label={$_('common.remove')} class="text-red-400"
                          onclick={() => { if (!confirm($_('character.class_remove_confirm'))) return; patchSheet(c, (s) => {
                            const pruned = pruneClassData(s, cls.name, cls.subclass);
                            return { ...pruned, classes: (pruned.classes ?? []).filter((x) => x.id !== cls.id) };
                          }); }}>
                          <Trash2 size={12} />
                        </button>
                      </div>
                      <div class="flex items-center gap-2 text-[11px]">
                        <select value={cls.spellcasting_ability ?? ''}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, spellcasting_ability: (e.currentTarget as HTMLSelectElement).value as Ability || undefined } : x) }))}
                          class="rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5">
                          <option value="">Casting</option>
                          {#each ABILITIES as ab}<option value={ab}>{ab.toUpperCase()}</option>{/each}
                        </select>
                        <select value={cls.hit_die ?? hitDieFor(cls.name ?? '')}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, hit_die: (e.currentTarget as HTMLSelectElement).value } : x) }))}
                          class="rounded bg-neutral-900 border border-neutral-700 px-1 py-0.5">
                          <option value="d6">d6</option>
                          <option value="d8">d8</option>
                          <option value="d10">d10</option>
                          <option value="d12">d12</option>
                        </select>
                      </div>
                    </li>
                  {/each}
                </ul>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, classes: [ ...(s.classes ?? []), { id: randomUUID(), name: '', level: 1 } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> {$_('character.add_class')}
              </button>
            </section>

            <!-- resources -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.resources')} <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.resources_hint')}</span></h4>
              {#if (c.sheet?.resources ?? []).length}
                <div class="space-y-2">
                  {#each c.sheet?.resources ?? [] as r (r.id)}
                    <div class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <input type="text" value={r.name} placeholder={$_('character.resource_name_ph')}
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))}
                        class="flex-1 bg-transparent border-0 px-1 py-0.5" />
                      <SlotTrack current={r.current} max={r.max}
                        onchange={(cur, mx) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, current: cur, max: mx } : x) }))} />
                      <label class="flex items-center gap-1.5 text-xs font-display font-bold"
                        style="color:#2c1810;" title={$_('character.resource_refresh_tooltip')}>
                        <span class="px-1.5 py-0.5 rounded"
                          style="background:#8b6914;color:#f4e4c1;letter-spacing:0.12em;text-transform:uppercase;font-size:0.65rem;">{$_('character.refresh_on')}</span>
                        <select value={r.reset ?? 'long'}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, reset: (e.currentTarget as HTMLSelectElement).value as 'short' | 'long' | 'none' } : x) }))}
                          class="text-xs">
                          <option value="short">{$_('character.refresh_short')}</option>
                          <option value="long">{$_('character.refresh_long')}</option>
                          <option value="none">{$_('character.refresh_manual')}</option>
                        </select>
                      </label>
                      <button aria-label={$_('common.remove')} class="text-red-400"
                        onclick={() => { if (!confirm($_('character.resource_remove_confirm'))) return; patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).filter((x) => x.id !== r.id) })); }}>
                        <Trash2 size={12} />
                      </button>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">{$_('character.no_resources')}</p>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, resources: [ ...(s.resources ?? []), { id: randomUUID(), name: '', current: 0, max: 0, reset: 'long' } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> {$_('character.add_resource')}
              </button>
            </section>

            <!-- class / racial features -->
            <section class="sheet-block">
              <div class="flex items-center justify-between gap-2 mb-2">
                <h4 class="sheet-h">{$_('character.features_traits')}</h4>
                {#if canEdit(c)}
                  <button type="button" class="seed-open-btn" onclick={() => { seedOpen = true; seedClass = (c.sheet?.classes?.[0]?.name ?? ''); seedSubclass = (c.sheet?.classes?.[0]?.subclass ?? ''); seedSelected = new Set(); }}>
                    <Plus size={12} /> {$_('character.seed_features')}
                  </button>
                {/if}
              </div>
              {#if (c.sheet?.features ?? []).length}
                <div class="space-y-2">
                  {#each c.sheet?.features ?? [] as f (f.id)}
                    <details class="rounded" style="background:rgba(139,105,20,0.08); border:1px solid rgba(139,105,20,0.3);">
                      <summary class="flex items-center gap-2 px-2 py-1 cursor-pointer text-sm">
                        <span class="font-semibold flex-1">{f.name || $_('character.feature_name_ph')}</span>
                        {#if f.source}<span class="text-xs" style="color:#8b6914;">{f.source}</span>{/if}
                        {#if f.uses}
                          <span class="text-xs tabular-nums" style="color:#8b6355;">{f.uses.current}/{f.uses.max}</span>
                        {/if}
                      </summary>
                      <div class="px-3 py-2 text-sm space-y-2">
                        <div class="grid sm:grid-cols-3 gap-2">
                          <input type="text" value={f.name} placeholder={$_('character.feature_name_ph')}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))} />
                          <input type="text" value={f.source ?? ''} placeholder={$_('character.feature_source_ph')}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, source: (e.currentTarget as HTMLInputElement).value || undefined } : x) }))} />
                          <div class="flex items-center gap-2">
                            {#if f.uses}
                              <SlotTrack current={f.uses.current} max={f.uses.max}
                                onchange={(cur, mx) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { ...(x.uses!), current: cur, max: mx } } : x) }))} />
                              <label class="flex items-center gap-1.5 text-xs font-display font-bold"
                                style="color:#2c1810;" title={$_('character.feature_refresh_tooltip')}>
                                <span class="px-1.5 py-0.5 rounded"
                                  style="background:#8b6914;color:#f4e4c1;letter-spacing:0.12em;text-transform:uppercase;font-size:0.65rem;">{$_('character.refresh_on')}</span>
                                <select value={f.uses.reset ?? 'long'}
                                  onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { ...(x.uses!), reset: (e.currentTarget as HTMLSelectElement).value as 'short' | 'long' | 'none' } } : x) }))}
                                  class="text-xs">
                                  <option value="short">{$_('character.refresh_short')}</option>
                                  <option value="long">{$_('character.refresh_long')}</option>
                                  <option value="none">{$_('character.refresh_manual')}</option>
                                </select>
                              </label>
                              <button class="text-[10px] underline" style="color:#8b6914;"
                                onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: undefined } : x) }))}>
                                {$_('character.feature_remove_uses')}
                              </button>
                            {:else}
                              <button class="text-[10px] underline" style="color:#8b6914;"
                                onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { current: 1, max: 1, reset: 'long' } } : x) }))}>
                                {$_('character.feature_limited_uses')}
                              </button>
                            {/if}
                          </div>
                        </div>
                        <textarea rows="3" value={f.description ?? ''} placeholder={$_('character.feature_description_ph')}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, description: (e.currentTarget as HTMLTextAreaElement).value || undefined } : x) }))}
                          class="w-full text-sm"></textarea>
                        <button class="text-xs text-red-400 inline-flex items-center gap-1"
                          onclick={() => { if (!confirm($_('character.feature_remove_confirm'))) return; patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).filter((x) => x.id !== f.id) })); }}>
                          <Trash2 size={12} /> {$_('character.feature_remove')}
                        </button>
                      </div>
                    </details>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">{$_('character.no_features')}</p>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, features: [ ...(s.features ?? []), { id: randomUUID(), name: '' } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> {$_('character.add_feature')}
              </button>

              <!-- seed features modal -->
              {#if seedOpen && canEdit(c)}
                <div class="seed-modal-back" role="presentation"
                  onclick={() => seedOpen = false}
                  onkeydown={(e) => e.key === 'Escape' && (seedOpen = false)}>
                  <div class="seed-modal" role="dialog" aria-modal="true" tabindex="-1"
                    onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
                    <div class="seed-modal-head">
                      <h5>{$_('character.seed_features_title')}</h5>
                      <button onclick={() => seedOpen = false} aria-label={$_('common.close')} class="seed-close"><X size={14} /></button>
                    </div>
                    <div class="seed-modal-body">
                      <div class="grid grid-cols-2 gap-2 mb-3">
                        <div>
                          <datalist id="seed-classes">
                            {#each ALL_CLASS_NAMES as cn (cn)}<option value={cn}></option>{/each}
                          </datalist>
                          <input list="seed-classes" placeholder={$_('character.seed_class_ph')} bind:value={seedClass}
                            onchange={() => { seedSubclass = ''; seedSelected = new Set(); }}
                            class="seed-input" />
                        </div>
                        <div>
                          <datalist id="seed-subclasses">
                            {#each seedSubclasses as sc (sc)}<option value={sc}></option>{/each}
                          </datalist>
                          <input list="seed-subclasses" placeholder={$_('character.seed_subclass_ph')} bind:value={seedSubclass}
                            class="seed-input" />
                        </div>
                      </div>

                      {#if seedBaseFeatures.length === 0 && seedSubclassFeatures.length === 0}
                        <p class="text-sm italic" style="color:#8b6355;">{$_('character.seed_none')}</p>
                      {:else}
                        {#if seedBaseFeatures.length}
                          <div class="seed-group">
                            <div class="seed-group-head">
                              <span>{$_('character.seed_base')} — {seedClass}</span>
                              <button type="button" class="seed-select-all" onclick={() => toggleSeedAll(seedBaseFeatures, c)}>
                                {seedBaseFeatures.every((f) => featureAlreadyExists(c, f.name) || seedSelected.has(f.name)) ? $_('common.none') : $_('common.all')}
                              </button>
                            </div>
                            {#each seedBaseFeatures as f (f.name)}
                              {@const exists = featureAlreadyExists(c, f.name)}
                              {@const tooHigh = featLevelExceeds(c, f.level)}
                              <label class="seed-feat-row {exists || tooHigh ? 'seed-exists' : ''}">
                                <input type="checkbox" disabled={exists || tooHigh}
                                  checked={exists || seedSelected.has(f.name)}
                                  onchange={(e) => {
                                    const next = new Set(seedSelected);
                                    if ((e.currentTarget as HTMLInputElement).checked) next.add(f.name); else next.delete(f.name);
                                    seedSelected = next;
                                  }} />
                                <div class="seed-feat-info">
                                  <span class="seed-feat-name">Lv{f.level} {f.name}
                                    {#if exists}<span class="seed-exists-badge">{$_('character.seed_already')}</span>
                                    {:else if tooHigh}<span class="seed-exists-badge seed-too-high">Lv{f.level} req.</span>
                                    {/if}
                                  </span>
                                  <span class="seed-feat-desc">{f.description}</span>
                                </div>
                              </label>
                            {/each}
                          </div>
                        {/if}
                        {#if seedSubclassFeatures.length}
                          <div class="seed-group">
                            <div class="seed-group-head">
                              <span>{$_('character.seed_subclass')} — {seedSubclass}</span>
                              <button type="button" class="seed-select-all" onclick={() => toggleSeedAll(seedSubclassFeatures, c)}>
                                {seedSubclassFeatures.every((f) => featureAlreadyExists(c, f.name) || seedSelected.has(f.name)) ? $_('common.none') : $_('common.all')}
                              </button>
                            </div>
                            {#each seedSubclassFeatures as f (f.name)}
                              {@const exists = featureAlreadyExists(c, f.name)}
                              {@const tooHigh = featLevelExceeds(c, f.level)}
                              <label class="seed-feat-row {exists || tooHigh ? 'seed-exists' : ''}">
                                <input type="checkbox" disabled={exists || tooHigh}
                                  checked={exists || seedSelected.has(f.name)}
                                  onchange={(e) => {
                                    const next = new Set(seedSelected);
                                    if ((e.currentTarget as HTMLInputElement).checked) next.add(f.name); else next.delete(f.name);
                                    seedSelected = next;
                                  }} />
                                <div class="seed-feat-info">
                                  <span class="seed-feat-name">Lv{f.level} {f.name}
                                    {#if exists}<span class="seed-exists-badge">{$_('character.seed_already')}</span>
                                    {:else if tooHigh}<span class="seed-exists-badge seed-too-high">Lv{f.level} req.</span>
                                    {/if}
                                  </span>
                                  <span class="seed-feat-desc">{f.description}</span>
                                </div>
                              </label>
                            {/each}
                          </div>
                        {/if}
                      {/if}
                    </div>
                    <div class="seed-modal-foot">
                      <button onclick={() => seedOpen = false} class="seed-cancel">{$_('common.cancel')}</button>
                      <button onclick={() => applySeed(c)} disabled={seedSelected.size === 0} class="seed-confirm">
                        <Plus size={13} /> {$_('character.seed_add')} ({seedSelected.size})
                      </button>
                    </div>
                  </div>
                </div>
              {/if}
            </section>

            <!-- feats -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.feats')} <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.feats_hint')}</span></h4>

              <!-- taken feats -->
              {#if (c.sheet?.feats ?? []).length}
                <div class="space-y-1.5 mb-3">
                  {#each c.sheet?.feats ?? [] as fe (fe.id)}
                    {@const fd = featByKey(fe.key)}
                    {#if fd}
                      <div class="feat-row">
                        <div class="feat-info">
                          <span class="feat-name">{fd.name}
                            {#if fe.config?.ability}<span class="feat-cfg">({fe.config.ability.toUpperCase()})</span>{/if}
                            {#if fe.config?.class_name}<span class="feat-cfg">({fe.config.class_name})</span>{/if}
                            {#if fe.config?.damage_type}<span class="feat-cfg">({fe.config.damage_type})</span>{/if}
                          </span>
                          <span class="feat-mech">{fd.mechanics}</span>
                        </div>
                        {#if canEdit(c)}
                          <button class="feat-remove" onclick={() => removeFeat(c, fe)} title={$_('character.feat_remove')}>
                            <X size={12} />
                          </button>
                        {/if}
                      </div>
                    {/if}
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic mb-3" style="color:#8b6355;">{$_('character.feat_no_feats')}</p>
              {/if}

              <!-- feat picker -->
              {#if canEdit(c)}
                <div class="feat-picker">
                  <div class="feat-search-wrap">
                    <Search size={13} style="color:#8b6355;" />
                    <input class="feat-search-input" placeholder={$_('character.feat_search_ph')} bind:value={featSearch} />
                  </div>
                  <div class="feat-list">
                    {#each featsSearch(c) as f (f.key)}
                      {@const taken = charHasFeat(c, f.key) && !f.multi}
                      {@const metPrereqs = featPrereqsMet(f, (c.sheet ?? {}) as Record<string, unknown>)}
                      <div class="feat-item {taken ? 'feat-taken' : ''} {!metPrereqs ? 'feat-locked' : ''}">
                        <div class="feat-item-info">
                          <span class="feat-item-name">{f.name}</span>
                          <span class="feat-item-mech">{f.mechanics}</span>
                          {#if f.prereqs.length}
                            <span class="feat-item-prereq">
                              {#if !metPrereqs}⚠ {$_('character.feat_prereq_not_met')} · {/if}
                              {f.prereqs.map((p) => {
                                if (p.ability) return `${p.ability.key.toUpperCase()} ${p.ability.min}+`;
                                if (p.armor_prof) return `${p.armor_prof} armor prof`;
                                if (p.can_cast) return 'can cast spells';
                                return '';
                              }).filter(Boolean).join(', ')}
                            </span>
                          {/if}
                        </div>
                        {#if taken}
                          <span class="feat-badge">{$_('character.feat_already_taken')}</span>
                        {:else if !metPrereqs}
                          <span class="feat-badge feat-badge-locked">{$_('character.feat_prereq_not_met')}</span>
                        {:else if featConfigFeat?.key === f.key}
                          <!-- config panel -->
                          <div class="feat-config">
                            {#if f.effects.config_type === 'ability_choice' && f.effects.ability_choice}
                              <span class="feat-cfg-label">{$_('character.feat_config_ability')}</span>
                              <select bind:value={featConfigAbility} class="feat-cfg-sel">
                                <option value="">—</option>
                                {#each (f.effects.ability_choice ?? []) as ab (ab)}
                                  <option value={ab}>{ab.toUpperCase()}</option>
                                {/each}
                              </select>
                            {:else if f.effects.config_type === 'ability'}
                              <span class="feat-cfg-label">{$_('character.feat_config_ability')}</span>
                              <select bind:value={featConfigAbility} class="feat-cfg-sel">
                                <option value="">—</option>
                                {#each ABILITY_OPTIONS as ab (ab.key)}
                                  <option value={ab.key}>{ab.label}</option>
                                {/each}
                              </select>
                            {:else if f.effects.config_type === 'class'}
                              <span class="feat-cfg-label">{$_('character.feat_config_class')}</span>
                              <select bind:value={featConfigClass} class="feat-cfg-sel">
                                <option value="">—</option>
                                {#each CLASS_OPTIONS as cl (cl)}
                                  <option value={cl}>{cl}</option>
                                {/each}
                              </select>
                            {:else if f.effects.config_type === 'damage_type'}
                              <span class="feat-cfg-label">{$_('character.feat_config_damage')}</span>
                              <select bind:value={featConfigDamage} class="feat-cfg-sel">
                                <option value="">—</option>
                                {#each DAMAGE_OPTIONS as dt (dt)}
                                  <option value={dt}>{dt}</option>
                                {/each}
                              </select>
                            {:else if f.effects.config_type === 'skills'}
                              {@const needed = f.effects.free_skills ?? 1}
                              <span class="feat-cfg-label">{$_('character.feat_config_skills', { values: { n: needed } })}</span>
                              <div class="flex flex-wrap gap-1 mt-1">
                                {#each SKILLS as sk (sk.key)}
                                  {@const chosen = featConfigSkills.includes(sk.key)}
                                  {@const alreadyProf = !!(c.sheet?.skills as Record<string,string> | undefined)?.[sk.key]}
                                  <button type="button"
                                    onclick={() => {
                                      if (alreadyProf) return;
                                      if (chosen) featConfigSkills = featConfigSkills.filter((s) => s !== sk.key);
                                      else if (featConfigSkills.length < needed) featConfigSkills = [...featConfigSkills, sk.key];
                                    }}
                                    class="rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest transition-colors"
                                    style={alreadyProf
                                      ? 'background:rgba(139,105,20,0.06);color:rgba(109,81,15,0.4);border:1px solid rgba(139,105,20,0.15);cursor:not-allowed;'
                                      : chosen
                                        ? 'background:rgba(201,168,76,0.25);color:#6d510f;border:1px solid #8b6914;'
                                        : 'background:rgba(139,105,20,0.06);color:#6d510f;border:1px solid rgba(139,105,20,0.25);'}>
                                    {sk.key.replace('_',' ')}
                                  </button>
                                {/each}
                              </div>
                              <div class="text-[10px] mt-1" style="color:#8b6355;">{featConfigSkills.length}/{needed} {$_('character.feat_config_skills_selected')}</div>
                            {/if}
                            <div class="feat-cfg-btns">
                              <button class="feat-cfg-cancel" onclick={() => { featConfigFeat = null; featConfigAbility=''; featConfigClass=''; featConfigDamage=''; featConfigSkills=[]; }}>{$_('common.cancel')}</button>
                              <button class="feat-cfg-take" onclick={() => takeFeat(c, f)}>{$_('character.feat_add')}</button>
                            </div>
                          </div>
                        {:else}
                          <button class="feat-take-btn" onclick={() => {
                            if (f.effects.config_type) { featConfigFeat = f; featConfigAbility=''; featConfigClass=''; featConfigDamage=''; featConfigSkills=[]; }
                            else takeFeat(c, f);
                          }}>
                            <Plus size={12} /> {$_('character.feat_add')}
                          </button>
                        {/if}
                      </div>
                    {:else}
                      <p class="text-sm italic p-2" style="color:#8b6355;">{$_('character.feat_no_match')}</p>
                    {/each}
                  </div>
                </div>
              {/if}
            </section>

            <!-- attunement -->
            <section class="sheet-block">
              {#if true}
                {@const attCount = (c.sheet?.attunement ?? []).length}
                <h4 class="sheet-h inline-flex flex-wrap items-center gap-2">
                  <span>{$_('character.attunement')}</span>
                  <span class="text-[10px] rounded px-1.5 py-0.5 font-semibold"
                    style="background:{attCount >= 3 ? 'rgba(139,26,26,0.2)' : 'rgba(201,168,76,0.2)'}; color:{attCount >= 3 ? '#8b1a1a' : '#6d510f'}; border:1px solid {attCount >= 3 ? '#8b1a1a' : 'rgba(139,105,20,0.4)'};">
                    {attCount}/3
                  </span>
                  <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.attunement_hint')}</span>
                </h4>
              {/if}
              {#if (c.sheet?.attunement ?? []).length}
                <div class="space-y-3">
                  {#each c.sheet?.attunement ?? [] as it, idx (it.id)}
                    <details class="att-item" open>
                      <summary class="att-summary">
                        <span class="att-num">{idx + 1}</span>
                        {#if canEdit(c)}
                          <input type="text" value={it.name} placeholder={$_('character.attunement_item_ph')}
                            onclick={(e) => e.stopPropagation()}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))}
                            class="att-name-input" />
                        {:else}
                          <span class="att-name-plain">{it.name || '—'}</span>
                        {/if}
                        <button aria-label={$_('common.remove')} class="att-remove"
                          onclick={(e) => { e.stopPropagation(); if (!confirm($_('character.attunement_remove_confirm'))) return; patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).filter((x) => x.id !== it.id) })); }}>
                          <Trash2 size={12} />
                        </button>
                      </summary>

                      <div class="att-body">
                        <!-- description -->
                        {#if canEdit(c)}
                          <textarea rows="2" placeholder={$_('character.attunement_description_ph')}
                            value={it.description ?? ''}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, description: (e.currentTarget as HTMLTextAreaElement).value || undefined } : x) }))}
                            class="att-textarea"></textarea>
                        {:else if it.description}
                          <p class="att-desc-ro">{it.description}</p>
                        {/if}

                        <!-- bonuses -->
                        <div class="att-section-head">{$_('character.attunement_bonuses')}</div>
                        <div class="att-bonuses">
                          {#each [
                            { key: 'ac',       label: $_('character.attunement_bonus_ac') },
                            { key: 'speed',    label: $_('character.attunement_bonus_speed') },
                            { key: 'initiative', label: $_('character.attunement_bonus_initiative') },
                            { key: 'attack',   label: $_('character.attunement_bonus_attack') },
                            { key: 'damage',   label: $_('character.attunement_bonus_damage') },
                            { key: 'spell_dc', label: $_('character.attunement_bonus_spell_dc') },
                            { key: 'str', label: 'STR' }, { key: 'dex', label: 'DEX' },
                            { key: 'con', label: 'CON' }, { key: 'int', label: 'INT' },
                            { key: 'wis', label: 'WIS' }, { key: 'cha', label: 'CHA' },
                          ] as b (b.key)}
                            <label class="att-bonus-field">
                              <span>{b.label}</span>
                              {#if canEdit(c)}
                                <input type="number" value={(it.bonuses as Record<string,number|undefined>)?.[b.key] ?? 0}
                                  onchange={(e) => {
                                    const v = +(e.currentTarget as HTMLInputElement).value;
                                    patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, bonuses: { ...(x.bonuses ?? {}), [b.key]: v || undefined } } : x) }));
                                  }}
                                  class="att-bonus-input" />
                              {:else}
                                <span class="att-bonus-ro">{(it.bonuses as Record<string,number|undefined>)?.[b.key] ?? 0}</span>
                              {/if}
                            </label>
                          {/each}
                        </div>

                        <!-- charges -->
                        <div class="att-section-head">{$_('character.attunement_charges')}</div>
                        {#if it.charges}
                          <div class="att-charges">
                            <div class="att-spend-row">
                              {#each Array(it.charges.max) as _, i (i)}
                                <button type="button"
                                  class="rivet-btn {i < it.charges.current ? 'rivet-on' : 'rivet-off'}"
                                  onclick={() => {
                                    const cur = it.charges!.current;
                                    const next = i < cur ? i : i + 1;
                                    patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, charges: { ...(x.charges!), current: next } } : x) }));
                                  }}
                                  aria-label="charge {i+1}"></button>
                              {/each}
                              <span class="att-spend-count">{it.charges.current}/{it.charges.max}</span>
                              <span class="att-reset-tag">{$_(`character.attunement_reset_${it.charges.reset}`)}</span>
                            </div>
                            {#if canEdit(c)}
                              <label class="att-reset-wrap">
                                <span class="att-reset-label">{$_('character.attunement_charges_reset')}</span>
                                <select value={it.charges.reset}
                                  onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, charges: { ...(x.charges!), reset: (e.currentTarget as HTMLSelectElement).value as 'dawn'|'dusk'|'long'|'short'|'none' } } : x) }))}
                                  class="att-reset-sel">
                                  {#each ['dawn','dusk','long','short','none'] as r (r)}
                                    <option value={r}>{$_(`character.attunement_reset_${r}`)}</option>
                                  {/each}
                                </select>
                              </label>
                              <label class="att-reset-wrap">
                                <span class="att-reset-label">{$_('character.attunement_charges_die')}</span>
                                <input type="text" value={it.charges.recharge_die ?? ''}
                                  placeholder="1d6+1"
                                  onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, charges: { ...(x.charges!), recharge_die: (e.currentTarget as HTMLInputElement).value || undefined } } : x) }))}
                                  class="att-die-input" />
                              </label>
                              <button class="att-rm-charges" onclick={() => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, charges: undefined } : x) }))}>
                                <X size={11} />
                              </button>
                            {/if}
                          </div>
                        {:else if canEdit(c)}
                          <button class="att-add-sub" onclick={() => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, charges: { current: 3, max: 3, reset: 'dawn' } } : x) }))}>
                            <Plus size={11} /> {$_('character.attunement_charges')}
                          </button>
                        {/if}

                        <!-- item spell slots -->
                        <div class="att-section-head">{$_('character.attunement_spell_slots')}</div>
                        {#each Object.entries(it.spell_slots ?? {}).sort(([a],[b]) => +a - +b) as [lvl, sl] (lvl)}
                          <div class="att-slot-row">
                            <span class="att-slot-label">{$_('character.attunement_slot_level').replace('{{n}}', lvl)}</span>
                            <div class="att-spend-row">
                              {#each Array(sl.max) as _, i (i)}
                                <button type="button"
                                  class="rivet-btn {i < sl.current ? 'rivet-on' : 'rivet-off'}"
                                  onclick={() => {
                                    const next = i < sl.current ? i : i + 1;
                                    patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, spell_slots: { ...(x.spell_slots ?? {}), [lvl]: { current: next, max: sl.max } } } : x) }));
                                  }}
                                  aria-label="slot {i+1}"></button>
                              {/each}
                              <span class="att-spend-count">{sl.current}/{sl.max}</span>
                            </div>
                            {#if canEdit(c)}
                              <button class="att-rm-slot" onclick={() => patchSheet(c, (s) => {
                                const slots = { ...(s.attunement?.find((x) => x.id === it.id)?.spell_slots ?? {}) };
                                delete slots[lvl];
                                return { ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, spell_slots: slots } : x) };
                              })}><X size={11} /></button>
                            {/if}
                          </div>
                        {/each}
                        {#if canEdit(c)}
                          <select class="att-add-slot-sel"
                            onchange={(e) => {
                              const lvl = (e.currentTarget as HTMLSelectElement).value;
                              if (!lvl) return;
                              (e.currentTarget as HTMLSelectElement).value = '';
                              if ((it.spell_slots ?? {})[lvl]) return;
                              patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, spell_slots: { ...(x.spell_slots ?? {}), [lvl]: { current: 1, max: 1 } } } : x) }));
                            }}>
                            <option value="">+ {$_('character.attunement_add_slot')}</option>
                            {#each [1,2,3,4,5,6,7,8,9] as lvl (lvl)}
                              {#if !(it.spell_slots ?? {})[String(lvl)]}
                                <option value={String(lvl)}>{$_('character.attunement_slot_level').replace('{{n}}', String(lvl))}</option>
                              {/if}
                            {/each}
                          </select>
                        {/if}

                        <!-- notes -->
                        {#if canEdit(c)}
                          <input type="text" value={it.notes ?? ''} placeholder={$_('character.attunement_notes_ph')}
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, notes: (e.currentTarget as HTMLInputElement).value || undefined } : x) }))}
                            class="att-notes-input" />
                        {:else if it.notes}
                          <p class="att-notes-ro">{it.notes}</p>
                        {/if}
                      </div>
                    </details>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">{$_('character.no_attunement')}</p>
              {/if}
              {#if (c.sheet?.attunement ?? []).length < 3}
                <button type="button"
                  onclick={() => patchSheet(c, (s) => ({ ...s, attunement: [ ...(s.attunement ?? []), { id: randomUUID(), name: '' } ] }))}
                  class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                  <Plus size={12} /> {$_('character.attune_item')}
                </button>
              {:else}
                <p class="mt-2 text-xs italic" style="color:#8b1a1a;">{$_('character.attunement_limit')}</p>
              {/if}
            </section>
          </div>
        {/if}

        {#if tab === 'story'}
          {@const bg = c.sheet?.background ?? {}}
          {@const ap = c.sheet?.appearance ?? {}}
          {@const align = c.sheet?.alignment}
          <div class="space-y-6">

            {#if align}
              <section class="sheet-block">
                <h4 class="sheet-h">{$_('character.alignment')}</h4>
                <div class="flex items-center gap-2">
                  <span class="rounded px-3 py-1 text-sm font-display font-bold tracking-wider"
                    style="background:rgba(139,105,20,0.12);border:1px solid rgba(139,105,20,0.4);color:#6d510f;">
                    {align}
                  </span>
                  {#if canEdit(c)}
                    <button type="button" class="text-[10px] underline" style="color:#8b6355;"
                      onclick={() => patchSheet(c, (s) => ({ ...s, alignment: undefined }))}>{$_('common.clear')}</button>
                  {/if}
                </div>
              </section>
            {/if}

            <!-- Physical appearance -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.appearance')}</h4>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
                {#each [
                  { key: 'age',                 label: $_('character.appearance_age'),                 ph: '25' },
                  { key: 'height',              label: $_('character.appearance_height'),              ph: "5'10\" / 178cm" },
                  { key: 'weight',              label: $_('character.appearance_weight'),              ph: '160 lb / 73 kg' },
                  { key: 'eyes',                label: $_('character.appearance_eyes'),                ph: $_('character.appearance_eyes_ph') },
                  { key: 'skin',                label: $_('character.appearance_skin'),                ph: $_('character.appearance_skin_ph') },
                  { key: 'hair',                label: $_('character.appearance_hair'),                ph: $_('character.appearance_hair_ph') },
                ] as f (f.key)}
                  <label class="flex flex-col gap-0.5">
                    <span class="text-[10px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{f.label}</span>
                    <input type="text"
                      value={(ap as Record<string, string | undefined>)[f.key] ?? ''}
                      placeholder={f.ph}
                      disabled={!canEdit(c)}
                      onchange={(e) => patchSheet(c, (s) => ({
                        ...s,
                        appearance: { ...(s.appearance ?? {}), [f.key]: (e.currentTarget as HTMLInputElement).value || undefined },
                      }))}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                  </label>
                {/each}
              </div>
              <!-- distinguishing marks — wider field -->
              <label class="flex flex-col gap-0.5 mt-3">
                <span class="text-[10px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{$_('character.appearance_marks')}</span>
                <input type="text"
                  value={ap.distinguishing_marks ?? ''}
                  placeholder={$_('character.appearance_marks_ph')}
                  disabled={!canEdit(c)}
                  onchange={(e) => patchSheet(c, (s) => ({
                    ...s,
                    appearance: { ...(s.appearance ?? {}), distinguishing_marks: (e.currentTarget as HTMLInputElement).value || undefined },
                  }))}
                  class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
              </label>
            </section>

            <!-- Backstory + personality etc. -->
            {#each [
              { key: 'backstory',   rows: 6 },
              { key: 'personality', rows: 3 },
              { key: 'ideals',      rows: 2 },
              { key: 'bonds',       rows: 2 },
              { key: 'flaws',       rows: 2 },
              { key: 'notes',       rows: 4 },
            ] as f (f.key)}
              <section class="sheet-block">
                <h4 class="sheet-h">{$_(`character.${f.key}`)}</h4>
                <textarea rows={f.rows}
                  value={(bg as Record<string, string | undefined>)[f.key] ?? ''}
                  disabled={!canEdit(c)}
                  onchange={(e) => patchSheet(c, (s) => ({
                    ...s,
                    background: { ...(s.background ?? {}), [f.key]: (e.currentTarget as HTMLTextAreaElement).value || undefined },
                  }))}
                  class="w-full rounded bg-neutral-900 border border-neutral-700 px-3 py-2 resize-y"></textarea>
              </section>
            {/each}
          </div>
        {/if}

        {#if tab === 'spellbook'}
        <div class="space-y-8">
          <section class="sheet-block">
            <h4 class="sheet-h inline-flex items-center gap-1.5"><BookOpen size={14} /> {$_('character.spellbook')}</h4>

            {#if spellbookLoading}
              <p class="text-sm italic" style="color:#8b6355;">{$_('spells.loading')}</p>
            {/if}

            {#each groupedSpellbook(spellbook) as [lv, ss] (lv)}
              <div class="mb-3">
                <div class="flex items-center justify-between text-[11px] uppercase tracking-widest font-display" style="color:#8b6914;">
                  <span>{lv === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${lv}`}</span>
                </div>
                <ul class="mt-1 space-y-1">
                  {#each ss as s (s.spell_id)}
                    <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1">
                      <label class="inline-flex items-center gap-1.5 text-xs cursor-pointer" title={$_('character.toggle_prepared')}>
                        <input type="checkbox" checked={s.prepared}
                          onchange={() => toggleSpellbookPrepared(c, s)}
                          disabled={!canEdit(c)}
                          class="w-4 h-4 accent-amber-600" />
                        <span class="text-[10px] font-bold">{s.prepared ? $_('character.prepared') : $_('character.known')}</span>
                      </label>
                      <span class="flex-1 text-left text-sm truncate" style="color:#2c1810;">
                        {s.name}
                      </span>
                      {#if canEdit(c)}
                        <input type="text" value={s.notes ?? ''}
                          placeholder={$_('character.spellbook_notes_ph')}
                          onchange={(e) => updateSpellbookNotes(c, s, (e.currentTarget as HTMLInputElement).value)}
                          class="w-32 sm:w-48 bg-transparent border-0 border-b px-1 py-0.5 text-xs"
                          style="border-color:rgba(139,105,20,0.3); color:#6d510f;" />
                      {:else if s.notes}
                        <span class="text-xs italic" style="color:#6d510f;">{s.notes}</span>
                      {/if}
                      {#if canEdit(c)}
                        <button type="button" aria-label={$_('common.remove')} onclick={() => removeSpellbookSpell(c, s)}
                          class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                      {/if}
                    </li>
                  {/each}
                </ul>
              </div>
            {/each}

            {#if !spellbook.length && !spellbookLoading}
              <p class="text-sm italic" style="color:#8b6355;">{$_('character.spellbook_empty')}</p>
            {/if}

            {#if canEdit(c)}
              <details class="mt-4">
                <summary class="cursor-pointer inline-flex items-center gap-1.5 text-sm font-display" style="color:#c9a84c;">
                  <Search size={14} /> {$_('character.spellbook_add_spell')}
                </summary>
                <div class="mt-2 space-y-2">
                  <div class="flex gap-2 flex-wrap">
                    <input type="search" placeholder={$_('character.book_search_ph')}
                      bind:value={spellbookSearch} oninput={onSpellbookSearchInput}
                      class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                  </div>
                  {#if spellbookSearchLoading}<p class="text-xs italic" style="color:#8b6355;">{$_('spells.loading')}</p>{/if}
                  {#if spellbookSearchResults.length}
                    <ul class="max-h-56 overflow-y-auto space-y-1 border rounded"
                      style="border-color:#d4b896;">
                      {#each spellbookSearchResults as r (r.slug)}
                        {@const already = spellbook.some((s) => s.slug === r.slug)}
                        <li class="text-sm border-b" style="border-color:rgba(139,105,20,0.15);">
                          <div class="flex items-center gap-2 px-2 py-1 {already ? 'opacity-50' : ''}">
                            <span class="rounded bg-neutral-800 px-1.5 text-[10px] font-bold" style="color:#f4e4c1;">
                              {r.level === 0 ? 'C' : r.level}
                            </span>
                            <span class="flex-1 text-left truncate inline-flex items-center gap-1"
                              style="color:#2c1810;">
                              {r.name}
                              <span class="text-[10px]" style="color:#8b6914;">· {r.school}</span>
                            </span>
                            <button type="button" disabled={already}
                              onclick={() => addSpellbookSpell(c, r)}
                              class="rounded bg-violet-600 px-2 py-0.5 text-[11px] text-white disabled:opacity-40">
                              {already ? '✓' : $_('spells.learn')}
                            </button>
                          </div>
                        </li>
                      {/each}
                    </ul>
                  {:else if spellbookSearch && !spellbookSearchLoading}
                    <p class="text-xs italic" style="color:#8b6355;">{$_('spells.none')}</p>
                  {/if}
                </div>
              </details>
            {/if}
          </section>
        </div>
        {/if}
      </article>

      {#if selectedSpell}
        <div class="fixed inset-0 z-40 bg-black/70 flex items-center justify-center p-4"
          role="presentation"
          onclick={() => (selectedSpell = null)}
          onkeydown={(e) => e.key === 'Escape' && (selectedSpell = null)}>
          <div class="w-full max-w-lg rounded-lg border p-6 max-h-[80vh] overflow-y-auto"
            role="dialog" tabindex="-1"
            style="border-color:#8b6914; background:#f4e4c1; color:#2c1810;"
            onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
            <h3 class="text-2xl font-display font-bold" style="color:#6d510f;">{selectedSpell.name}</h3>
            <p class="text-sm italic" style="color:#8b6355;">
              {selectedSpell.level === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${selectedSpell.level}`}
              {#if selectedSpell.school} · {selectedSpell.school}{/if}
              {#if selectedSpell.ritual} · {$_('spells.ritual')}{/if}
              {#if selectedSpell.concentration} · {$_('spells.concentration')}{/if}
            </p>
            {#if selectedSpell.classes?.length}
              <p class="mt-2 text-xs" style="color:#8b6914;">{$_('spells.classes')}: {selectedSpell.classes.join(', ')}</p>
            {/if}
            {#if selectedSpell.source}
              <p class="mt-1 text-xs" style="color:#6d510f;"><b style="color:#8b6914;">{$_('spells.source')}:</b> {selectedSpell.source}</p>
            {/if}
            <div class="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              {#if selectedSpell.casting_time}
                <div><b style="color:#8b6914;">{$_('spells.cast')}:</b> {selectedSpell.casting_time}</div>
              {/if}
              {#if selectedSpell.range_text}
                <div><b style="color:#8b6914;">{$_('spells.range')}:</b> {selectedSpell.range_text}</div>
              {/if}
              {#if selectedSpell.components}
                <div class="col-span-2"><b style="color:#8b6914;">{$_('spells.components')}:</b> {selectedSpell.components}</div>
              {/if}
              {#if selectedSpell.duration}
                <div><b style="color:#8b6914;">{$_('spells.duration')}:</b> {selectedSpell.duration}</div>
              {/if}
            </div>
            {#if selectedSpell.level === 0}
              {@const mult = cantripDiceMultiplier(c.level_total)}
              {#if mult > 1}
                <p class="mt-2 text-xs font-semibold" style="color:#6d510f;">
                  {$_('spells.cantrip_scaling').replace('{{mult}}', String(mult)).replace('{{level}}', String(c.level_total))}
                </p>
              {/if}
            {/if}
            {#if selectedSpell.description}
              <p class="mt-3 whitespace-pre-wrap text-sm">{selectedSpell.description}</p>
            {/if}
            {#if selectedSpell.higher_levels}
              <p class="mt-2 whitespace-pre-wrap text-sm"><b style="color:#8b6914;">{$_('spells.higher')}:</b> {selectedSpell.higher_levels}</p>
            {/if}
            <div class="mt-4 flex justify-end">
              <button onclick={() => (selectedSpell = null)} class="rounded bg-violet-600 px-4 py-1.5 text-sm text-white">{$_('common.close')}</button>
            </div>
          </div>
        </div>
      {/if}
    {/if}
  {/if}

  {#if upcastSpell}
    {@const { spell: us, c: uc } = upcastSpell}
    <div class="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-4"
      role="presentation"
      onclick={() => (upcastSpell = null)}
      onkeydown={(e) => e.key === 'Escape' && (upcastSpell = null)}>
      <div class="w-full max-w-xs rounded-lg border p-5"
        role="dialog" tabindex="-1"
        style="border-color:#8b6914; background:#f4e4c1; color:#2c1810;"
        onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
        <h3 class="font-display font-bold text-lg" style="color:#6d510f;">{us.name}</h3>
        <p class="text-xs mt-1 mb-4" style="color:#8b6355;">{$_('spells.upcast_prompt')}</p>
        <div class="flex flex-col gap-2">
          {#each Object.entries(uc.sheet?.slots ?? {}).filter(([lvl, sl]) => parseInt(lvl) >= us.level && sl.current > 0).sort(([a],[b]) => parseInt(a)-parseInt(b)) as [lvl, sl]}
            <button type="button"
              onclick={() => castSpellAtLevel(uc, us, parseInt(lvl))}
              class="flex items-center justify-between rounded px-3 py-2 text-sm font-semibold"
              style="background:rgba(139,105,20,0.15);border:1px solid rgba(139,105,20,0.4);color:#2c1810;">
              <span>{$_('spells.level')} {lvl}</span>
              <span class="text-xs" style="color:#8b6914;">{sl.current}/{sl.max} {$_('spells.slots_left')}</span>
            </button>
          {/each}
        </div>
        <button onclick={() => (upcastSpell = null)} class="mt-4 text-xs underline" style="color:#8b6355;">{$_('common.cancel')}</button>
      </div>
    </div>
  {/if}
</section>

{#if rollResult}
  <div class="roll-toast" role="status" aria-live="polite">
    <span class="roll-toast-die">🎲</span>
    <span class="roll-toast-label">{rollResult.label}</span>
    <span class="roll-toast-total">{rollResult.total}</span>
    <span class="roll-toast-expr">({rollResult.expr})</span>
  </div>
{/if}

<style>
  /* read-only mode: viewer is not the sheet owner. Tabs remain clickable,
     but any mutating control inside is disabled. */
  :global(.readonly-sheet) :global(.sheet-block) button:not([data-allow-ro]),
  :global(.readonly-sheet) :global(.sheet-block) input,
  :global(.readonly-sheet) :global(.sheet-block) select,
  :global(.readonly-sheet) :global(.sheet-block) textarea {
    pointer-events: none !important;
    opacity: 0.9;
    cursor: default !important;
  }
  :global(.readonly-sheet) :global(.sheet-block) input,
  :global(.readonly-sheet) :global(.sheet-block) textarea,
  :global(.readonly-sheet) :global(.sheet-block) select {
    background: transparent !important;
    border-color: transparent !important;
  }
  /* header controls (avatar upload, delete, rests, level edit) are already
     conditionally rendered via canEdit(c). */

  /* Class-row level stepper — wide enough to show 2-digit values without
     truncation, with a brighter label against the dark row background. */
  .lvl-stepper { width: 7.5rem; }
  .lvl-stepper :global(.lbl) { color: #c9a84c !important; font-size: 0.75rem; }
  .lvl-stepper :global(.row input) { font-size: 0.95rem; }

  .ds-dot {
    height: 1.5rem;
    width: 1.5rem;
    border-radius: 9999px;
    border: 2px solid;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.35), 0 2px 4px rgba(0,0,0,0.4);
    transition: transform 0.08s;
  }
  .ds-dot:hover { transform: scale(1.1); }
  .ds-dot:active { transform: scale(0.95); }

  .sheet-block {
    padding-top: 0.5rem;
    padding-bottom: 0.25rem;
    border-top: 1px dashed rgba(139, 105, 20, 0.35);
  }
  .sheet-h {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    color: #c9a84c;
    margin-bottom: 0.75rem;
  }
  .sheet-tabs {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
    border-bottom: 1px solid rgba(139, 105, 20, 0.35);
    margin-bottom: 0.25rem;
  }
  .sheet-tab {
    padding: 0.4rem 0.6rem;
    font-family: 'Cinzel', serif;
    font-weight: 600;
    letter-spacing: 0.05em;
    font-size: 0.75rem;
    text-transform: uppercase;
    color: #8b6914;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
    white-space: nowrap;
  }
  @media (min-width: 640px) {
    .sheet-tab {
      padding: 0.5rem 1rem;
      letter-spacing: 0.08em;
      font-size: 0.85rem;
    }
  }
  .sheet-tab:hover { color: #6d510f; background: rgba(201,168,76,0.1); }
  .sheet-tab.active {
    color: #2c1810 !important;
    border-bottom-color: #8b6914;
    background: linear-gradient(180deg, rgba(201,168,76,0.25), transparent);
  }

  .lvl-badge {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.15rem 0.6rem;
    border-radius: 9999px;
    border: 1.5px solid #4e3909;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 1px 2px rgba(0,0,0,0.5);
    font-family: 'Cinzel', serif;
    color: #1a0f08 !important;
  }
  .lvl-label {
    font-size: 0.65rem;
    letter-spacing: 0.18em;
    text-transform: uppercase;
    font-weight: 700;
  }
  .lvl-value {
    font-size: 1.05rem;
    font-weight: 800;
    line-height: 1;
  }
  .lvl-badge input {
    width: 2.25rem;
    padding: 0 !important;
    background: transparent !important;
    border: 0 !important;
    box-shadow: none !important;
    color: #1a0f08 !important;
    font-family: 'Cinzel', serif !important;
    font-size: 1.05rem !important;
    font-weight: 800 !important;
    text-align: center;
    line-height: 1;
  }

  /* Attunement items */
  .att-item {
    border: 1.5px solid rgba(139,105,20,0.4); border-radius: 0.4rem;
    background: rgba(139,105,20,0.05); overflow: hidden;
  }
  .att-summary {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.5rem 0.75rem; cursor: pointer; list-style: none;
    background: rgba(139,105,20,0.1);
  }
  .att-summary::-webkit-details-marker { display: none; }
  .att-num {
    font-family: 'Cinzel', serif; font-weight: 800; font-size: 0.75rem;
    color: #8b6914; min-width: 1rem; text-align: center;
  }
  .att-name-input {
    flex: 1; background: transparent !important; border: 0 !important;
    border-bottom: 1px dashed rgba(139,105,20,0.4) !important;
    padding: 0 0 1px !important; outline: none;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.85rem;
    color: #2c1810 !important;
  }
  .att-name-plain { flex: 1; font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.85rem; color: #2c1810; }
  .att-remove { color: #8b1a1a; padding: 0.2rem; border-radius: 0.2rem; }
  .att-remove:hover { background: rgba(139,26,26,0.1); }
  .att-body { padding: 0.65rem 0.75rem; display: flex; flex-direction: column; gap: 0.55rem; }
  .att-textarea {
    width: 100%;
    border: 1.5px solid rgba(139,105,20,0.4) !important;
    background: rgba(244,228,193,0.6) !important;
    color: #2c1810 !important; border-radius: 0.3rem !important;
    padding: 0.35rem 0.55rem !important;
    font-family: 'Crimson Text', serif; font-size: 0.85rem; resize: vertical;
  }
  .att-desc-ro { font-family: 'Crimson Text', serif; font-size: 0.85rem; white-space: pre-wrap; color: #3a2313; }
  .att-section-head {
    font-family: 'IM Fell English SC', serif; font-size: 0.7rem;
    letter-spacing: 0.12em; text-transform: uppercase; color: #6d510f;
    border-bottom: 1px dashed rgba(139,105,20,0.3); padding-bottom: 0.15rem;
  }
  .att-bonuses {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(5.5rem, 1fr));
    gap: 0.35rem;
  }
  .att-bonus-field {
    display: flex; flex-direction: column; gap: 0.15rem;
    font-family: 'Cinzel', serif; font-size: 0.65rem;
    letter-spacing: 0.08em; text-transform: uppercase; color: #8b6914;
  }
  .att-bonus-input {
    border: 1.5px solid rgba(139,105,20,0.4) !important;
    background: rgba(244,228,193,0.6) !important;
    color: #2c1810 !important; border-radius: 0.25rem !important;
    padding: 0.2rem 0.4rem !important; font-size: 0.8rem !important;
    text-align: center; width: 100%;
  }
  .att-bonus-ro { font-size: 0.85rem; font-weight: 700; color: #2c1810; text-align: center; }
  .att-charges { display: flex; flex-direction: column; gap: 0.45rem; }
  .att-spend-row { display: flex; align-items: center; gap: 0.3rem; flex-wrap: wrap; }
  .rivet-btn {
    width: 1.1rem; height: 1.1rem; border-radius: 9999px;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.4), 0 1px 2px rgba(0,0,0,0.35);
    transition: transform 0.06s;
  }
  .rivet-btn:hover { transform: scale(1.18); }
  .rivet-btn.rivet-on { background: radial-gradient(circle at 35% 30%, #f4d97a 0%, #c9a84c 40%, #6d510f 100%); }
  .rivet-btn.rivet-off { background: radial-gradient(circle at 35% 30%, #d4b896 0%, #8b6355 70%); }
  .att-spend-count {
    font-family: 'Special Elite', monospace; font-size: 0.75rem;
    color: #6d510f; margin-left: 0.2rem; font-variant-numeric: tabular-nums;
  }
  .att-reset-tag {
    font-family: 'Cinzel', serif; font-size: 0.6rem; letter-spacing: 0.1em;
    text-transform: uppercase; color: #8b6914;
    padding: 0.1rem 0.4rem; border-radius: 0.2rem;
    background: rgba(139,105,20,0.1); border: 1px solid rgba(139,105,20,0.3);
    margin-left: 0.25rem;
  }
  .att-reset-wrap {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-family: 'Cinzel', serif; font-size: 0.68rem;
    letter-spacing: 0.06em; text-transform: uppercase; color: #6d510f;
  }
  .att-reset-label { white-space: nowrap; }
  .att-reset-sel {
    border: 1.5px solid rgba(139,105,20,0.4) !important;
    background: rgba(244,228,193,0.7) !important; color: #2c1810 !important;
    border-radius: 0.25rem !important; padding: 0.2rem 0.4rem !important;
    font-size: 0.78rem;
  }
  .att-die-input {
    width: 5rem;
    border: 1.5px solid rgba(139,105,20,0.4) !important;
    background: rgba(244,228,193,0.7) !important; color: #2c1810 !important;
    border-radius: 0.25rem !important; padding: 0.2rem 0.4rem !important;
    font-family: 'Special Elite', monospace; font-size: 0.78rem;
  }
  .att-rm-charges, .att-rm-slot {
    padding: 0.2rem; border-radius: 0.2rem; color: #8b1a1a; background: transparent;
  }
  .att-rm-charges:hover, .att-rm-slot:hover { background: rgba(139,26,26,0.1); }
  .att-add-sub {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.25rem 0.6rem; border-radius: 0.25rem;
    background: rgba(139,105,20,0.1); color: #6d510f;
    border: 1px solid rgba(139,105,20,0.35);
    font-family: 'Cinzel', serif; font-size: 0.68rem;
    letter-spacing: 0.07em; text-transform: uppercase;
  }
  .att-add-sub:hover { background: rgba(139,105,20,0.2); }
  .att-slot-row { display: flex; align-items: center; gap: 0.5rem; }
  .att-slot-label {
    font-family: 'Cinzel', serif; font-size: 0.72rem; min-width: 4rem;
    letter-spacing: 0.06em; text-transform: uppercase; color: #6d510f;
  }
  .att-add-slot-sel {
    margin-top: 0.25rem; width: 100%;
    border: 1.5px solid rgba(139,105,20,0.4) !important;
    background: rgba(244,228,193,0.6) !important; color: #6d510f !important;
    border-radius: 0.3rem !important; padding: 0.3rem 0.5rem !important;
    font-family: 'Cinzel', serif; font-size: 0.75rem;
  }
  .att-notes-input {
    width: 100%;
    border: 0 !important; border-bottom: 1px dashed rgba(139,105,20,0.35) !important;
    background: transparent !important; color: #6d510f !important;
    padding: 0 0 1px !important; font-family: 'Crimson Text', serif;
    font-size: 0.82rem; font-style: italic;
  }
  .att-notes-ro { font-family: 'Crimson Text', serif; font-size: 0.82rem; font-style: italic; color: #6d510f; }

  /* Seed features button + modal */
  .seed-open-btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.25rem 0.6rem; border-radius: 0.25rem;
    background: rgba(139,105,20,0.12); color: #6d510f;
    border: 1px solid rgba(139,105,20,0.4);
    font-family: 'Cinzel', serif; font-size: 0.68rem;
    letter-spacing: 0.07em; text-transform: uppercase;
    white-space: nowrap;
  }
  .seed-open-btn:hover { background: rgba(139,105,20,0.25); }

  .seed-modal-back {
    position: fixed; inset: 0; background: rgba(0,0,0,0.65);
    display: grid; place-items: center; z-index: 60; padding: 1rem;
  }
  .seed-modal {
    width: min(48rem, 100%); max-height: 80vh;
    display: flex; flex-direction: column;
    border: 2px solid #8b6914; border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 18px 40px rgba(0,0,0,0.65);
  }
  .seed-modal-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.7rem 1rem; border-bottom: 1px dashed rgba(139,105,20,0.45);
    flex-shrink: 0;
  }
  .seed-modal-head h5 {
    margin: 0; font-family: 'IM Fell English SC', serif;
    font-size: 1rem; letter-spacing: 0.08em; color: #2c1810;
  }
  .seed-close {
    width: 1.75rem; height: 1.75rem; display: grid; place-items: center;
    border-radius: 9999px; background: #3a2313; color: #c9a84c;
    border: 1px solid #4e3909;
  }
  .seed-close:hover { background: #4e3909; }
  .seed-modal-body { flex: 1; overflow-y: auto; padding: 0.75rem 1rem; }
  .seed-input {
    width: 100%;
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important;
    border-radius: 0.3rem !important;
    padding: 0.4rem 0.65rem !important;
    font-family: 'Crimson Text', serif; font-size: 0.88rem;
  }
  .seed-group { margin-bottom: 1rem; }
  .seed-group-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.3rem 0.5rem;
    background: rgba(139,105,20,0.12); border-radius: 0.25rem;
    margin-bottom: 0.35rem;
    font-family: 'IM Fell English SC', serif; font-size: 0.78rem;
    letter-spacing: 0.1em; text-transform: uppercase; color: #6d510f;
  }
  .seed-select-all {
    font-family: 'Cinzel', serif; font-size: 0.65rem;
    letter-spacing: 0.1em; text-transform: uppercase; color: #8b6914;
    text-decoration: underline; background: transparent; border: none;
  }
  .seed-feat-row {
    display: flex; gap: 0.5rem; align-items: flex-start;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px dashed rgba(139,105,20,0.15);
    cursor: pointer;
  }
  .seed-feat-row:last-child { border-bottom: 0; }
  .seed-feat-row.seed-exists { opacity: 0.55; cursor: default; }
  .seed-feat-row:not(.seed-exists):hover { background: rgba(139,105,20,0.07); }
  .seed-feat-info { flex: 1; min-width: 0; }
  .seed-feat-name {
    font-family: 'Cinzel', serif; font-size: 0.78rem; font-weight: 700;
    color: #2c1810; display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap;
  }
  .seed-too-high { background: rgba(139,26,26,0.1); color: #8b1a1a; border-color: rgba(139,26,26,0.3); }
  .seed-exists-badge {
    font-size: 0.6rem; padding: 0.05rem 0.35rem; border-radius: 0.2rem;
    background: rgba(139,105,20,0.15); color: #8b6914;
    border: 1px solid rgba(139,105,20,0.3); text-transform: uppercase;
    letter-spacing: 0.08em; font-family: 'Cinzel', serif; font-weight: 400;
  }
  .seed-feat-desc {
    display: block; font-family: 'Crimson Text', serif;
    font-size: 0.8rem; color: #6d510f; margin-top: 0.1rem;
  }
  .seed-modal-foot {
    display: flex; justify-content: flex-end; gap: 0.5rem;
    padding: 0.65rem 1rem; border-top: 1px dashed rgba(139,105,20,0.45);
    flex-shrink: 0;
  }
  .seed-cancel {
    padding: 0.4rem 0.85rem; border-radius: 0.3rem;
    background: #3a2313; color: #f4e4c1; border: 1px solid #6d510f;
    font-family: 'Cinzel', serif; font-size: 0.75rem;
    letter-spacing: 0.06em; text-transform: uppercase;
  }
  .seed-confirm {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.4rem 1rem; border-radius: 0.3rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08; border: 1px solid #4e3909;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.75rem;
    letter-spacing: 0.06em; text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.35);
  }
  .seed-confirm:hover:not(:disabled) { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .seed-confirm:disabled { opacity: 0.45; cursor: not-allowed; }

  /* Feats */
  .feat-row {
    display: flex; align-items: flex-start; justify-content: space-between; gap: 0.5rem;
    padding: 0.4rem 0.65rem;
    border: 1px solid rgba(139,105,20,0.3); border-radius: 0.3rem;
    background: rgba(139,105,20,0.06);
  }
  .feat-info { flex: 1; min-width: 0; }
  .feat-name { font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.82rem; color: #2c1810; }
  .feat-cfg { color: #8b6914; font-size: 0.72rem; margin-left: 0.3rem; }
  .feat-mech { display: block; font-family: 'Crimson Text', serif; font-size: 0.8rem; color: #6d510f; margin-top: 0.15rem; }
  .feat-remove {
    padding: 0.2rem; border-radius: 0.2rem; color: #8b1a1a;
    background: transparent;
  }
  .feat-remove:hover { background: rgba(139,26,26,0.1); }

  .feat-picker {
    border: 1.5px solid rgba(139,105,20,0.4); border-radius: 0.35rem;
    background: rgba(244,228,193,0.5);
    overflow: hidden;
  }
  .feat-search-wrap {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.65rem;
    border-bottom: 1px dashed rgba(139,105,20,0.3);
  }
  .feat-search-input {
    flex: 1; background: transparent !important; border: 0 !important;
    outline: none; font-family: 'Crimson Text', serif; font-size: 0.88rem;
    color: #2c1810 !important; padding: 0 !important;
  }
  .feat-list { max-height: 18rem; overflow-y: auto; }
  .feat-item {
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    padding: 0.5rem 0.65rem;
    border-bottom: 1px dashed rgba(139,105,20,0.15);
  }
  .feat-item:last-child { border-bottom: 0; }
  .feat-item.feat-taken { opacity: 0.55; }
  .feat-item.feat-locked { opacity: 0.65; }
  .feat-item-info { flex: 1; min-width: 0; }
  .feat-item-name { font-family: 'Cinzel', serif; font-size: 0.78rem; font-weight: 700; color: #2c1810; }
  .feat-item-mech { display: block; font-family: 'Crimson Text', serif; font-size: 0.77rem; color: #6d510f; }
  .feat-item-prereq { display: block; font-size: 0.68rem; color: #8b6355; font-style: italic; }
  .feat-badge {
    padding: 0.1rem 0.4rem; border-radius: 0.2rem; font-size: 0.6rem;
    letter-spacing: 0.08em; text-transform: uppercase; font-family: 'Cinzel', serif;
    background: rgba(139,105,20,0.12); color: #8b6914; border: 1px solid rgba(139,105,20,0.3);
    white-space: nowrap;
  }
  .feat-badge-locked { background: rgba(139,26,26,0.1); color: #8b1a1a; border-color: rgba(139,26,26,0.3); }
  .feat-take-btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.3rem 0.65rem; border-radius: 0.25rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08; border: 1px solid #4e3909;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.7rem;
    letter-spacing: 0.06em; text-transform: uppercase;
    white-space: nowrap;
  }
  .feat-take-btn:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .feat-config {
    display: flex; flex-direction: column; gap: 0.35rem; width: 100%;
    padding: 0.4rem 0; border-top: 1px dashed rgba(139,105,20,0.25); margin-top: 0.3rem;
  }
  .feat-cfg-label {
    font-family: 'IM Fell English SC', serif; font-size: 0.7rem; color: #6d510f;
    letter-spacing: 0.1em; text-transform: uppercase;
  }
  .feat-cfg-sel {
    border: 1.5px solid rgba(139,105,20,0.5) !important; background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important; border-radius: 0.25rem !important; padding: 0.3rem 0.5rem !important;
    font-family: 'Cinzel', serif; font-size: 0.78rem;
  }
  .feat-cfg-btns { display: flex; gap: 0.4rem; justify-content: flex-end; }
  .feat-cfg-cancel {
    padding: 0.3rem 0.65rem; border-radius: 0.25rem;
    background: #3a2313; color: #f4e4c1; border: 1px solid #6d510f;
    font-family: 'Cinzel', serif; font-size: 0.7rem; letter-spacing: 0.06em; text-transform: uppercase;
  }
  .feat-cfg-take {
    padding: 0.3rem 0.65rem; border-radius: 0.25rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08; border: 1px solid #4e3909;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.7rem; letter-spacing: 0.06em; text-transform: uppercase;
  }

  .roll-toast {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    z-index: 9999;
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.65rem 1rem;
    border-radius: 0.5rem;
    border: 1.5px solid #c9a84c;
    background: linear-gradient(135deg, #f4e4c1 0%, #e8d5a3 100%);
    box-shadow: 0 4px 20px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,248,220,0.6);
    animation: roll-toast-in 0.2s ease-out;
    max-width: min(22rem, calc(100vw - 2rem));
  }
  @keyframes roll-toast-in {
    from { opacity: 0; transform: translateY(0.75rem); }
    to   { opacity: 1; transform: translateY(0); }
  }
  .roll-toast-die { font-size: 1.25rem; line-height: 1; }
  .roll-toast-label {
    font-family: 'Cinzel', serif; font-size: 0.75rem; font-weight: 600;
    letter-spacing: 0.06em; text-transform: uppercase; color: #6d510f;
    flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .roll-toast-total {
    font-family: 'Cinzel', serif; font-size: 1.5rem; font-weight: 800;
    color: #2c1810; line-height: 1; padding: 0 0.25rem;
  }
  .roll-toast-expr {
    font-size: 0.7rem; color: #8b6355; white-space: nowrap;
  }
</style>

