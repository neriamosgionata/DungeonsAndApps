<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { Characters, Campaigns, Spells } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  const campaign = useCampaign();
  import Stepper from '$lib/components/Stepper.svelte';
  import SlotTrack from '$lib/components/SlotTrack.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import CoinPurse from '$lib/components/CoinPurse.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import { _ } from 'svelte-i18n';
  import { Trash2, Sparkles, Star, ChevronLeft, ChevronRight, BookOpen, Plus, Zap, Search, Swords, Skull, Heart, Bed, Moon, Brain, X } from '@lucide/svelte';
  import { DND_CLASSES, SPELLCASTER_CLASSES, isCustomClass as isCustomClassShared } from '$lib/dnd/classes';
  import { templatesForClass } from '$lib/dnd/resources';
  import { FEATS, featByKey, featPrereqsMet, type Feat, type Ability } from '$lib/feats';
  import { randomUUID } from '$lib/uuid';
  import { getBaseFeatures, getSubclassFeatures, listSubclasses, ALL_CLASS_NAMES } from '$lib/dnd/subclasses';

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

  type Sheet = {
    hp?: { current?: number; max?: number; temp?: number };
    hit_dice?: { current?: number; max?: number; die?: string };
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
    features?: Array<{ id: string; name: string; source?: string; description?: string; uses?: { current: number; max: number; reset?: 'short' | 'long' | 'none' } }>;
    classes?: Array<{ id: string; name: string; level: number; subclass?: string }>;
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
    background?: {
      backstory?: string;
      personality?: string;
      ideals?: string;
      bonds?: string;
      flaws?: string;
      notes?: string;
    };
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
      damage_type?: string;   // e.g. "slashing"
      range?: string;         // "melee" / "60/120 ft"
      properties?: string;    // "finesse, light"
      description?: string;   // freeform notes / flavor / effects
      equipped?: boolean;
    }>;
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

  let newName = $state('');
  let newRace = $state('');
  let newLevel = $state(1);

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
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t === 'character_updated' || t === 'combatant_updated') load();
    });
  });
  onDestroy(() => {
    offWs?.();
    clearTimeout(bookTimer);
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

    if (!toAdd.length && !slotsChanged) return;
    
    // Fix: queue patch but guard against re-entrancy by checking pending
    if (pendingPatch) return; // Already have pending patch
    pendingPatch = { c, patchFn: (s) => ({
      ...s,
      resources: toAdd.length ? [ ...(s.resources ?? []), ...toAdd ] : s.resources,
      slots: slotsChanged ? nextSlots : s.slots,
    })};
    
    queueMicrotask(() => {
      if (!pendingPatch) return;
      const { c: char, patchFn } = pendingPatch;
      pendingPatch = null;
      patchSheet(char, patchFn);
    });
  });

  // own characters count for gating
  const owned = $derived(list.filter((c) => c.owner_id === auth.user?.id).length);
  const canCreate = $derived(campaign().isMaster || owned < limit);

  async function create(close: () => void) {
    busy = true;
    try {
      await Characters.create(cid, { name: newName, race: newRace, level_total: newLevel });
      newName = ''; newRace = ''; newLevel = 1;
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
        // stabilized: reset both, restore alive, set HP to 1 (stable but unconscious)
        sheet.death_saves = { successes: 0, failures: 0 };
        sheet.alive = true;
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
  function profBonus(level: number): number {
    // standard 5e proficiency scaling
    return 2 + Math.floor((Math.max(1, level) - 1) / 4);
  }
  function saveMod(c: Character, ab: Ability): number {
    const ov = c.sheet?.saves_override?.[ab];
    if (typeof ov === 'number') return ov;
    const mod = abilityMod(c.sheet?.abilities?.[ab]);
    return mod + (c.sheet?.saves?.[ab] ? profBonus(c.level_total) : 0);
  }
  function abilityScore(c: Character, ab: Ability): number {
    return c.sheet?.abilities_override?.[ab] ?? c.sheet?.abilities?.[ab] ?? 10;
  }
  function hasAbilityOverride(c: Character, ab: Ability): boolean {
    return typeof c.sheet?.abilities_override?.[ab] === 'number';
  }
  function hasSaveOverride(c: Character, ab: Ability): boolean {
    return typeof c.sheet?.saves_override?.[ab] === 'number';
  }
  function skillMod(c: Character, sk: Skill): number {
    const mod = abilityMod(c.sheet?.abilities?.[sk.ability]);
    const lvl = c.sheet?.skills?.[sk.key];
    const pb = profBonus(c.level_total);
    if (lvl === 'expert') return mod + pb * 2;
    if (lvl === 'prof')   return mod + pb;
    return mod;
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
    await patchSheet(c, (s) => {
      const resources = (s.resources ?? []).map((r) =>
        r.reset === 'short' || r.reset === 'long' ? { ...r, current: r.max } : r);
      const features = (s.features ?? []).map((f) =>
        f.uses && (f.uses.reset === 'short' || f.uses.reset === 'long')
          ? { ...f, uses: { ...f.uses, current: f.uses.max } } : f);

      // Warlock pact magic: refill slots at the warlock's pact-slot level.
      const warlock = (s.classes ?? []).find((cl) =>
        cl.name?.trim().toLowerCase() === 'warlock');
      let slots = s.slots;
      if (warlock) {
        const lvl = String(warlockPactSlotLevel(warlock.level));
        if (lvl !== '0' && s.slots?.[lvl]) {
          slots = { ...s.slots, [lvl]: { ...s.slots[lvl], current: s.slots[lvl].max } };
        }
      }

      return { ...s, resources, features, slots };
    });
  }
  async function longRest(c: Character) {
    if (!confirm($_('character.long_rest_confirm'))) return;
    await patchSheet(c, (s) => {
      const hp = { ...(s.hp ?? {}), current: s.hp?.max ?? 0, temp: 0 };
      const maxHd = s.hit_dice?.max ?? 0;
      const curHd = s.hit_dice?.current ?? 0;
      const hit_dice = { ...(s.hit_dice ?? {}), current: Math.min(maxHd, curHd + Math.max(1, Math.floor(maxHd / 2))) };
      const slots: Record<string, { current: number; max: number }> = {};
      for (const [k, v] of Object.entries(s.slots ?? {})) slots[k] = { ...v, current: v.max };
      const resources = (s.resources ?? []).map((r) => r.reset !== 'none' ? { ...r, current: r.max } : r);
      const features  = (s.features  ?? []).map((f) =>
        f.uses && f.uses.reset !== 'none' ? { ...f, uses: { ...f.uses, current: f.uses.max } } : f);
      const exhaustion = Math.max(0, (s.exhaustion ?? 0) - 1);
      const death_saves = { successes: 0, failures: 0 };
      return { ...s, hp, hit_dice, slots, resources, features, exhaustion, death_saves, active_effects: [], concentration: null };
    });
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
    if (['paladin','ranger'].includes(n)) return 'half';
    if (n === 'warlock') return 'warlock';
    // third-casters: Fighter/Eldritch Knight, Rogue/Arcane Trickster
    if (n === 'fighter' && sub.includes('eldritch'))   return 'third';
    if (n === 'rogue'   && sub.includes('arcane'))     return 'third';
    // fighter/rogue w/o magical subclass = not a caster
    if (n === 'fighter' || n === 'rogue') return 'none';
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
      else if (t === 'half')  total += Math.floor(cl.level / 2);
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
        // PHB Paladin/Ranger table is identical to FULL at their floor(level/2) row.
        const casterLv = Math.floor(cl.level / 2);
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
      if (L >=  5) return 2; if (L >=  2) return 1;
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
        alert(`Cannot learn ${s.name}: your highest spell-slot level is ${maxSlotLevel(c)}.`);
        return;
      }
    } else if (!canLearn(c, { level: s.level, classes: s.classes })) {
      alert(`Cannot learn ${s.name}: not on your class list or above your class level's access.`);
      return;
    }
    const list = [...(c.sheet?.spells ?? []), s];
    await patchSheet(c, (sh) => ({ ...sh, spells: list }));
  }
  async function removeSpell(c: Character, s: CharSpell) {
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
      // cantrip: no slot but may still be a passive (e.g. Guidance, Resistance)
      if (isPassiveSpell(s) || s.concentration) {
        await patchSheet(c, (sh) => applyCastEffects(sh, s));
      }
      return;
    }
    const key = String(s.level);
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
  function applyFeatEffects(sh: Sheet, feat: Feat, config: { ability?: string; class_name?: string; damage_type?: string }, remove = false): Sheet {
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
      if (remove) delete saves[feat.effects.save_prof];
      else saves[feat.effects.save_prof] = true;
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
    next.abilities = ab as Sheet['abilities'];
    next.proficiencies = prof as Sheet['proficiencies'];
    next.senses = senses as Sheet['senses'];
    next.saves = saves as Sheet['saves'];
    return next;
  }

  async function takeFeat(c: Character, feat: Feat) {
    const config: { ability?: string; class_name?: string; damage_type?: string } = {};
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
    featConfigAbility = ''; featConfigClass = ''; featConfigDamage = '';
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

  async function removeFeat(c: Character, featEntry: { id: string; key: string; config?: { ability?: string; class_name?: string; damage_type?: string } }) {
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

  function canEdit(c: Character): boolean {
    // Only owners can modify their own character sheet. Master/admin observe
    // but cannot edit — use combat or NPC tools for their roles.
    return c.owner_id === auth.user?.id;
  }

  type Tab = 'vitals' | 'combat' | 'magic' | 'loot' | 'features' | 'story';
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
    const next = (c.sheet?.equipment ?? []).filter((it) => it.id !== id);
    await patchSheet(c, (s) => ({ ...s, equipment: next }));
  }

  // ---- weapon helpers ----
  let newWpName = $state('');
  let newWpAtk = $state<number>(0);
  let newWpDmg = $state('');
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
      damage_type: newWpDmgType.trim() || undefined,
      range: newWpRange.trim() || undefined,
      properties: newWpProps.trim() || undefined,
      description: newWpDesc.trim() || undefined,
      equipped: false,
    };
    await patchSheet(c, (s) => ({ ...s, weapons: [ ...(s.weapons ?? []), w ] }));
    newWpName = ''; newWpAtk = 0; newWpDmg = ''; newWpDmgType = ''; newWpRange = ''; newWpProps = ''; newWpDesc = '';
  }
  async function patchWeapon(c: Character, id: string, patch: Record<string, unknown>) {
    const next = (c.sheet?.weapons ?? []).map((it) => it.id === id ? { ...it, ...patch } : it);
    await patchSheet(c, (s) => ({ ...s, weapons: next }));
  }
  async function removeWeapon(c: Character, id: string) {
    const next = (c.sheet?.weapons ?? []).filter((it) => it.id !== id);
    await patchSheet(c, (s) => ({ ...s, weapons: next }));
  }

  const current = $derived(list[idx]);
</script>

<section class="mx-auto max-w-6xl px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-xl font-semibold">{$_('character.title')}</h2>
    {#if canCreate}
      <CollapsibleAdd label={$_('character.new')} title={$_('character.new')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2 sm:grid-cols-2">
            <input required placeholder={$_('character.name')} bind:value={newName}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <input placeholder={$_('character.race')} bind:value={newRace}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <input type="number" min="1" max="20" placeholder={$_('character.level')} bind:value={newLevel}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
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
      <article class="mt-4 rounded-lg border border-neutral-800 bg-neutral-900 p-6 lg:p-10 space-y-8 {canEdit(c) ? '' : 'readonly-sheet'}">
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
                <span class="lvl-badge" title={$_('character.level')}>
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
                  {#if c.sheet?.alive === false}<Skull size={12} /> {$_('character.dead')}{:else}<Heart size={12} fill="currentColor" /> {$_('character.alive')}{/if}
                </span>
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
                {:else if c.sheet?.inspiration}
                  <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;">
                    <Star size={12} fill="currentColor" /> {$_('character.inspiration_active')}
                  </span>
                {/if}
              </div>
              <p class="mt-1 text-sm text-neutral-400">
                {c.race ?? '—'}{#if (c.sheet?.classes ?? []).some((cl) => cl.name?.trim())} · {(c.sheet?.classes ?? []).filter((cl) => cl.name?.trim()).map((cl) => `${cl.name}${cl.subclass ? ` (${cl.subclass})` : ''} ${cl.level}`).join(' / ')}{/if}
              </p>

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
                {#if campaign().leveling === 'xp'}
                  <span>{$_('character.xp')} <b style="color:#2c1810;">{c.sheet?.xp ?? 0}</b></span>
                {:else}
                  <span class="italic">{$_('character.milestone')}</span>
                {/if}
              </div>

              {#if (c.sheet?.resources ?? []).some((r) => classResourceNames(c).has(r.name.trim().toLowerCase()))}
                <div class="mt-2 flex flex-wrap gap-1.5">
                  {#each (c.sheet?.resources ?? []).filter((r) => classResourceNames(c).has(r.name.trim().toLowerCase())) as r (r.id)}
                    <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                      style={r.current <= 0
                        ? 'background:rgba(139,26,26,0.2);color:#a93535;border:1px solid #8b1a1a;'
                        : 'background:rgba(201,168,76,0.22);color:#6d510f;border:1px solid rgba(139,105,20,0.5);'}
                      title="{r.reset ?? 'manual'} rest reset">
                      {r.name}: <b style="color:#2c1810;">{r.current}/{r.max}</b>
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
          <button class="sheet-tab {tab === 'vitals' ? 'active' : ''}" onclick={() => tab = 'vitals'}>{$_('character.tab_vitals')}</button>
          <button class="sheet-tab {tab === 'combat' ? 'active' : ''}" onclick={() => tab = 'combat'}>{$_('character.tab_combat')}</button>
          <button class="sheet-tab {tab === 'magic'  ? 'active' : ''}" onclick={() => tab = 'magic'}>{$_('character.tab_magic')}</button>
          <button class="sheet-tab {tab === 'loot'   ? 'active' : ''}" onclick={() => tab = 'loot'}>{$_('character.tab_loot')}</button>
          <button class="sheet-tab {tab === 'features' ? 'active' : ''}" onclick={() => tab = 'features'}>{$_('character.tab_features')}</button>
          <button class="sheet-tab {tab === 'story'  ? 'active' : ''}" onclick={() => tab = 'story'}>{$_('character.tab_story')}</button>
        </div>

        {#if tab === 'vitals'}
        <!-- vitals block -->
        <section class="sheet-block">
          <h4 class="sheet-h">{$_('character.vitals')}</h4>
          <div class="grid grid-cols-3 gap-4">
            <Stepper label={$_('character.hp_current')} value={hp.current ?? 0} min={0} max={hp.max ?? 999}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, current: v } }))} />
            <Stepper label={$_('character.hp_max')} value={hp.max ?? 0} min={0}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, max: v, current: Math.min(s.hp?.current ?? 0, v) } }))} />
            <Stepper label={$_('character.temp_hp')} value={hp.temp ?? 0} min={0}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, temp: v } }))} />
          </div>
          {#if (hp.max ?? 0) > 0}
            {@const cur = hp.current ?? 0}
            {@const mx  = hp.max ?? 1}
            {@const tmp = hp.temp ?? 0}
            {@const denom = Math.max(mx, cur + tmp, 1)}
            {@const pct = Math.max(0, Math.min(100, (cur / denom) * 100))}
            {@const tmpPct = Math.max(0, Math.min(100 - pct, (tmp / denom) * 100))}
            <div class="mt-3 h-3 rounded-full overflow-hidden relative"
              style="background:#2c1810; border:1px solid rgba(139,105,20,0.55);">
              <div class="absolute inset-y-0 left-0 transition-[width] duration-200"
                style={`width:${pct}%; background:linear-gradient(180deg,#8aa86f,#4f6d36);`}></div>
              {#if tmp > 0}
                <div class="absolute inset-y-0 transition-[width] duration-200"
                  style={`left:${pct}%; width:${tmpPct}%; background:linear-gradient(180deg,#a8d4cb,#4a7f76); box-shadow:inset 0 1px 0 rgba(255,248,220,0.35);`}
                  title={$_('character.temporary_hp')}></div>
              {/if}
            </div>
            <div class="mt-1 text-xs flex items-center gap-2" style="color:#8b6355;">
              <span>{cur}/{mx}</span>
              {#if tmp > 0}
                <span class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-bold"
                  style="background:rgba(74,127,118,0.25); color:#2f6058; border:1px solid #2f6058;">
                  +{tmp} {$_('character.temp_short')}
                </span>
                <span>→ {cur + tmp} {$_('character.effective')}</span>
              {/if}
            </div>
          {/if}
        </section>

        <div class="space-y-8">
          <!-- LEFT → now full-width under vitals tab -->
          <div class="space-y-8">
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.hit_dice')}</h4>
              <div class="grid grid-cols-3 gap-4">
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
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.status')}</h4>
              <div class="grid grid-cols-2 gap-4">
                <Stepper label={$_('character.exhaustion')} value={c.sheet?.exhaustion ?? 0} min={0} max={6}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, exhaustion: v }))} />
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
                  <input type="text" value={c.sheet?.languages ?? ''} placeholder={$_('character.languages_ph')}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, languages: (e.currentTarget as HTMLInputElement).value }))}
                    class="w-full text-sm" />
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mt-3 mb-1" style="color:#8b6914;">{$_('character.proficiencies')}</div>
                  {#each ['armor','weapons','tools'] as k (k)}
                    <div class="flex items-center gap-2 mb-1">
                      <span class="w-16 text-xs" style="color:#8b6914;">{$_(`character.prof_${k}`)}</span>
                      <input type="text"
                        value={(c.sheet?.proficiencies as Record<string, string | undefined> | undefined)?.[k] ?? ''}
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, proficiencies: { ...(s.proficiencies ?? {}), [k]: (e.currentTarget as HTMLInputElement).value } }))}
                        class="flex-1 text-sm" />
                    </div>
                  {/each}
                </div>
              </div>
            </section>

          </div>
        </div>
        {/if}

        {#if tab === 'combat'}
        {@const ab = c.sheet?.abilities ?? {}}
        {@const dexMod = Math.floor((((ab.dex ?? 10) as number) - 10) / 2)}
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
                    <div class="text-xs" style="color:#8b6355;">{mod >= 0 ? '+' : ''}{mod}</div>
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
                <Stepper label={$_('character.ac')} value={c.sheet?.ac ?? 10} min={0} max={40}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, ac: v }))} />
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
                <Stepper label={$_('character.speed')} value={c.sheet?.speed ?? 30} min={0} max={120} step={5}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, speed: v }))} />
              </div>
            </section>

            <!-- saving throws + skills -->
            <section class="sheet-block">
              <h4 class="sheet-h">{$_('character.saving_throws')}</h4>
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
                        <span class="tabular-nums font-bold text-sm" style="color:#2c1810;">{sm >= 0 ? '+' : ''}{sm}</span>
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
              <h4 class="sheet-h">{$_('character.skills')} <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.skills_hint')}</span></h4>
              <div class="grid sm:grid-cols-2 gap-1">
                {#each SKILLS as sk (sk.key)}
                  {@const lvl = c.sheet?.skills?.[sk.key]}
                  {@const mod = skillMod(c, sk)}
                  <button type="button" onclick={() => cycleSkill(c, sk.key)}
                    class="flex items-center justify-between gap-2 rounded px-2 py-1 text-sm"
                    style={`background:${lvl ? 'rgba(201,168,76,0.18)' : 'rgba(139,105,20,0.05)'}; border:1px solid rgba(139,105,20,0.25);`}>
                    <span class="flex items-center gap-2">
                      <span class="h-3 w-3 rounded-full border flex items-center justify-center text-[8px] font-bold"
                        style={`border-color:#8b6914; background:${lvl === 'expert' ? '#8b6914' : lvl === 'prof' ? '#c9a84c' : 'transparent'}; color:#1a0f08;`}>
                        {lvl === 'expert' ? '★' : ''}
                      </span>
                      <span>{$_(`character.skill_${sk.key}`)}</span>
                      <span class="text-[10px] uppercase" style="color:#8b6914;">{$_(`character.ability_${sk.ability}`)}</span>
                    </span>
                    <span class="tabular-nums font-bold" style="color:#2c1810;">{mod >= 0 ? '+' : ''}{mod}</span>
                  </button>
                {/each}
              </div>
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
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.damage ?? ''} placeholder={$_('character.weapon_damage_inline_ph')}
                              onchange={(e) => patchWeapon(c, w.id, { damage: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5" />
                            <input type="text" value={w.damage_type ?? ''} placeholder={$_('character.weapon_damage_type_ph')}
                              onchange={(e) => patchWeapon(c, w.id, { damage_type: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5 text-xs italic" style="color:#8b6914;" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.range ?? ''} placeholder={$_('character.weapon_range')}
                              onchange={(e) => patchWeapon(c, w.id, { range: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5" />
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
                              rows="1"
                              class="w-full bg-transparent border-0 border-b px-1 py-0.5 text-xs italic resize-y"
                              style="border-color:rgba(139,105,20,0.2); color:#5c3d2e;"></textarea>
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">{$_('character.enchantments_empty')}</p>
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
                <textarea placeholder={$_('character.weapon_description')} bind:value={newWpDesc} rows="2"
                  class="col-span-2 md:col-span-6 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
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
              <div class="space-y-2">
                {#each ['1','2','3','4','5','6','7','8','9'] as lvl (lvl)}
                  {@const s = slot(c, lvl)}
                  {#if s.max > 0}
                    <SlotTrack label={`${$_('spells.level')} ${lvl}`} current={s.current} max={s.max}
                      onchange={(cur, mx) => patchSheet(c, (sh) => ({ ...sh, slots: { ...(sh.slots ?? {}), [lvl]: { current: cur, max: mx } } }))} />
                  {/if}
                {/each}
              </div>
              <details class="mt-3">
                <summary class="cursor-pointer text-xs text-neutral-500 hover:text-neutral-300">{$_('character.add_slot_level')}</summary>
                <div class="mt-2 flex flex-wrap gap-1.5">
                  {#each ['1','2','3','4','5','6','7','8','9'] as lvl (lvl)}
                    {#if slot(c, lvl).max === 0}
                      <button type="button"
                        onclick={() => patchSheet(c, (sh) => ({ ...sh, slots: { ...(sh.slots ?? {}), [lvl]: { current: 1, max: 1 } } }))}
                        class="rounded bg-neutral-800 hover:bg-violet-700 px-2 py-0.5 text-xs"
                        style="color:#f4e4c1;">+L{lvl}</button>
                    {/if}
                  {/each}
                </div>
              </details>
            </section>

            <!-- spellcasting stats -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Zap size={14} /> {$_('character.spellcasting')}</h4>
              <div class="grid grid-cols-3 gap-4">
                <label class="flex flex-col">
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">{$_('character.casting_ability')}</span>
                  <select value={c.sheet?.casting?.ability ?? ''}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), ability: (e.currentTarget as HTMLSelectElement).value || undefined } }))}
                    class="mt-0.5 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    <option value="">—</option>
                    {#each ['INT','WIS','CHA','STR','DEX','CON'] as a (a)}<option>{a}</option>{/each}
                  </select>
                </label>
                <Stepper label={$_('character.spell_attack_bonus')} value={c.sheet?.casting?.spell_attack ?? 0} min={-5} max={20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), spell_attack: v } }))} />
                <Stepper label={$_('character.spell_save_dc')} value={c.sheet?.casting?.save_dc ?? 8} min={0} max={30}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), save_dc: v } }))} />
              </div>
            </section>

            <!-- enchantments -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><BookOpen size={14} /> {$_('character.enchantments')}</h4>

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
              <details class="mt-4">
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
          </section>

          <!-- equipment list -->
          <section class="sheet-block">
            <h4 class="sheet-h">{$_('character.equipment')}</h4>

            {#if (c.sheet?.equipment ?? []).length}
              {@const w = totalWeight(c)}
              {@const cap = carryCapacity(c)}
              {@const over = w > cap}
              <div class="mb-2 text-xs" style="color:#8b6355;">
                {$_('character.equipment_total')}: <b style={over ? 'color:#8b1a1a;' : 'color:#2c1810;'}>{w.toFixed(1)} lb</b>
                / {$_('character.equipment_capacity')}: <b style="color:#2c1810;">{cap} lb</b> (STR × 15)
                {#if over}<span class="ml-2 italic" style="color:#8b1a1a;">{$_('character.equipment_encumbered')}</span>{/if}
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
                    <div class="w-16 shrink-0">
                      <Stepper compact value={it.qty} min={0}
                        onchange={(v) => patchEq(c, it.id, { qty: v })} />
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
                    <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <ClassAutocomplete value={cls.name}
                        onchange={(v) => patchSheet(c, (s) => {
                          const pruned = pruneClassData(s, cls.name, cls.subclass);
                          return { ...pruned, classes: (pruned.classes ?? []).map((x) => x.id === cls.id ? { ...x, name: v, subclass: undefined } : x) };
                        })} />
                      <SubclassAutocomplete value={cls.subclass ?? ''} className={cls.name}
                        onchange={(v) => patchSheet(c, (s) => {
                          const pruned = pruneClassData(s, cls.name, cls.subclass);
                          return { ...pruned, classes: (pruned.classes ?? []).map((x) => x.id === cls.id ? { ...x, subclass: v || undefined } : x) };
                        })} />
                      <div class="lvl-stepper shrink-0">
                        <Stepper compact label={$_('character.level')} value={cls.level} min={1}
                          max={Math.min(20, c.level_total - ((c.sheet?.classes ?? []).filter((x) => x.id !== cls.id).reduce((s, x) => s + (x.level || 0), 0)))}
                          onchange={(v) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, level: v } : x) }))} />
                      </div>
                      <button aria-label={$_('common.remove')} class="text-red-400"
                        onclick={() => patchSheet(c, (s) => {
                          const pruned = pruneClassData(s, cls.name, cls.subclass);
                          return { ...pruned, classes: (pruned.classes ?? []).filter((x) => x.id !== cls.id) };
                        })}>
                        <Trash2 size={12} />
                      </button>
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
                        onclick={() => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).filter((x) => x.id !== r.id) }))}>
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
                          onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).filter((x) => x.id !== f.id) }))}>
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
                            {/if}
                            <div class="feat-cfg-btns">
                              <button class="feat-cfg-cancel" onclick={() => { featConfigFeat = null; featConfigAbility=''; featConfigClass=''; featConfigDamage=''; }}>{$_('common.cancel')}</button>
                              <button class="feat-cfg-take" onclick={() => takeFeat(c, f)}>{$_('character.feat_add')}</button>
                            </div>
                          </div>
                        {:else}
                          <button class="feat-take-btn" onclick={() => {
                            if (f.effects.config_type) { featConfigFeat = f; featConfigAbility=''; featConfigClass=''; featConfigDamage=''; }
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
              <h4 class="sheet-h">{$_('character.attunement')} <span class="text-[10px] font-normal" style="color:#8b6355;">— {$_('character.attunement_hint')}</span></h4>
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
                          onclick={(e) => { e.stopPropagation(); patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).filter((x) => x.id !== it.id) })); }}>
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
          <div class="space-y-6">
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
</section>

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
    gap: 0.25rem;
    border-bottom: 1px solid rgba(139, 105, 20, 0.35);
    margin-bottom: 0.25rem;
  }
  .sheet-tab {
    padding: 0.5rem 1rem;
    font-family: 'Cinzel', serif;
    font-weight: 600;
    letter-spacing: 0.08em;
    font-size: 0.85rem;
    text-transform: uppercase;
    color: #8b6914;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
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
</style>

