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
    /** Skill proficiencies: key → 'none' (default) | 'prof' | 'expert'. */
    skills?: Record<string, 'prof' | 'expert'>;
    senses?: { darkvision?: number; blindsight?: number; truesight?: number; tremorsense?: number; passive_perception_bonus?: number };
    languages?: string;
    proficiencies?: { armor?: string; weapons?: string; tools?: string };
    features?: Array<{ id: string; name: string; source?: string; description?: string; uses?: { current: number; max: number; reset?: 'short' | 'long' | 'none' } }>;
    classes?: Array<{ id: string; name: string; level: number; subclass?: string }>;
    resources?: Array<{ id: string; name: string; current: number; max: number; reset?: 'short' | 'long' | 'none' }>;
    attunement?: Array<{ id: string; name: string; notes?: string }>;
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
  onDestroy(() => offWs?.());

  /**
   * Auto-seed class-default resources when classes change. Runs for the
   * currently-viewed character (owner only — guarded by canEdit / patchSheet).
   * Each template is added at most once per (class, resource) and only when
   * its level-table gives a max > 0.
   *
   * Existing rows are left alone; players can manually edit or remove them.
   */
  let seededFor = $state<string | null>(null);
  $effect(() => {
    const c = list[idx];
    if (!c || !canEdit(c)) return;
    const classes = (c.sheet?.classes ?? []).filter((cl) => cl.name?.trim());
    // stable signature: class-name@level joined — triggers effect when either changes
    const sig = classes.map((cl) => `${cl.name}@${cl.level}`).join('|') + `#${c.id}`;
    if (sig === seededFor) return;
    seededFor = sig;
    const existing = new Set((c.sheet?.resources ?? []).map((r) => r.name.trim().toLowerCase()));
    const toAdd: Array<{ id: string; name: string; current: number; max: number; reset: 'short' | 'long' | 'none' }> = [];
    for (const cl of classes) {
      for (const tpl of templatesForClass(cl.name)) {
        if (tpl.minLevel && cl.level < tpl.minLevel) continue;
        const max = tpl.maxFor(cl.level);
        if (max <= 0) continue;
        if (existing.has(tpl.name.toLowerCase())) continue;
        existing.add(tpl.name.toLowerCase());
        toAdd.push({ id: crypto.randomUUID(), name: tpl.name, current: max, max, reset: tpl.reset });
      }
    }
    if (!toAdd.length) return;
    // schedule patch off-effect to avoid re-entrant $effect warnings
    queueMicrotask(() => patchSheet(c, (s) => ({ ...s, resources: [ ...(s.resources ?? []), ...toAdd ] })));
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
    if (!confirm(`Delete ${c.name}?`)) return;
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
    const mod = abilityMod(c.sheet?.abilities?.[ab]);
    return mod + (c.sheet?.saves?.[ab] ? profBonus(c.level_total) : 0);
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
    if (!confirm('Short rest? Class-specific resources refresh; you may also spend hit dice manually to heal.')) return;
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
    if (!confirm('Long rest? HP → max, half hit dice regained, all slots & resources refreshed.')) return;
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
        next = { ...next, active_effects: [...list, { id: crypto.randomUUID(), spell: s.name, duration: s.duration ?? null, since: new Date().toISOString() }] };
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
      id: crypto.randomUUID(),
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
      id: crypto.randomUUID(),
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
                <ImageUpload value={c.sheet?.avatar_url ?? null} kind="avatar" size={88}
                  onchange={(url) => patchSheet(c, (s) => ({ ...s, avatar_url: url }))} />
              </div>
            {:else if c.sheet?.avatar_url}
              <img src={c.sheet.avatar_url} alt="" class="h-22 w-22 rounded-full object-cover border border-amber-900 shrink-0" />
            {/if}
            <div class="min-w-0 pt-1">
              <div class="flex items-center gap-3 flex-wrap">
                <h3 class="text-2xl font-display font-bold leading-tight">{c.name}</h3>
                {#if !canEdit(c)}
                  <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:rgba(47,96,88,0.25);color:#6fa39a;border:1px solid #2f6058;">
                    read-only
                  </span>
                {/if}
                <span class="lvl-badge" title="Level">
                  <span class="lvl-label">Lv</span>
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
                  {#if c.sheet?.alive === false}<Skull size={12} /> Dead{:else}<Heart size={12} fill="currentColor" /> Alive{/if}
                </span>
                {#if canEdit(c)}
                  <button type="button" title="Inspiration"
                    onclick={() => patchSheet(c, (s) => ({ ...s, inspiration: !s.inspiration }))}
                    class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style={c.sheet?.inspiration
                      ? 'background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;'
                      : 'background:rgba(139,105,20,0.1);color:#8b6914;border:1px solid rgba(139,105,20,0.4);'}>
                    <Star size={12} fill={c.sheet?.inspiration ? 'currentColor' : 'none'} />
                    {c.sheet?.inspiration ? 'Inspired' : 'No insp.'}
                  </button>
                {:else if c.sheet?.inspiration}
                  <span class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px] font-bold uppercase tracking-widest"
                    style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;">
                    <Star size={12} fill="currentColor" /> Inspired
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
                      title="Concentration">
                      <Brain size={12} /> Concentrating: {c.sheet.concentration.spell}
                      {#if canEdit(c)}
                        <button class="ml-1" title="drop concentration"
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
                      <Sparkles size={12} /> Spell: {eff.spell}
                      {#if canEdit(c)}
                        <button class="ml-1" title="end effect" onclick={() => dropEffect(c, eff.id)}>
                          <X size={10} />
                        </button>
                      {/if}
                    </span>
                  {/each}
                </div>
              {/if}
              <div class="mt-2 flex flex-wrap gap-3 text-xs" style="color:#8b6914;">
                <span>Prof <b style="color:#2c1810;">+{profBonus(c.level_total)}</b></span>
                <span>Passive Perception <b style="color:#2c1810;">{passivePerception(c)}</b></span>
                {#if campaign().leveling === 'xp'}
                  <span>XP <b style="color:#2c1810;">{c.sheet?.xp ?? 0}</b></span>
                {:else}
                  <span class="italic">Milestone leveling</span>
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
                onclick={() => shortRest(c)} title="Short rest">
                <Bed size={12} /> short
              </button>
              <button class="inline-flex items-center gap-1 rounded px-2.5 py-1 text-xs"
                style="background:#c9a84c;color:#1a0f08;border:1px solid #4e3909;"
                onclick={() => longRest(c)} title="Long rest">
                <Moon size={12} /> long
              </button>
              <button class="inline-flex items-center gap-1 text-xs text-red-400" onclick={() => remove(c)}>
                <Trash2 size={12} /> delete
              </button>
            {/if}
          </div>
        </header>

        <!-- tab bar -->
        <div class="sheet-tabs">
          <button class="sheet-tab {tab === 'vitals' ? 'active' : ''}" onclick={() => tab = 'vitals'}>Vitals</button>
          <button class="sheet-tab {tab === 'combat' ? 'active' : ''}" onclick={() => tab = 'combat'}>Combat</button>
          <button class="sheet-tab {tab === 'magic'  ? 'active' : ''}" onclick={() => tab = 'magic'}>Magic</button>
          <button class="sheet-tab {tab === 'loot'   ? 'active' : ''}" onclick={() => tab = 'loot'}>Equipment</button>
          <button class="sheet-tab {tab === 'features' ? 'active' : ''}" onclick={() => tab = 'features'}>Features</button>
          <button class="sheet-tab {tab === 'story'  ? 'active' : ''}" onclick={() => tab = 'story'}>Background</button>
        </div>

        {#if tab === 'vitals'}
        <!-- vitals block -->
        <section class="sheet-block">
          <h4 class="sheet-h">Vitals</h4>
          <div class="grid grid-cols-3 gap-4">
            <Stepper label="HP (current)" value={hp.current ?? 0} min={0} max={hp.max ?? 999}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, current: v } }))} />
            <Stepper label="HP (max)" value={hp.max ?? 0} min={0}
              onchange={(v) => patchSheet(c, (s) => ({ ...s, hp: { ...s.hp, max: v, current: Math.min(s.hp?.current ?? 0, v) } }))} />
            <Stepper label="Temp HP" value={hp.temp ?? 0} min={0}
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
                  title="Temporary HP"></div>
              {/if}
            </div>
            <div class="mt-1 text-xs flex items-center gap-2" style="color:#8b6355;">
              <span>{cur}/{mx}</span>
              {#if tmp > 0}
                <span class="inline-flex items-center gap-1 rounded px-1.5 py-0.5 text-[10px] font-bold"
                  style="background:rgba(74,127,118,0.25); color:#2f6058; border:1px solid #2f6058;">
                  +{tmp} temp
                </span>
                <span>→ {cur + tmp} effective</span>
              {/if}
            </div>
          {/if}
        </section>

        <div class="space-y-8">
          <!-- LEFT → now full-width under vitals tab -->
          <div class="space-y-8">
            <section class="sheet-block">
              <h4 class="sheet-h">Hit dice</h4>
              <div class="grid grid-cols-3 gap-4">
                <Stepper label="Current" value={hd.current ?? 0} min={0} max={hd.max ?? 20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, current: v } }))} />
                <Stepper label="Max" value={hd.max ?? 0} min={0} max={20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, max: v } }))} />
                <label class="flex flex-col">
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">Type</span>
                  <select value={hd.die ?? 'd8'}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, hit_dice: { ...s.hit_dice, die: (e.currentTarget as HTMLSelectElement).value } }))}
                    class="mt-0.5 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    {#each ['d6','d8','d10','d12'] as d (d)}<option>{d}</option>{/each}
                  </select>
                </label>
              </div>
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h">Status</h4>
              <div class="grid grid-cols-2 gap-4">
                <Stepper label="Exhaustion" value={c.sheet?.exhaustion ?? 0} min={0} max={6}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, exhaustion: v }))} />
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-2" style="color:#8b6914;">Death saves</div>
                  <div class="flex items-center gap-3">
                    <span class="inline-flex items-center gap-1.5" title="Successes">
                      <span class="text-[10px] uppercase font-display font-bold" style="color:#6b8a4f;">S</span>
                      {#each [0,1,2] as i (i)}
                        <button type="button" aria-label={`Death save success ${i+1}`}
                          onclick={() => setDeathSave(c, 'successes', i)}
                          class="ds-dot"
                          style={`border-color:#6b8a4f; background:${i < (c.sheet?.death_saves?.successes ?? 0) ? 'radial-gradient(circle at 35% 30%, #a8c88f 0%, #6b8a4f 60%, #3a5226 100%)' : 'transparent'};`}></button>
                      {/each}
                    </span>
                    <span class="inline-flex items-center gap-1.5" title="Failures">
                      <span class="text-[10px] uppercase font-display font-bold" style="color:#a93535;">F</span>
                      {#each [0,1,2] as i (i)}
                        <button type="button" aria-label={`Death save failure ${i+1}`}
                          onclick={() => setDeathSave(c, 'failures', i)}
                          class="ds-dot"
                          style={`border-color:#a93535; background:${i < (c.sheet?.death_saves?.failures ?? 0) ? 'radial-gradient(circle at 35% 30%, #d47a7a 0%, #a93535 60%, #4e0a0a 100%)' : 'transparent'};`}></button>
                      {/each}
                    </span>
                    {#if (c.sheet?.death_saves?.successes ?? 0) > 0 || (c.sheet?.death_saves?.failures ?? 0) > 0}
                      <button type="button" class="text-[11px] underline ml-auto" style="color:#8b6914;"
                        onclick={() => patchSheet(c, (s) => ({ ...s, death_saves: { successes: 0, failures: 0 } }))}>reset</button>
                    {/if}
                  </div>
                </div>
              </div>
            </section>

            <!-- senses + languages + proficiencies -->
            <section class="sheet-block">
              <h4 class="sheet-h">Senses & Languages</h4>
              <div class="grid sm:grid-cols-2 gap-3">
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">Senses (ft)</div>
                  <div class="grid grid-cols-2 gap-2 text-sm">
                    {#each [['darkvision','Darkvision'],['blindsight','Blindsight'],['truesight','Truesight'],['tremorsense','Tremorsense']] as [k, label] (k)}
                      <label class="flex items-center justify-between gap-2">
                        <span class="text-xs" style="color:#8b6914;">{label}</span>
                        <input type="number" min="0" step="5"
                          value={(c.sheet?.senses as Record<string, number | undefined> | undefined)?.[k] ?? 0}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, senses: { ...(s.senses ?? {}), [k]: +(e.currentTarget as HTMLInputElement).value } }))}
                          class="w-16 text-center text-sm" />
                      </label>
                    {/each}
                    <label class="flex items-center justify-between gap-2 col-span-2">
                      <span class="text-xs" style="color:#8b6914;">Passive Perc bonus</span>
                      <input type="number" value={c.sheet?.senses?.passive_perception_bonus ?? 0}
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, senses: { ...(s.senses ?? {}), passive_perception_bonus: +(e.currentTarget as HTMLInputElement).value } }))}
                        class="w-16 text-center text-sm" />
                    </label>
                  </div>
                  <div class="mt-2 text-xs" style="color:#8b6355;">
                    Passive Perception total: <b style="color:#2c1810;">{passivePerception(c)}</b>
                  </div>
                </div>
                <div>
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">Languages</div>
                  <input type="text" value={c.sheet?.languages ?? ''} placeholder="Common, Elvish…"
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, languages: (e.currentTarget as HTMLInputElement).value }))}
                    class="w-full text-sm" />
                  <div class="text-[11px] uppercase tracking-widest font-display font-semibold mt-3 mb-1" style="color:#8b6914;">Proficiencies</div>
                  {#each [['armor','Armor'],['weapons','Weapons'],['tools','Tools']] as [k, label] (k)}
                    <div class="flex items-center gap-2 mb-1">
                      <span class="w-16 text-xs" style="color:#8b6914;">{label}</span>
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
              <h4 class="sheet-h">Ability scores</h4>
              <div class="grid grid-cols-3 sm:grid-cols-6 gap-2 text-center">
                {#each [['str','STR'],['dex','DEX'],['con','CON'],['int','INT'],['wis','WIS'],['cha','CHA']] as [k, label] (k)}
                  {@const val = (ab as Record<string, number | undefined>)[k] ?? 10}
                  {@const mod = Math.floor((val - 10) / 2)}
                  <div class="rounded-md p-2" style="background:rgba(139,105,20,0.08); border:1px solid rgba(139,105,20,0.3);">
                    <div class="text-[10px] font-display tracking-widest" style="color:#8b6914;">{label}</div>
                    <input type="number" min="1" max="30" value={val}
                      onchange={(e) => patchSheet(c, (s) => ({ ...s, abilities: { ...s.abilities, [k]: +(e.currentTarget as HTMLInputElement).value } }))}
                      class="w-full text-center text-lg font-bold bg-transparent border-0 p-0" />
                    <div class="text-xs" style="color:#8b6355;">{mod >= 0 ? '+' : ''}{mod}</div>
                  </div>
                {/each}
              </div>
            </section>

            <!-- defense / initiative -->
            <section class="sheet-block">
              <h4 class="sheet-h">Defense & Speed</h4>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-3">
                <Stepper label="AC" value={c.sheet?.ac ?? 10} min={0} max={40}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, ac: v }))} />
                <div>
                  <Stepper label="Initiative bonus" value={initBonus} min={-10} max={20}
                    onchange={(v) => patchSheet(c, (s) => ({ ...s, initiative: v }))} />
                  {#if c.sheet?.initiative === undefined}
                    <div class="text-[10px] mt-1" style="color:#8b6355;">derived from DEX ({dexMod >= 0 ? '+' : ''}{dexMod})</div>
                  {:else}
                    <button type="button" class="text-[10px] underline mt-1" style="color:#8b6914;"
                      onclick={() => patchSheet(c, (s) => { const { initiative: _i, ...rest } = s; return rest; })}>
                      reset to DEX
                    </button>
                  {/if}
                </div>
                <Stepper label="Speed (ft)" value={c.sheet?.speed ?? 30} min={0} max={120} step={5}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, speed: v }))} />
              </div>
            </section>

            <!-- saving throws + skills -->
            <section class="sheet-block">
              <h4 class="sheet-h">Saving throws</h4>
              <div class="grid grid-cols-2 sm:grid-cols-3 gap-2">
                {#each ABILITIES as a (a)}
                  {@const sm = saveMod(c, a)}
                  <button type="button"
                    onclick={() => toggleSave(c, a)}
                    class="flex items-center justify-between gap-2 rounded px-2 py-1"
                    style={`background:${c.sheet?.saves?.[a] ? 'rgba(201,168,76,0.25)' : 'rgba(139,105,20,0.06)'}; border:1px solid rgba(139,105,20,0.35);`}
                    title="Toggle proficiency">
                    <span class="inline-flex items-center gap-1.5 text-xs">
                      <span class="h-3 w-3 rounded-full border" style={`border-color:#8b6914; background:${c.sheet?.saves?.[a] ? '#8b6914' : 'transparent'};`}></span>
                      <span class="uppercase tracking-widest font-display" style="color:#8b6914;">{a}</span>
                    </span>
                    <span class="tabular-nums font-bold" style="color:#2c1810;">{sm >= 0 ? '+' : ''}{sm}</span>
                  </button>
                {/each}
              </div>
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h">Skills <span class="text-[10px] font-normal" style="color:#8b6355;">— click cycles none / prof / expertise</span></h4>
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
                      <span>{sk.label}</span>
                      <span class="text-[10px] uppercase" style="color:#8b6914;">{sk.ability}</span>
                    </span>
                    <span class="tabular-nums font-bold" style="color:#2c1810;">{mod >= 0 ? '+' : ''}{mod}</span>
                  </button>
                {/each}
              </div>
            </section>

            <!-- weapons -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Swords size={14} /> Weapons</h4>

              {#if (c.sheet?.weapons ?? []).length}
                <div class="overflow-x-auto">
                  <table class="w-full text-sm">
                    <thead class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">
                      <tr>
                        <th class="text-left py-1">Equip</th>
                        <th class="text-left py-1">Name</th>
                        <th class="py-1">Atk</th>
                        <th class="text-left py-1">Damage</th>
                        <th class="text-left py-1">Range (ft)</th>
                        <th class="text-left py-1">Properties</th>
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
                              {w.equipped ? 'YES' : 'NO'}
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
                            <input type="text" value={w.damage ?? ''} placeholder="1d8+3"
                              onchange={(e) => patchWeapon(c, w.id, { damage: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5" />
                            <input type="text" value={w.damage_type ?? ''} placeholder="slashing"
                              onchange={(e) => patchWeapon(c, w.id, { damage_type: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5 text-xs italic" style="color:#8b6914;" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.range ?? ''} placeholder="melee"
                              onchange={(e) => patchWeapon(c, w.id, { range: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-20 bg-transparent border-0 px-1 py-0.5" />
                          </td>
                          <td class="py-1 pr-2">
                            <input type="text" value={w.properties ?? ''} placeholder="finesse, light"
                              onchange={(e) => patchWeapon(c, w.id, { properties: (e.currentTarget as HTMLInputElement).value || undefined })}
                              class="w-full bg-transparent border-0 px-1 py-0.5 text-xs" />
                          </td>
                          <td class="py-1">
                            <button type="button" aria-label="remove" onclick={() => removeWeapon(c, w.id)}
                              class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                          </td>
                        </tr>
                        <tr style="border:0;">
                          <td></td>
                          <td colspan="6" class="pb-2 pr-2">
                            <textarea value={w.description ?? ''} placeholder="Description / effects (optional)"
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
                <p class="text-sm italic" style="color:#8b6355;">No weapons yet.</p>
              {/if}

              <form onsubmit={(e) => { e.preventDefault(); addWeapon(c); }}
                class="mt-3 grid grid-cols-2 md:grid-cols-6 gap-2 items-end">
                <input required placeholder="Name" bind:value={newWpName}
                  class="col-span-2 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <div>
                  <Stepper compact label="Atk" value={newWpAtk} min={-5} max={20}
                    onchange={(v) => newWpAtk = v} />
                </div>
                <input placeholder="Damage (1d8+3)" bind:value={newWpDmg}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder="Type" bind:value={newWpDmgType}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder="Range (ft)" bind:value={newWpRange}
                  class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <input placeholder="Properties" bind:value={newWpProps}
                  class="col-span-2 md:col-span-4 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                <textarea placeholder="Description / effects (optional)" bind:value={newWpDesc} rows="2"
                  class="col-span-2 md:col-span-6 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
                <button class="col-span-2 md:col-span-6 rounded bg-violet-600 px-3 py-1 text-sm text-white inline-flex items-center justify-center gap-1">
                  <Plus size={14} /> Add weapon
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
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Brain size={14} /> Concentration</h4>
              {#if c.sheet?.concentration?.spell}
                <div class="flex items-center justify-between gap-2">
                  <span class="text-sm"><b>{c.sheet.concentration.spell}</b>
                    {#if c.sheet.concentration.since}
                      <span class="text-xs italic ml-2" style="color:#8b6355;">since {new Date(c.sheet.concentration.since).toLocaleTimeString()}</span>
                    {/if}
                  </span>
                  <button class="rounded px-3 py-1 text-xs" style="background:#8b1a1a;color:#f4e4c1;"
                    onclick={() => patchSheet(c, (s) => ({ ...s, concentration: null }))}>drop</button>
                </div>
              {:else}
                <form onsubmit={(e) => {
                    e.preventDefault();
                    const v = (e.currentTarget.elements.namedItem('sp') as HTMLInputElement)?.value?.trim();
                    if (v) patchSheet(c, (s) => ({ ...s, concentration: { spell: v, since: new Date().toISOString() } }));
                  }}
                  class="flex items-center gap-2">
                  <input name="sp" placeholder="spell name (e.g. Bless)"
                    class="flex-1 text-sm" />
                  <button class="rounded bg-violet-600 px-3 py-1 text-xs text-white">set</button>
                </form>
              {/if}
            </section>

            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Sparkles size={14} /> Spell slots</h4>
              <div class="space-y-2">
                {#each ['1','2','3','4','5','6','7','8','9'] as lvl (lvl)}
                  {@const s = slot(c, lvl)}
                  {#if s.max > 0}
                    <SlotTrack label={`Lvl ${lvl}`} current={s.current} max={s.max}
                      onchange={(cur, mx) => patchSheet(c, (sh) => ({ ...sh, slots: { ...(sh.slots ?? {}), [lvl]: { current: cur, max: mx } } }))} />
                  {/if}
                {/each}
              </div>
              <details class="mt-3">
                <summary class="cursor-pointer text-xs text-neutral-500 hover:text-neutral-300">Add slot level…</summary>
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
              <h4 class="sheet-h inline-flex items-center gap-1.5"><Zap size={14} /> Spellcasting</h4>
              <div class="grid grid-cols-3 gap-4">
                <label class="flex flex-col">
                  <span class="text-[11px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">Ability</span>
                  <select value={c.sheet?.casting?.ability ?? ''}
                    onchange={(e) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), ability: (e.currentTarget as HTMLSelectElement).value || undefined } }))}
                    class="mt-0.5 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                    <option value="">—</option>
                    {#each ['INT','WIS','CHA','STR','DEX','CON'] as a (a)}<option>{a}</option>{/each}
                  </select>
                </label>
                <Stepper label="Attack +" value={c.sheet?.casting?.spell_attack ?? 0} min={-5} max={20}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), spell_attack: v } }))} />
                <Stepper label="Save DC" value={c.sheet?.casting?.save_dc ?? 8} min={0} max={30}
                  onchange={(v) => patchSheet(c, (s) => ({ ...s, casting: { ...(s.casting ?? {}), save_dc: v } }))} />
              </div>
            </section>

            <!-- enchantments -->
            <section class="sheet-block">
              <h4 class="sheet-h inline-flex items-center gap-1.5"><BookOpen size={14} /> Enchantments</h4>

              {#each grouped(c) as [lv, ss] (lv)}
                {@const sl = lv > 0 ? slot(c, String(lv)) : { current: 0, max: 0 }}
                <div class="mb-3">
                  <div class="flex items-center justify-between text-[11px] uppercase tracking-widest font-display" style="color:#8b6914;">
                    <span>{lv === 0 ? 'Cantrips' : `Level ${lv}`}</span>
                    {#if lv > 0}<span class="opacity-70">{sl.current}/{sl.max}</span>{/if}
                  </div>
                  <ul class="mt-1 space-y-1">
                    {#each ss as s (spellKey(s))}
                      <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1">
                        <button type="button" title="Toggle prepared" onclick={() => togglePrepared(c, s)}
                          class="rounded px-1.5 py-0.5 text-[10px] font-bold {s.prepared ? 'bg-amber-500 text-black' : 'bg-neutral-700'}"
                          style={s.prepared ? '' : 'color:#f4e4c1;'}>
                          {s.prepared ? 'PREP' : 'KNOWN'}
                        </button>
                        <button type="button" class="flex-1 text-left text-sm truncate"
                          onclick={() => selectedSpell = s}
                          style="color:#2c1810;" title="View details">
                          {s.name}
                          {#if s.custom}<span class="text-[10px] italic" style="color:#8b6914;"> · custom</span>{/if}
                          {#if s.ritual}<span class="text-[10px]" style="color:#8b6914;"> · ritual</span>{/if}
                          {#if s.concentration}<span class="text-[10px]" style="color:#8b6914;"> · conc</span>{/if}
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
                            class="rounded bg-violet-600 px-2 py-0.5 text-[11px] text-white disabled:opacity-40">Cast</button>
                        {/if}
                        <button type="button" aria-label="remove" onclick={() => removeSpell(c, s)}
                          class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/each}

              {#if !(c.sheet?.spells ?? []).length}
                <p class="text-sm italic" style="color:#8b6355;">No enchantments learned yet.</p>
              {/if}

              <!-- add from book -->
              <details class="mt-4">
                <summary class="cursor-pointer inline-flex items-center gap-1.5 text-sm font-display" style="color:#c9a84c;">
                  <Search size={14} /> Add from book
                </summary>
                <div class="mt-2 space-y-2">
                  <div class="flex gap-2 flex-wrap">
                    <input type="search" placeholder="Search SRD + expansions…"
                      bind:value={bookQuery} oninput={onBookInput}
                      class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                    <select bind:value={bookLevel}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      <option value="">any lvl</option>
                      {#each [0,1,2,3,4,5,6,7,8,9] as l (l)}
                        <option value={l}>{l === 0 ? 'Cantrip' : `Lv ${l}`}</option>
                      {/each}
                    </select>
                    <select bind:value={bookClass}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      <option value="">any class</option>
                      {#each CASTER_CLASSES as cl (cl)}
                        <option value={cl}>{cl}</option>
                      {/each}
                    </select>
                  </div>
                  {#if bookLoading}<p class="text-xs italic" style="color:#8b6355;">Searching…</p>{/if}
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
                              title={tooHigh ? `Not on your class list or above your class level's access` : ''}
                              onclick={() => addSpell(c, { slug: r.slug, name: r.name, level: r.level, school: r.school, classes: r.classes, ritual: r.ritual, concentration: r.concentration, description: r.description, casting_time: r.casting_time, range_text: r.range_text, components: r.components, duration: r.duration, higher_levels: r.higher_levels, source: r.source })}
                              class="rounded bg-violet-600 px-2 py-0.5 text-[11px] text-white disabled:opacity-40">
                              {already ? '✓' : tooHigh ? '🔒' : 'Learn'}
                            </button>
                          </div>
                          {#if isOpen}
                            <div class="px-3 py-2 text-xs space-y-1.5" style="background:rgba(139,105,20,0.06); color:#3a2313;">
                              <div class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">
                                {r.level === 0 ? 'Cantrip' : `Level ${r.level}`} · {r.school}
                                {#if r.classes?.length} · {r.classes.join(', ')}{/if}
                                {#if r.ritual} · ritual{/if}
                                {#if r.concentration} · concentration{/if}
                              </div>
                              {#if r.source}
                                <div class="text-[10px]" style="color:#6d510f;">
                                  <b style="color:#8b6914;">Source:</b> {r.source}
                                </div>
                              {/if}
                              <div class="grid grid-cols-2 sm:grid-cols-4 gap-x-3 gap-y-0.5">
                                {#if r.casting_time}
                                  <div><b style="color:#8b6914;">Cast:</b> {r.casting_time}</div>
                                {/if}
                                {#if r.range_text}
                                  <div><b style="color:#8b6914;">Range:</b> {r.range_text}</div>
                                {/if}
                                {#if r.components}
                                  <div class="col-span-2"><b style="color:#8b6914;">Components:</b> {r.components}</div>
                                {/if}
                                {#if r.duration}
                                  <div><b style="color:#8b6914;">Duration:</b> {r.duration}</div>
                                {/if}
                              </div>
                              <p class="whitespace-pre-wrap">{r.description}</p>
                              {#if r.higher_levels}
                                <p class="whitespace-pre-wrap"><b style="color:#8b6914;">At higher levels:</b> {r.higher_levels}</p>
                              {/if}
                            </div>
                          {/if}
                        </li>
                      {/each}
                    </ul>
                  {:else if bookQuery && !bookLoading}
                    <p class="text-xs italic" style="color:#8b6355;">No results.</p>
                  {/if}
                </div>
              </details>

              <!-- add custom -->
              <details class="mt-3">
                <summary class="cursor-pointer inline-flex items-center gap-1.5 text-sm font-display" style="color:#c9a84c;">
                  <Plus size={14} /> Add custom
                </summary>
                <form onsubmit={(e) => { e.preventDefault(); addCustom(c); }}
                  class="mt-2 space-y-2">
                  <input required placeholder="Name" bind:value={customName}
                    class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
                  <label class="flex items-center gap-2 text-xs" style="color:#8b6914;">
                    Level
                    <select bind:value={customLevel}
                      class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
                      {#each [0,1,2,3,4,5,6,7,8,9] as l (l)}
                        <option value={l}>{l === 0 ? 'Cantrip' : l}</option>
                      {/each}
                    </select>
                  </label>
                  <textarea rows="2" placeholder="Description (optional)" bind:value={customDesc}
                    class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
                  <button class="rounded bg-violet-600 px-3 py-1 text-sm text-white">Add</button>
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
            <h4 class="sheet-h">Equipment</h4>

            {#if (c.sheet?.equipment ?? []).length}
              {@const w = totalWeight(c)}
              {@const cap = carryCapacity(c)}
              {@const over = w > cap}
              <div class="mb-2 text-xs" style="color:#8b6355;">
                Total weight: <b style={over ? 'color:#8b1a1a;' : 'color:#2c1810;'}>{w.toFixed(1)} lb</b>
                / Carry capacity: <b style="color:#2c1810;">{cap} lb</b> (STR × 15)
                {#if over}<span class="ml-2 italic" style="color:#8b1a1a;">encumbered</span>{/if}
              </div>
              <ul class="space-y-1.5">
                {#each c.sheet?.equipment ?? [] as it (it.id)}
                  <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1">
                    <button type="button" title="Toggle equipped"
                      onclick={() => patchEq(c, it.id, { equipped: !it.equipped })}
                      class="rounded px-1.5 py-0.5 text-[10px] font-bold {it.equipped ? 'bg-amber-500 text-black' : 'bg-neutral-700'}"
                      style={it.equipped ? '' : 'color:#f4e4c1;'}>
                      {it.equipped ? 'EQUIP' : 'BAG'}
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
                    <button type="button" aria-label="remove" onclick={() => removeEq(c, it.id)}
                      class="text-red-400 hover:text-red-300"><Trash2 size={12} /></button>
                  </li>
                {/each}
              </ul>
            {:else}
              <p class="text-sm italic" style="color:#8b6355;">Pack's empty. Add some gear below.</p>
            {/if}

            <form onsubmit={(e) => { e.preventDefault(); addEq(c); }}
              class="mt-3 flex flex-wrap gap-2 items-end">
              <input required placeholder="Item name" bind:value={newEqName}
                class="flex-1 min-w-40 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
              <div class="w-20">
                <Stepper compact label="Qty" value={newEqQty} min={1} onchange={(v) => newEqQty = v} />
              </div>
              <label class="flex flex-col">
                <span class="text-[10px] uppercase tracking-widest font-display font-semibold" style="color:#8b6914;">Weight (lb)</span>
                <input type="number" step="0.1" min="0" placeholder="—"
                  bind:value={newEqWeight}
                  class="w-20 rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
              </label>
              <button class="rounded bg-violet-600 px-3 py-1 text-sm text-white inline-flex items-center gap-1">
                <Plus size={14} /> Add
              </button>
            </form>
          </section>
        </div>
        {/if}

        {#if tab === 'features'}
          <div class="space-y-8">
            <!-- XP + classes -->
            <section class="sheet-block">
              <h4 class="sheet-h">Classes &amp; {campaign().leveling === 'xp' ? 'Experience' : 'Progression'}</h4>
              <div class="grid sm:grid-cols-3 gap-3 mb-3">
                {#if campaign().leveling === 'xp'}
                  <label class="flex flex-col">
                    <span class="text-[11px] uppercase tracking-widest font-display font-semibold mb-1" style="color:#8b6914;">XP</span>
                    <input type="number" min="0" value={c.sheet?.xp ?? 0}
                      onchange={(e) => patchSheet(c, (s) => ({ ...s, xp: Math.max(0, +(e.currentTarget as HTMLInputElement).value) }))}
                      class="text-sm" />
                  </label>
                  <div class="sm:col-span-2 text-xs" style="color:#8b6355;">
                    Total level: <b style="color:#2c1810;">{c.level_total}</b> · Proficiency: <b style="color:#2c1810;">+{profBonus(c.level_total)}</b>
                  </div>
                {:else}
                  <div class="sm:col-span-3 text-xs" style="color:#8b6355;">
                    <span class="italic">Milestone leveling — the GM levels you up manually.</span>
                    <div class="mt-1">
                      Total level: <b style="color:#2c1810;">{c.level_total}</b> · Proficiency: <b style="color:#2c1810;">+{profBonus(c.level_total)}</b>
                    </div>
                  </div>
                {/if}
              </div>

              {#if (c.sheet?.classes ?? []).length}
                <ul class="space-y-1.5">
                  {#each c.sheet?.classes ?? [] as cls (cls.id)}
                    <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <ClassAutocomplete value={cls.name}
                        onchange={(v) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, name: v } : x) }))} />
                      <SubclassAutocomplete value={cls.subclass ?? ''} className={cls.name}
                        onchange={(v) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, subclass: v || undefined } : x) }))} />
                      <div class="lvl-stepper shrink-0">
                        <Stepper compact label="Level" value={cls.level} min={1} max={20}
                          onchange={(v) => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).map((x) => x.id === cls.id ? { ...x, level: v } : x) }))} />
                      </div>
                      <button aria-label="remove class" class="text-red-400"
                        onclick={() => patchSheet(c, (s) => ({ ...s, classes: (s.classes ?? []).filter((x) => x.id !== cls.id) }))}>
                        <Trash2 size={12} />
                      </button>
                    </li>
                  {/each}
                </ul>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, classes: [ ...(s.classes ?? []), { id: crypto.randomUUID(), name: '', level: 1 } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> Add class / multiclass
              </button>
            </section>

            <!-- resources -->
            <section class="sheet-block">
              <h4 class="sheet-h">Resources <span class="text-[10px] font-normal" style="color:#8b6355;">— ki, sorcery, superiority, rages…</span></h4>
              {#if (c.sheet?.resources ?? []).length}
                <div class="space-y-2">
                  {#each c.sheet?.resources ?? [] as r (r.id)}
                    <div class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <input type="text" value={r.name} placeholder="Resource name"
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))}
                        class="flex-1 bg-transparent border-0 px-1 py-0.5" />
                      <SlotTrack current={r.current} max={r.max}
                        onchange={(cur, mx) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, current: cur, max: mx } : x) }))} />
                      <label class="flex items-center gap-1.5 text-xs font-display font-bold"
                        style="color:#2c1810;" title="When this resource automatically refills">
                        <span class="px-1.5 py-0.5 rounded"
                          style="background:#8b6914;color:#f4e4c1;letter-spacing:0.12em;text-transform:uppercase;font-size:0.65rem;">Refresh on</span>
                        <select value={r.reset ?? 'long'}
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).map((x) => x.id === r.id ? { ...x, reset: (e.currentTarget as HTMLSelectElement).value as 'short' | 'long' | 'none' } : x) }))}
                          class="text-xs">
                          <option value="short">short rest</option>
                          <option value="long">long rest</option>
                          <option value="none">manual</option>
                        </select>
                      </label>
                      <button aria-label="remove resource" class="text-red-400"
                        onclick={() => patchSheet(c, (s) => ({ ...s, resources: (s.resources ?? []).filter((x) => x.id !== r.id) }))}>
                        <Trash2 size={12} />
                      </button>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">No tracked resources.</p>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, resources: [ ...(s.resources ?? []), { id: crypto.randomUUID(), name: '', current: 0, max: 0, reset: 'long' } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> Add resource
              </button>
            </section>

            <!-- class / racial features -->
            <section class="sheet-block">
              <h4 class="sheet-h">Features & traits</h4>
              {#if (c.sheet?.features ?? []).length}
                <div class="space-y-2">
                  {#each c.sheet?.features ?? [] as f (f.id)}
                    <details class="rounded" style="background:rgba(139,105,20,0.08); border:1px solid rgba(139,105,20,0.3);">
                      <summary class="flex items-center gap-2 px-2 py-1 cursor-pointer text-sm">
                        <span class="font-semibold flex-1">{f.name || 'Unnamed feature'}</span>
                        {#if f.source}<span class="text-xs" style="color:#8b6914;">{f.source}</span>{/if}
                        {#if f.uses}
                          <span class="text-xs tabular-nums" style="color:#8b6355;">{f.uses.current}/{f.uses.max}</span>
                        {/if}
                      </summary>
                      <div class="px-3 py-2 text-sm space-y-2">
                        <div class="grid sm:grid-cols-3 gap-2">
                          <input type="text" value={f.name} placeholder="Name"
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))} />
                          <input type="text" value={f.source ?? ''} placeholder="Source (class, race, feat)"
                            onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, source: (e.currentTarget as HTMLInputElement).value || undefined } : x) }))} />
                          <div class="flex items-center gap-2">
                            {#if f.uses}
                              <SlotTrack current={f.uses.current} max={f.uses.max}
                                onchange={(cur, mx) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { ...(x.uses!), current: cur, max: mx } } : x) }))} />
                              <label class="flex items-center gap-1.5 text-xs font-display font-bold"
                                style="color:#2c1810;" title="When this feature's uses refill">
                                <span class="px-1.5 py-0.5 rounded"
                                  style="background:#8b6914;color:#f4e4c1;letter-spacing:0.12em;text-transform:uppercase;font-size:0.65rem;">Refresh on</span>
                                <select value={f.uses.reset ?? 'long'}
                                  onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { ...(x.uses!), reset: (e.currentTarget as HTMLSelectElement).value as 'short' | 'long' | 'none' } } : x) }))}
                                  class="text-xs">
                                  <option value="short">short rest</option>
                                  <option value="long">long rest</option>
                                  <option value="none">manual</option>
                                </select>
                              </label>
                              <button class="text-[10px] underline" style="color:#8b6914;"
                                onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: undefined } : x) }))}>
                                remove uses
                              </button>
                            {:else}
                              <button class="text-[10px] underline" style="color:#8b6914;"
                                onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, uses: { current: 1, max: 1, reset: 'long' } } : x) }))}>
                                + limited uses
                              </button>
                            {/if}
                          </div>
                        </div>
                        <textarea rows="3" value={f.description ?? ''} placeholder="Description"
                          onchange={(e) => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).map((x) => x.id === f.id ? { ...x, description: (e.currentTarget as HTMLTextAreaElement).value || undefined } : x) }))}
                          class="w-full text-sm"></textarea>
                        <button class="text-xs text-red-400 inline-flex items-center gap-1"
                          onclick={() => patchSheet(c, (s) => ({ ...s, features: (s.features ?? []).filter((x) => x.id !== f.id) }))}>
                          <Trash2 size={12} /> remove feature
                        </button>
                      </div>
                    </details>
                  {/each}
                </div>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">No features yet.</p>
              {/if}
              <button type="button"
                onclick={() => patchSheet(c, (s) => ({ ...s, features: [ ...(s.features ?? []), { id: crypto.randomUUID(), name: '' } ] }))}
                class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                <Plus size={12} /> Add feature
              </button>
            </section>

            <!-- attunement -->
            <section class="sheet-block">
              <h4 class="sheet-h">Attunement <span class="text-[10px] font-normal" style="color:#8b6355;">— max 3 magic items</span></h4>
              {#if (c.sheet?.attunement ?? []).length}
                <ul class="space-y-1">
                  {#each c.sheet?.attunement ?? [] as it, i (it.id)}
                    <li class="flex items-center gap-2 rounded bg-neutral-800/60 px-2 py-1 text-sm">
                      <span class="w-6 text-center text-xs" style="color:#8b6914;">{i + 1}</span>
                      <input type="text" value={it.name} placeholder="Item"
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, name: (e.currentTarget as HTMLInputElement).value } : x) }))}
                        class="flex-1 bg-transparent border-0 px-1 py-0.5" />
                      <input type="text" value={it.notes ?? ''} placeholder="Notes"
                        onchange={(e) => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).map((x) => x.id === it.id ? { ...x, notes: (e.currentTarget as HTMLInputElement).value || undefined } : x) }))}
                        class="flex-1 bg-transparent border-0 px-1 py-0.5 text-xs italic" />
                      <button aria-label="remove" class="text-red-400"
                        onclick={() => patchSheet(c, (s) => ({ ...s, attunement: (s.attunement ?? []).filter((x) => x.id !== it.id) }))}>
                        <Trash2 size={12} />
                      </button>
                    </li>
                  {/each}
                </ul>
              {:else}
                <p class="text-sm italic" style="color:#8b6355;">No attuned items.</p>
              {/if}
              {#if (c.sheet?.attunement ?? []).length < 3}
                <button type="button"
                  onclick={() => patchSheet(c, (s) => ({ ...s, attunement: [ ...(s.attunement ?? []), { id: crypto.randomUUID(), name: '' } ] }))}
                  class="mt-2 inline-flex items-center gap-1 rounded bg-violet-600 px-3 py-1 text-xs text-white">
                  <Plus size={12} /> Attune item
                </button>
              {:else}
                <p class="mt-2 text-xs italic" style="color:#8b1a1a;">Attunement limit reached (3).</p>
              {/if}
            </section>
          </div>
        {/if}

        {#if tab === 'story'}
          {@const bg = c.sheet?.background ?? {}}
          <div class="space-y-6">
            {#each [
              { key: 'backstory',   label: 'Backstory',   rows: 6 },
              { key: 'personality', label: 'Personality', rows: 3 },
              { key: 'ideals',      label: 'Ideals',      rows: 2 },
              { key: 'bonds',       label: 'Bonds',       rows: 2 },
              { key: 'flaws',       label: 'Flaws',       rows: 2 },
              { key: 'notes',       label: 'Notes',       rows: 4 },
            ] as f (f.key)}
              <section class="sheet-block">
                <h4 class="sheet-h">{f.label}</h4>
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
              {selectedSpell.level === 0 ? 'Cantrip' : `Level ${selectedSpell.level}`}
              {#if selectedSpell.school} · {selectedSpell.school}{/if}
              {#if selectedSpell.ritual} · ritual{/if}
              {#if selectedSpell.concentration} · concentration{/if}
            </p>
            {#if selectedSpell.classes?.length}
              <p class="mt-2 text-xs" style="color:#8b6914;">Classes: {selectedSpell.classes.join(', ')}</p>
            {/if}
            {#if selectedSpell.source}
              <p class="mt-1 text-xs" style="color:#6d510f;"><b style="color:#8b6914;">Source:</b> {selectedSpell.source}</p>
            {/if}
            <div class="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
              {#if selectedSpell.casting_time}
                <div><b style="color:#8b6914;">Cast:</b> {selectedSpell.casting_time}</div>
              {/if}
              {#if selectedSpell.range_text}
                <div><b style="color:#8b6914;">Range:</b> {selectedSpell.range_text}</div>
              {/if}
              {#if selectedSpell.components}
                <div class="col-span-2"><b style="color:#8b6914;">Components:</b> {selectedSpell.components}</div>
              {/if}
              {#if selectedSpell.duration}
                <div><b style="color:#8b6914;">Duration:</b> {selectedSpell.duration}</div>
              {/if}
            </div>
            {#if selectedSpell.description}
              <p class="mt-3 whitespace-pre-wrap text-sm">{selectedSpell.description}</p>
            {/if}
            {#if selectedSpell.higher_levels}
              <p class="mt-2 whitespace-pre-wrap text-sm"><b style="color:#8b6914;">At higher levels:</b> {selectedSpell.higher_levels}</p>
            {/if}
            <div class="mt-4 flex justify-end">
              <button onclick={() => (selectedSpell = null)} class="rounded bg-violet-600 px-4 py-1.5 text-sm text-white">Close</button>
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
</style>

