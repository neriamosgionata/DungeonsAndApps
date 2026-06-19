<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { Encounters, Characters, Dice, Effects, Combatants, Overlays, Spells, NPCs } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import { _ } from 'svelte-i18n';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import { Dice5, Play, SkipBack, SkipForward, Square, Crown, Heart, Shield, Swords, Hourglass, X, Trash2, Map as MapIcon, Grid, ListOrdered, Users as UsersIcon, Sparkles, Circle, Triangle, Minus, Wind, Hand, Dices, Brain, Search } from '@lucide/svelte';
  import type { Encounter, Combatant, Character, CombatantEffect, EncounterOverlay } from '$lib/types';
  import { racialAbilityBonus } from '$lib/dnd/racialBonuses';
  import EffectBadge from '$lib/components/EffectBadge.svelte';
  import EffectPanel from '$lib/components/EffectPanel.svelte';
  import NpcStatBlock from '$lib/components/NpcStatBlock.svelte';
  import MyRolls from '$lib/combat/MyRolls.svelte';
  import CombatLog from '$lib/combat/CombatLog.svelte';
  import DiceRoller from '$lib/combat/DiceRoller.svelte';
  import EncounterTabs from '$lib/combat/EncounterTabs.svelte';
  import ReactionNotice from '$lib/combat/ReactionNotice.svelte';
  import Modal from '$lib/combat/Modal.svelte';
  import Banner from '$lib/combat/Banner.svelte';
  import Roster from '$lib/combat/Roster.svelte';

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);
  let encs = $state<Encounter[]>([]);
  let selectedId = $state<string | null>(null);
  let combatants = $state<Combatant[]>([]);
  let error = $state('');
  let loading = $state(true);

  let newName = $state('');
  let newComb = $state({ display_name: '', initiative: 10, hp_max: 10, hp_current: 10, ac: 10 });
  let newCombNpcId = $state<string | null>(null);
  let partyChars = $state<Character[]>([]);
  let allNpcs = $state<Array<{ id: string; name: string; stats?: Record<string, unknown> }>>([]);
  let rolling = $state<Record<string, boolean>>({});
  // In-flight guard for combat action buttons (prevents double-click double-action).
  // Per AGENTS.md §6.4: H8 — every button that fires HTTP/WS should disable while pending.
  let actionInFlight = $state(new Set<string>());

  function isInFlight(key: string): boolean { return actionInFlight.has(key); }

  async function guarded(key: string, fn: () => Promise<unknown>) {
    if (actionInFlight.has(key)) return;
    const next = new Set(actionInFlight);
    next.add(key);
    actionInFlight = next;
    try { await fn(); }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally {
      const after = new Set(actionInFlight);
      after.delete(key);
      actionInFlight = after;
    }
  }
  let effects = $state<CombatantEffect[]>([]);
  let effectPanelCombatant = $state<Combatant | null>(null);
  let statBlockCombatant = $state<Combatant | null>(null);
  let overlays = $state<EncounterOverlay[]>([]);
  let showOverlays = $state(true);
  let placingOverlay = $state<EncounterOverlay | null>(null);
  let overlayStart = $state<{ x: number; y: number } | null>(null);
  let overlayEnd = $state<{ x: number; y: number } | null>(null);

  // combat action state
  let attackTarget = $state<string>('');
  let attackExpr = $state('');
  let damageExpr = $state('');
  let damageType = $state('slashing');
  let attackAdv = $state(false);
  let attackDis = $state(false);
  let coverType = $state('none');
  let attackWeaponId = $state('');
  let extraDamageExpr = $state('');
  let extraDamageType = $state('piercing');
  let powerAttack = $state(false);
  let recklessAttack = $state(false);
  let skipAmmo = $state(false);
  let blessDice = $state<number>(0);
  let bardicInspirationDie = $state<number>(0);
  let attackResult = $state<import('$lib/types').AttackResult | null>(null);
  let showAttackForm = $state(false);

  let dmgAmount = $state(0);
  let dmgType = $state('slashing');
  let dmgResult = $state<import('$lib/types').DamageResult | null>(null);
  let showDmgForm = $state(false);

  let saveAbility = $state('dex');
  let saveDc = $state(15);
  let saveAdv = $state(false);
  let saveDis = $state(false);
  let saveResult = $state<import('$lib/types').SaveResult | null>(null);
  let showSaveForm = $state(false);
  let activeComputedStats = $state<import('$lib/types').ComputedStats | null>(null);

  // cast spell state
  let castSpellSlug = $state('');
  let castTargets = $state<string[]>([]);
  let castDamageExpr = $state('');
  let castHalfOnSave = $state(true);
  let castUpcastLevel = $state<number | ''>('');
  let castSaveDc = $state<number | ''>('');
  let castAsRitual = $state(false);
  let castUseSpellAttack = $state(false);
  let castResult = $state<{ spell_name: string; targets: Array<{ target_id: string; target_name: string; hit?: boolean | null; critical: boolean; attack_total?: number | null; damage_applied: number; save_passed?: boolean | null; concentration_broken: boolean }> } | null>(null);
  let showCastForm = $state(false);
  let allSpells = $state<import('$lib/types').Spell[]>([]);
  let castSpellFilter = $state('');

  // opportunity attack state
  let oppAttackPrompt = $state<Array<{ attacker_id: string; attacker_name: string; target_id: string }>>([]);

  // encounter difficulty
  let encounterDifficulty = $state<{ total_xp: number; adjusted_xp: number; difficulty: string; thresholds: { easy: number; medium: number; hard: number; deadly: number }; party_levels: number[]; monster_xp: Array<[string, number, number]> } | null>(null);

  // combat log state
  let showCombatLog = $state(false);
  let combatEvents = $state<Array<{ id: string; encounter_id: string; round: number; actor_combatant: string | null; target_combatant: string | null; action: string; delta_hp: number | null; note: string | null; created_at: string }>>([]);
  let combatEventsLoading = $state(false);

  // grapple/shove state
  let grappleTarget = $state('');
  let grappleResult = $state<import('$lib/types').GrappleResult | null>(null);
  let shoveTarget = $state('');
  let shoveKnockProne = $state(true);
  let shoveResult = $state<import('$lib/types').ShoveResult | null>(null);
  let showGrappleForm = $state(false);
  let showShoveForm = $state(false);

  // help state
  let showHelpForm = $state(false);
  let helpTarget = $state('');

  // grapple escape state
  let escapeGrapplerId = $state('');
  let escapeResult = $state<import('$lib/types').GrappleEscapeResult | null>(null);
  let showEscapeForm = $state(false);

  // skill check state
  let skillName = $state('perception');
  let skillDc = $state(15);
  let skillAdv = $state(false);
  let skillDis = $state(false);
  let skillResult = $state<import('$lib/types').SkillCheckResult | null>(null);
  let showSkillForm = $state(false);

  // death save state
  let deathSaveResult = $state<import('$lib/types').DeathSaveResult | null>(null);

  // ready state
  let readyTrigger = $state('');
  let readyAction = $state('attack');
  let readyTriggerEvent = $state('');
  let readyWatchTarget = $state('');
  let showReadyForm = $state(false);

  // class feature state
  let classFeatureResult = $state<import('$lib/types').ClassFeatureResult | null>(null);

  // multiattack state
  let showMultiattackForm = $state(false);
  let multiattackTargets = $state<Array<{ target_id: string; attack_expr: string; damage_expr: string; damage_type: string; weapon_id?: string }>>([]);
  let multiattackResult = $state<import('$lib/types').MultiAttackResult | null>(null);
  let multiattackParseTarget = $state('');

  // overlay damage state
  let showOverlayDmgForm = $state(false);
  let overlayDmgId = $state('');
  let overlayDmgExpr = $state('');
  let overlayDmgType = $state('fire');
  let overlaySaveAbility = $state('dex');
  let overlaySaveDc = $state<number | ''>('');
  let overlayHalfOnSave = $state(true);
  let overlayDmgResult = $state<import('$lib/types').OverlayDamageResult | null>(null);
  // Hazard overlay creation
  let hazardDmgExpr = $state('1d6');
  let hazardDmgType = $state('fire');
  let hazardSaveAbility = $state('');
  let hazardSaveDc = $state<number | ''>('');
  let hazardHalfOnSave = $state(false);

  // surprise round state
  let showSurpriseForm = $state(false);
  let surprisedCombatantIds = $state<string[]>([]);
  let surpriseAutoResult = $state<{ stealth_rolls: Array<{ combatant_id: string; name: string; stealth_total: number; natural: number }>; perceptions: Array<{ combatant_id: string; name: string; passive_perception: number; surprised: boolean }> } | null>(null);

  // react state
  let showReactForm = $state(false);
  let reactType = $state('shield');
  let reactLabel = $state('');
  let reactionWindowNotice = $state<{ type: string; message: string } | null>(null);

  // context menu state: combatant override for forms
  let ctxMenu = $state<{ x: number; y: number; combatant: Combatant } | null>(null);
  let formCombatant = $state<string | null>(null); // overrides activeC for forms when set

  // dice roller state
  let showDicePanel = $state(false);
  let diceExpr = $state('');
  let diceCount = $state(1);
  let rosterSearch = $state('');

  const rosterCombs = $derived(combatants.filter((c) => {
    const q = rosterSearch.trim().toLowerCase();
    return !q || c.display_name.toLowerCase().includes(q);
  }));
  let diceLabel = $state('');
  let diceHistory = $state<Array<import('$lib/types').DiceRollResult | import('$lib/types').DiceHistory>>([]);
  let diceHistoryOpen = $state(false);

  // audio state
  let audioEnabled = $state(false);

  function playTone(freq: number, duration: number, type: OscillatorType = 'sine') {
    if (!audioEnabled) return;
    try {
      const ctx = new AudioContext();
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = type;
      osc.frequency.setValueAtTime(freq, ctx.currentTime);
      gain.gain.setValueAtTime(0.1, ctx.currentTime);
      gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + duration);
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.start();
      osc.stop(ctx.currentTime + duration);
    } catch { /* ignore audio errors */ }
  }

  // flanking state
  let flankingPairs = $state<import('$lib/types').FlankPair[]>([]);

  // cover state
  let coverResult = $state<import('$lib/types').CoverResult | null>(null);

  let view = $state<'roster' | 'map'>('roster');
  let mapEl: HTMLDivElement | undefined = $state();
  let mapW = $state(0);
  let mapH = $state(0);
  let dragId = $state<string | null>(null);
  let dragOffset = { dx: 0, dy: 0 };
  let dragStartPct = $state<{ x: number; y: number } | null>(null);
  let dragCurrentPct = $state<{ x: number; y: number } | null>(null);

  // Keep mapW/mapH reactive to resize
  $effect(() => {
    if (!mapEl) return;
    const update = () => {
      if (!mapEl) return;
      const r = mapEl.getBoundingClientRect();
      mapW = r.width;
      mapH = r.height;
    };
    update();
    const ro = new ResizeObserver(update);
    ro.observe(mapEl);
    return () => ro.disconnect();
  });

  function abilityModForChar(c: Character, ab: string): number {
    const abilities = (c.sheet?.abilities ?? {}) as Record<string, number>;
    const overrides = (c.sheet?.abilities_override ?? {}) as Record<string, number>;
    if (overrides[ab] != null) return Math.floor(((overrides[ab] - 10) / 2));
    const score = abilities[ab] ?? 10;
    const racial = racialAbilityBonus(c.race, ab);
    return Math.floor(((score + racial - 10) / 2));
  }
  function profBonus(level: number): number {
    return 2 + Math.floor((Math.max(1, level) - 1) / 4);
  }

  // Auto-fill attack/damage expressions when weapon is selected
  let prevWeaponId = $state('');
  $effect(() => {
    if (!attackWeaponId || attackWeaponId === prevWeaponId) return;
    prevWeaponId = attackWeaponId;
    const currentEncLoop = encs.find((e) => e.id === selectedId);
    const rolledLoop = combatants.filter((c) => c.initiative_rolled);
    const activeCLoop = currentEncLoop?.status === 'active' && rolledLoop.length > 0
      ? rolledLoop[((currentEncLoop.turn_index as number) ?? 0) % rolledLoop.length]
      : null;
    if (!activeCLoop?.character_id) return;
    const activeChar = partyChars.find((p) => p.id === activeCLoop.character_id);
    if (!activeChar) return;
    const weapons = (activeChar.sheet?.weapons ?? []) as Array<{ id: string; name: string; attack_bonus?: number; damage?: string; damage_die?: string; damage_type?: string; properties?: string; range?: string }>;
    const w = weapons.find((x) => x.id === attackWeaponId);
    if (!w) return;
    const props = (w.properties ?? '').toLowerCase();
    const isFinesse = props.includes('finesse');
    const isRanged = props.includes('ranged') || (w.range && !w.range.toLowerCase().includes('melee') && w.range !== '');
    const strMod = abilityModForChar(activeChar, 'str');
    const dexMod = abilityModForChar(activeChar, 'dex');
    const abilityModForAtk = isFinesse ? Math.max(strMod, dexMod) : isRanged ? dexMod : strMod;
    const pb = profBonus(activeChar.level_total);
    const styles: string[] = (activeChar.sheet?.fighting_styles as string[] | undefined) ?? [];
    const archeryBonus = isRanged && styles.some((s) => s.toLowerCase() === 'archery') ? 2 : 0;
    const weaponAtkBonus = w.attack_bonus ?? 0;
    // Auto-fill attack expression if not manually edited
    if (!attackExpr || prevWeaponId !== attackWeaponId) {
      const total = abilityModForAtk + pb + archeryBonus + weaponAtkBonus;
      attackExpr = `1d20${total >= 0 ? '+' : ''}${total}`;
    }
    // Auto-fill damage expression
    const die = w.damage_die || w.damage || '1d4';
    const duelingBonus = !isRanged && !props.includes('two-handed') && styles.some((s) => s.toLowerCase() === 'dueling') ? 2 : 0;
    const abilityModForDmg = isFinesse ? Math.max(strMod, dexMod) : isRanged ? dexMod : strMod;
    const totalDmgMod = abilityModForDmg + duelingBonus;
    const dmgExpr = totalDmgMod !== 0 ? `${die}+${totalDmgMod}` : die;
    if (!damageExpr || prevWeaponId !== attackWeaponId) {
      damageExpr = dmgExpr;
    }
    if (w.damage_type) damageType = w.damage_type;
  });

  // Load computed stats when active combatant changes
  $effect(() => {
    if (currentEnc?.status === 'active' && rolledCombs.length > 0) {
      const ac = rolledCombs[currentEnc.turn_index as number];
      if (ac) { loadComputedStats(ac); }
    }
  });

  async function loadList() {
    try {
      encs = await Encounters.list(cid);
      if (!selectedId && encs.length) selectedId = encs[0].id as string;
      if (selectedId) {
        combatants = await Encounters.combatants.list(selectedId);
        await loadEffects();
        await loadOverlays();
      }
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  async function loadOverlays() {
    if (!selectedId) { overlays = []; return; }
    try { overlays = await Overlays.list(selectedId); }
    catch { overlays = []; }
  }

  async function loadEffects() {
    if (!selectedId) { effects = []; return; }
    try { effects = await Effects.forEncounter(selectedId); }
    catch { effects = []; }
  }

  function effectsFor(c: Combatant): CombatantEffect[] {
    return effects.filter((e) => e.combatant_id === c.id).sort((a, b) => a.name.localeCompare(b.name));
  }

  async function loadParty() {
    try { partyChars = await Characters.list(cid); } catch { partyChars = []; }
  }

  async function loadNpcs() {
    try { allNpcs = await NPCs.list(cid) as Array<{ id: string; name: string; stats?: Record<string, unknown> }>; }
    catch { allNpcs = []; }
  }

  async function loadSpells() {
    try { allSpells = await Spells.list(); } catch { allSpells = []; }
  }

  onMount(loadList);
  onMount(loadParty);
  onMount(loadNpcs);
  onMount(loadSpells);

  const pendingCombatants = $derived(combatants.filter((c) => c.ref_type === 'character' && !c.initiative_rolled));
  const myPending = $derived(pendingCombatants.filter((c) => {
    const ch = partyChars.find((p) => p.id === c.character_id);
    return ch && ch.owner_id === auth.user?.id;
  }));

  let off: (() => void) | undefined;
  onMount(() => {
    off = campaignSocket.on((ev) => {
      const t = ev.type as string;
      // Token moves: patch local state in place to avoid reload flicker during drag.
      if (t === 'combatant_moves') {
        const id = (ev as Record<string, unknown>).id as string;
        const nx = (ev as Record<string, unknown>).x as number;
        const ny = (ev as Record<string, unknown>).y as number;
        const movedRound = (ev as Record<string, unknown>).token_moved_round as number | undefined;
        if (id !== dragId) {
          combatants = combatants.map((c) => c.id === id ? { ...c, token_x: nx, token_y: ny, token_on_map: true, token_moved_round: movedRound ?? c.token_moved_round } : c);
        }
        return;
      }
      if (t.startsWith('combatant_') || t === 'next_turn' || t === 'encounter_starts' || t === 'encounter_ends' || t === 'encounter_updates' || t === 'encounter_deletes' || t === 'encounter_creates' || t === 'lair_action' || t === 'surprise_rounds' || t === 'surprise_auto' || t === 'overlay_damages') {
        loadList();
      }
      if (t === 'effects_change') {
        loadEffects();
      }
      if (t === 'overlay_adds' || t === 'overlay_removes' || t === 'overlays_expire') {
        loadOverlays();
      }
      // Audio cues
      if (t === 'next_turn') {
        const turnIdx = (ev as Record<string, unknown>).turn_index as number;
        const round = (ev as Record<string, unknown>).round as number;
        // Check if it's our turn
        const activeComb = combatants.find((c, i) => i === turnIdx);
        if (activeComb) {
          const ch = partyChars.find((p) => p.id === activeComb.character_id);
          if (ch && ch.owner_id === auth.user?.id) {
            playTone(523, 0.15, 'sine'); // C5 — your turn!
            setTimeout(() => playTone(659, 0.15, 'sine'), 150); // E5
          } else {
            playTone(330, 0.1, 'triangle'); // E4 — next turn
          }
        }
      }
      if (t === 'combatant_attacks') {
        const hit = (ev as Record<string, unknown>).hit as boolean;
        const crit = (ev as Record<string, unknown>).critical as boolean;
        if (crit) { playTone(880, 0.2, 'square'); playTone(1100, 0.2, 'square'); }
        else if (hit) { playTone(440, 0.1, 'square'); }
        else { playTone(220, 0.15, 'sawtooth'); }
      }
      if (t === 'reaction_window') {
        const wtype = (ev as Record<string, unknown>).window_type as string;
        if (wtype === 'hit_before_damage') {
          const targetId = (ev as Record<string, unknown>).target_id as string;
          const myChars = partyChars.filter(p => p.owner_id === auth.user?.id);
          const myIds = combatants.filter(c => myChars.some(p => p.id === c.character_id)).map(c => c.id);
          if (myIds.includes(targetId)) {
            reactionWindowNotice = { type: 'shield', message: `You were hit! Use Shield reaction?` };
            setTimeout(() => reactionWindowNotice = null, 8000);
          }
        }
        if (wtype === 'spell_being_cast') {
          reactionWindowNotice = { type: 'counterspell', message: `Spell being cast — Counterspell available!` };
          setTimeout(() => reactionWindowNotice = null, 5000);
        }
        loadList();
      }
      if (t === 'combatant_triggers_readied_action') {
        loadList();
      }
    });
  });
  onDestroy(() => off?.());

  $effect(() => {
    if (selectedId) Encounters.combatants.list(selectedId).then((c) => combatants = c).catch(() => {});
  });

  async function create(close: () => void) {
    try {
      const enc = await Encounters.create(cid, { name: newName });
      selectedId = enc.id as string;
      newName = '';
      close();
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function addCombatant(close: () => void) {
    if (!selectedId) return;
    try {
      await Encounters.combatants.add(selectedId, { ...newComb, ref_type: 'npc', npc_id: newCombNpcId });
      newComb = { display_name: '', initiative: 10, hp_max: 10, hp_current: 10, ac: 10 };
      newCombNpcId = null;
      close();
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  function selectNpcForCombatant(id: string | null) {
    newCombNpcId = id;
    if (!id) {
      newComb = { display_name: '', initiative: 10, hp_max: 10, hp_current: 10, ac: 10 };
      return;
    }
    const npc = allNpcs.find((n) => n.id === id);
    if (!npc) return;
    const stats = npc.stats as Record<string, unknown> | undefined;
    const hp = (stats?.hp as Record<string, unknown> | undefined)?.max as number | undefined;
    newComb = {
      display_name: npc.name,
      initiative: 10,
      hp_max: hp ?? 10,
      hp_current: hp ?? 10,
      ac: (stats?.ac as number | undefined) ?? 10,
    };
  }

  async function rollInitiativeFor(comb: Combatant) {
    if (!selectedId) return;
    const chid = comb.character_id as string;
    const ch = partyChars.find((p) => p.id === chid);
    if (!ch) return;
    const sheet = (ch.sheet ?? {}) as Record<string, unknown>;
    const bonus = initBonus(sheet);
    const expr = bonus >= 0 ? `1d20+${bonus}` : `1d20${bonus}`;
    rolling[chid] = true;
    try {
      const roll = await Dice.roll(cid, expr, `Initiative — ${ch.name as string}`, false, chid);
      await Encounters.setInitiative(selectedId, chid, roll.total);
      await loadList();
    } catch (e) { error = (e as Error).message; }
    finally { rolling[chid] = false; }
  }

  async function start() {
    const id = selectedId; if (!id) return;
    await guarded('encounter:start', async () => { await Encounters.start(id); await loadList(); });
  }
  async function end()   {
    const id = selectedId; if (!id) return;
    const e = encs.find((x) => x.id === id);
    if (!e) return;
    if (!confirm($_('initiative.end_encounter_confirm').replace('{{name}}', e.name))) return;
    await guarded('encounter:end', async () => { await Encounters.end(id); await loadList(); });
  }
  async function next()  {
    const id = selectedId; if (!id) return;
    await guarded('encounter:next', async () => { await Encounters.nextTurn(id); await loadList(); });
  }
  async function prev()  {
    const id = selectedId; if (!id) return;
    await guarded('encounter:prev', async () => { await Encounters.prevTurn(id); await loadList(); });
  }
  async function gotoTurn(idx: number) {
    const id = selectedId; if (!id) return;
    await guarded(`encounter:goto:${idx}`, async () => { await Encounters.gotoTurn(id, idx); await loadList(); });
  }

  function initBonus(sheet: Record<string, unknown>): number {
    const explicit = sheet.initiative as number | undefined;
    if (typeof explicit === 'number') return explicit;
    const ab = (sheet.abilities ?? {}) as Record<string, number | undefined>;
    const dex = ab.dex ?? 10;
    return Math.floor((dex - 10) / 2);
  }

  async function applyDamage(c: Combatant, delta: number) {
    let temp = (c.temp_hp as number | undefined) ?? 0;
    let hp   = c.hp_current as number;
    const mx = c.hp_max as number;
    if (delta < 0) {
      let dmg = -delta;
      const absorb = Math.min(temp, dmg);
      temp -= absorb; dmg -= absorb;
      hp = Math.max(0, hp - dmg);
    } else {
      hp = Math.min(mx, hp + delta);
    }
    try {
      await Encounters.combatants.update(c.id as string, { hp_current: hp, temp_hp: temp });
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }
  async function setTemp(c: Combatant, v: number) {
    try {
      await Encounters.combatants.update(c.id as string, { temp_hp: Math.max(0, v) });
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function removeEncounter() {
    if (!selectedId) return;
    const enc = encs.find((e) => e.id === selectedId);
    const name = (enc?.name as string) ?? 'encounter';
    if (!confirm($_('initiative.delete_confirm').replace('{{name}}', name))) return;
    try {
      await Encounters.delete(selectedId);
      selectedId = null;
      combatants = [];
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  const currentEnc = $derived(encs.find((e) => e.id === selectedId));
  const rolledCombs = $derived(combatants.filter((c) => c.initiative_rolled));
  const waitingCount = $derived(combatants.length - rolledCombs.length);
  const activeCtxCombatant = $derived(
    combatants.find((c) => c.id === formCombatant)
    ?? rolledCombs[currentEnc?.turn_index ?? -1]
    ?? combatants[0]
  );

  // ---- grid snap ----
  function snapToSquare(x: number, y: number, gridPx: number, mapW: number, mapH: number): { x: number; y: number } {
    // Work in px space. CSS background-size grid uses gridPx × gridPx cells anchored at (0,0).
    // Cell containing px is k = floor(px / gridPx); cell center is (k + 0.5) * gridPx.
    const px = (x / 100) * mapW;
    const py = (y / 100) * mapH;
    const bx = (Math.floor(px / gridPx) + 0.5) * gridPx;
    const by = (Math.floor(py / gridPx) + 0.5) * gridPx;
    return {
      x: Math.max(0, Math.min(100, (bx / mapW) * 100)),
      y: Math.max(0, Math.min(100, (by / mapH) * 100)),
    };
  }

  function snapToHex(x: number, y: number, gridPx: number, mapW: number, mapH: number): { x: number; y: number } {
    // Convert input % to px, find nearest hex center in px, convert back.
    // Compare distances in px (not %) because 1% width ≠ 1% height.
    const R = gridPx / 2;
    const colSpacing = 1.5 * R;              // 0.75 * gridPx — horizontal distance between column centers
    const tileH = R * Math.sqrt(3);          // vertical tile repeat

    const px = (x / 100) * mapW;
    const py = (y / 100) * mapH;

    // Candidate column: nearest integer such that cx = R + col * colSpacing is closest to px.
    const colEst = Math.round((px - R) / colSpacing);
    let best = { bx: px, by: py, dist: Infinity };
    for (let dc = -2; dc <= 2; dc++) {
      const col = colEst + dc;
      if (col < 0) continue;
      const bx = R + col * colSpacing;
      // Even columns: row centers at tileH/2, 3*tileH/2, 5*tileH/2, ...
      // Odd columns:  row centers at tileH,   2*tileH,   3*tileH, ...
      const yBase = (col % 2 === 0) ? tileH / 2 : tileH;
      const rowEst = Math.round((py - yBase) / tileH);
      for (let dr = -2; dr <= 2; dr++) {
        const row = rowEst + dr;
        if (row < 0) continue;
        const by = yBase + row * tileH;
        const dist = Math.hypot(px - bx, py - by);
        if (dist < best.dist) best = { bx, by, dist };
      }
    }
    return {
      x: Math.max(0, Math.min(100, (best.bx / mapW) * 100)),
      y: Math.max(0, Math.min(100, (best.by / mapH) * 100)),
    };
  }

  function snapPos(x: number, y: number, enc: Encounter | undefined): { x: number; y: number } {
    if (!enc || !mapEl) return { x, y };
    if (!(enc.show_grid as boolean)) return { x, y };
    const r = mapEl.getBoundingClientRect();
    const g = (enc.map_grid_size as number) ?? 50;
    if ((enc.grid_type as string) === 'hex') return snapToHex(x, y, g, r.width, r.height);
    return snapToSquare(x, y, g, r.width, r.height);
  }

  // ---- movement cap ----
  function charSpeed(c: Combatant): number {
    if (c.ref_type !== 'character') return Infinity;
    const ch = partyChars.find((p) => p.id === c.character_id);
    if (!ch) return 30;
    const sheet = (ch.sheet as Record<string, unknown> | undefined) ?? {};
    return (sheet.speed as number | undefined) ?? 30;
  }

  /** Only forced-movement effects (push/pull/teleport/forced_move) — NOT dash_bonus.
   *  These bypass the normal once-per-round limit and are consumed after use. */
  function forcedMovementFt(c: Combatant): number {
    const effs = effectsFor(c);
    let total = 0;
    for (const e of effs) {
      if (!e.active) continue;
      const m = e.modifiers as Record<string, unknown> | undefined;
      const mov = m?.movement as Record<string, unknown> | undefined;
      const type = mov?.type as string | undefined;
      if (type && type !== 'dash_bonus' && mov?.distance_ft) {
        total += (mov.distance_ft as number) || 0;
      }
    }
    return total;
  }

  function hasMovementEffect(c: Combatant): boolean {
    return forcedMovementFt(c) > 0;
  }

  /** Dash bonuses add to speed but do NOT bypass the once-per-round rule. */
  function dashBonusFt(c: Combatant): number {
    const effs = effectsFor(c);
    let total = 0;
    for (const e of effs) {
      if (!e.active) continue;
      const m = e.modifiers as Record<string, unknown> | undefined;
      const mov = m?.movement as Record<string, unknown> | undefined;
      if (mov?.type === 'dash_bonus' && mov?.distance_ft) {
        total += (mov.distance_ft as number) || 0;
      }
    }
    return total;
  }

  /** Max drag distance in PIXELS given speed (ft), grid size (px).
   *  1 cell = 5 ft of movement. Working in px means the cap is an
   *  accurate Euclidean distance independent of map aspect ratio. */
  function maxMovePx(speedFt: number, gridPx: number): number {
    if (!isFinite(speedFt) || speedFt <= 0) return Infinity;
    const cells = speedFt / 5;
    return cells * gridPx;
  }

  /** Clamp a target point to within maxPx from start, where coordinates
   *  are in percent but the cap is in pixels. Needs map dims to convert. */
  function clampToRange(
    nx: number, ny: number,
    sx: number, sy: number,
    maxPx: number,
    mapW: number, mapH: number,
  ): { x: number; y: number } {
    if (!isFinite(maxPx)) return { x: nx, y: ny };
    const dxPx = ((nx - sx) / 100) * mapW;
    const dyPx = ((ny - sy) / 100) * mapH;
    const d = Math.hypot(dxPx, dyPx);
    if (d <= maxPx) return { x: nx, y: ny };
    const s = maxPx / d;
    return {
      x: sx + (dxPx * s / mapW) * 100,
      y: sy + (dyPx * s / mapH) * 100,
    };
  }

  /** Distance (px) between two % points given map dimensions. */
  function distPx(ax: number, ay: number, bx: number, by: number, mapW: number, mapH: number): number {
    return Math.hypot(((ax - bx) / 100) * mapW, ((ay - by) / 100) * mapH);
  }

  function hasRogueClass(c: Combatant): boolean {
    if (!c.character_id) return false;
    const ch = partyChars.find((p) => p.id === c.character_id);
    if (!ch) return false;
    const classes = (ch.sheet as Record<string, unknown>)?.classes as Array<{ name: string; level?: number }> | undefined;
    return classes?.some((cl) => cl.name?.toLowerCase() === 'rogue') ?? false;
  }

  function canMoveToken(c: Combatant): boolean {
    if (campaign().isMaster) return true;
    if (c.ref_type !== 'character') return false;
    const ch = partyChars.find((p) => p.id === c.character_id);
    if (!ch || ch.owner_id !== auth.user?.id) return false;
    // Before combat starts: free placement anywhere.
    if (currentEnc?.status !== 'active') return true;
    // Combat active: once per round, speed-capped.
    // BUT: forced movement effects (push/pull/teleport) bypass this limit.
    const movedRound = c.token_moved_round as number | null | undefined;
    const currentRound = (currentEnc?.round as number | undefined) ?? 0;
    if (movedRound != null && movedRound >= currentRound) {
      return hasMovementEffect(c);
    }
    return true;
  }

  function tokenMovedThisRound(c: Combatant): boolean {
    if (campaign().isMaster) return false;
    const movedRound = c.token_moved_round as number | null | undefined;
    const currentRound = (currentEnc?.round as number | undefined) ?? 0;
    return movedRound != null && movedRound >= currentRound && !!c.token_on_map;
  }

  function startTokenDrag(ev: PointerEvent, c: Combatant) {
    if (!mapEl || !canMoveToken(c)) return;
    ev.preventDefault();
    ev.stopPropagation();
    dragId = c.id as string;
    const r = mapEl.getBoundingClientRect();
    const startX = (c.token_x as number | null) ?? 50;
    const startY = (c.token_y as number | null) ?? 50;
    const cx = (startX / 100) * r.width + r.left;
    const cy = (startY / 100) * r.height + r.top;
    dragOffset = { dx: ev.clientX - cx, dy: ev.clientY - cy };
    dragStartPct = { x: startX, y: startY };
    dragCurrentPct = { x: startX, y: startY };
    (ev.target as Element).setPointerCapture?.(ev.pointerId);
  }

  function onTokenDragMove(ev: PointerEvent) {
    if (!dragId || !mapEl) return;
    const r = mapEl.getBoundingClientRect();
    let x = Math.max(0, Math.min(100, ((ev.clientX - dragOffset.dx - r.left) / r.width) * 100));
    let y = Math.max(0, Math.min(100, ((ev.clientY - dragOffset.dy - r.top) / r.height) * 100));

    // Clamp to movement cap during drag (smooth, no snap yet — snap happens on drop)
    const c = combatants.find((cb) => cb.id === dragId);
    if (c && dragStartPct && !campaign().isMaster && currentEnc?.status === 'active') {
      const speed = charSpeed(c);
      const forcedFt = forcedMovementFt(c);
      const dashFt = dashBonusFt(c);
      const g = (currentEnc.map_grid_size as number) ?? 50;
      const maxPx = maxMovePx(speed + dashFt + forcedFt, g);
      const clamped = clampToRange(x, y, dragStartPct.x, dragStartPct.y, maxPx, r.width, r.height);
      x = clamped.x; y = clamped.y;
    }

    dragCurrentPct = { x, y };
    combatants = combatants.map((cb) => cb.id === dragId ? { ...cb, token_x: x, token_y: y, token_on_map: true } : cb);
  }

  async function endTokenDrag(ev: PointerEvent) {
    if (!dragId) return;
    const id = dragId;
    const moved = combatants.find((c) => c.id === id);
    const start = dragStartPct;
    dragId = null;
    dragStartPct = null;
    dragCurrentPct = null;
    (ev.target as Element).releasePointerCapture?.(ev.pointerId);
    if (moved && moved.token_x != null && moved.token_y != null) {
      // Snap to grid on drop
      let final = snapPos(moved.token_x as number, moved.token_y as number, currentEnc);
      // If the snapped cell would overshoot the movement cap, fall back to
      // the nearest in-range cell (don't let post-snap push us past max).
      if (mapEl && start && !campaign().isMaster && currentEnc?.status === 'active' && moved.ref_type === 'character') {
        const r = mapEl.getBoundingClientRect();
        const g = (currentEnc.map_grid_size as number) ?? 50;
        const forcedFt = forcedMovementFt(moved);
        const dashFt = dashBonusFt(moved);
        const maxPx = maxMovePx(charSpeed(moved) + dashFt + forcedFt, g);
        if (distPx(final.x, final.y, start.x, start.y, r.width, r.height) > maxPx) {
          const clamped = clampToRange(final.x, final.y, start.x, start.y, maxPx, r.width, r.height);
          final = snapPos(clamped.x, clamped.y, currentEnc);
          // If snapped cell still outside range, bail to start.
          if (distPx(final.x, final.y, start.x, start.y, r.width, r.height) > maxPx) {
            final = { x: start.x, y: start.y };
          }
        }
      }
      combatants = combatants.map((c) => c.id === id ? { ...c, ...final } : c);
      try {
        const r = mapEl?.getBoundingClientRect();
        const g = (currentEnc?.map_grid_size as number) ?? 50;
        let moveCostFt = 0;
        if (r && start) {
          const d = distPx(final.x, final.y, start.x, start.y, r.width, r.height);
          moveCostFt = d / g * 5;
        }
        await Encounters.combatants.move(id, final.x, final.y, moveCostFt || undefined);
        // If forced movement effects were active and the token actually moved,
        // consume them (deactivate). This applies to both master moves (push/pull)
        // and player self-teleport moves.
        if (moved && start) {
          const dx = final.x - start.x;
          const dy = final.y - start.y;
          if (dx !== 0 || dy !== 0) {
            checkOpportunityAttacks(moved, start.x, start.y, final.x, final.y);
            if (hasMovementEffect(moved)) {
              await consumeMovementEffects(moved);
              await loadEffects();
            }
          }
        }
      }
      catch (e) { error = (e as Error).message; await loadList(); }
    }
  }

  async function consumeMovementEffects(c: Combatant) {
    const effs = effectsFor(c).filter((e) => {
      if (!e.active) return false;
      const m = e.modifiers as Record<string, unknown> | undefined;
      const mov = m?.movement as Record<string, unknown> | undefined;
      const type = mov?.type as string | undefined;
      return !!mov && type !== 'dash_bonus';
    });
    for (const eff of effs) {
      try { await Effects.update(c.id as string, eff.id, { active: false }); }
      catch { /* ignore */ }
    }
  }

  // ---- overlay helpers ----
  function ftToPx(ft: number): number {
    const g = (currentEnc?.map_grid_size as number) ?? 50;
    return (ft / 5) * g;
  }

  function ftToPctX(ft: number): number { return mapW > 0 ? (ftToPx(ft) / mapW) * 100 : 0; }
  function ftToPctY(ft: number): number { return mapH > 0 ? (ftToPx(ft) / mapH) * 100 : 0; }

  function renderOverlayShape(o: EncounterOverlay): string {
    // Returns SVG markup for the overlay shape
    const ox = o.origin_x;
    const oy = o.origin_y;
    switch (o.shape) {
      case 'circle': {
        const r = o.radius_ft ? ftToPctX(o.radius_ft) : 5;
        return `<circle cx="${ox}%" cy="${oy}%" r="${r}%" fill="${o.color}" stroke="${o.color.replace(/[\d.]+\)$/, '0.6)')}" stroke-width="1" />`;
      }
      case 'cone': {
        const len = o.length_ft ? ftToPctX(o.length_ft) : 5;
        const angle = (o.angle_deg ?? 0) * (Math.PI / 180);
        const spread = 53.13 * (Math.PI / 180); // 5e cone is ~53.13°
        const p1x = ox;
        const p1y = oy;
        const p2x = ox + len * Math.cos(angle - spread / 2);
        const p2y = oy + len * Math.sin(angle - spread / 2) * (mapW / mapH);
        const p3x = ox + len * Math.cos(angle + spread / 2);
        const p3y = oy + len * Math.sin(angle + spread / 2) * (mapW / mapH);
        return `<polygon points="${p1x},${p1y} ${p2x},${p2y} ${p3x},${p3y}" fill="${o.color}" stroke="${o.color.replace(/[\d.]+\)$/, '0.6)')}" stroke-width="1" />`;
      }
      case 'line': {
        const ex = o.end_x ?? ox;
        const ey = o.end_y ?? oy;
        const w = o.width_ft ? ftToPctX(o.width_ft) : 1;
        // For a line with width, draw a thick stroke
        return `<line x1="${ox}%" y1="${oy}%" x2="${ex}%" y2="${ey}%" stroke="${o.color}" stroke-width="${w}%" stroke-linecap="round" />`;
      }
      case 'cube': {
        const side = o.length_ft ? ftToPctX(o.length_ft) : 5;
        return `<rect x="${ox - side / 2}%" y="${oy - side / 2 * (mapW / mapH)}%" width="${side}%" height="${side * (mapW / mapH)}%" fill="${o.color}" stroke="${o.color.replace(/[\d.]+\)$/, '0.6)')}" stroke-width="1" />`;
      }
      default: return '';
    }
  }

  async function createZoneOverlay(shape: EncounterOverlay['shape'], zoneType: string, color: string) {
    if (!selectedId || !mapEl) return;
    // For simplicity: click center on map, use default size
    const r = mapEl.getBoundingClientRect();
    const cx = 50;
    const cy = 50;
    const defaults: Record<string, { radius_ft?: number; length_ft?: number; width_ft?: number }> = {
      difficult_terrain: { radius_ft: 20 },
      low_visibility: { radius_ft: 30 },
      no_visibility: { radius_ft: 15 },
      magical_darkness: { radius_ft: 15 },
      fire: { radius_ft: 10 },
      ice: { radius_ft: 15 },
      water: { radius_ft: 20 },
      poison: { radius_ft: 10 },
      hazard: { radius_ft: 10 },
    };
    const def = defaults[zoneType] ?? { radius_ft: 15 };

    // Walls: create a horizontal line obstacle
    if (zoneType === 'wall') {
      const len = 15; // % of map width
      try {
        await Overlays.create(selectedId, {
          kind: 'zone', shape: 'line',
          origin_x: cx - len / 2, origin_y: cy,
          end_x: cx + len / 2, end_y: cy,
          color, label: $_(zoneType), zone_type: zoneType,
          width_ft: 2,
        });
        await loadOverlays();
      } catch (e) { error = (e as Error).message; }
      return;
    }
    const hazardFields = zoneType === 'hazard' ? {
      hazard_damage_expression: hazardDmgExpr || '1d6',
      hazard_damage_type: hazardDmgType,
      hazard_save_ability: hazardSaveAbility || undefined,
      hazard_save_dc: hazardSaveDc !== '' ? Number(hazardSaveDc) : undefined,
      hazard_half_on_save: hazardHalfOnSave,
    } : {};
    try {
      await Overlays.create(selectedId, {
        kind: 'zone',
        shape,
        origin_x: cx,
        origin_y: cy,
        color,
        label: $_(zoneType),
        zone_type: zoneType,
        ...def,
        ...hazardFields,
      });
      await loadOverlays();
    } catch (e) { error = (e as Error).message; }
  }

  async function removeOverlay(oid: string) {
    if (!selectedId) return;
    if (!confirm($_('initiative.remove_overlay_confirm'))) return;
    try { await Overlays.delete(selectedId, oid); await loadOverlays(); }
    catch (e) { error = (e as Error).message; }
  }

  // ---- combat actions ----
  async function doAttack(attacker: Combatant) {
    if (!attackTarget) { error = 'Select a target'; return; }
    error = '';
    try {
      const res = await Combatants.attack(attacker.id as string, {
        target_id: attackTarget,
        attack_expression: attackExpr || undefined,
        damage_expression: damageExpr || undefined,
        damage_type: damageType,
        advantage: attackAdv,
        disadvantage: attackDis,
        cover: coverType,
        is_magical: false,
        weapon_id: attackWeaponId || undefined,
        extra_damage_expression: extraDamageExpr || undefined,
        extra_damage_type: extraDamageExpr ? extraDamageType : undefined,
        power_attack: powerAttack || undefined,
        skip_ammo: skipAmmo || undefined,
        reckless: recklessAttack || undefined,
        bless_dice: blessDice > 0 ? blessDice : undefined,
        bardic_inspiration_dice: bardicInspirationDie > 0 ? bardicInspirationDie : undefined,
      });
      attackResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; attackResult = null; }
  }

  async function doDamage(target: Combatant) {
    if (dmgAmount <= 0) { error = 'Enter damage amount'; return; }
    error = '';
    try {
      const res = await Combatants.damage(target.id as string, {
        amount: dmgAmount,
        damage_type: dmgType,
        is_magical: false,
      });
      dmgResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; dmgResult = null; }
  }

  async function doHeal(target: Combatant) {
    if (dmgAmount <= 0) { error = 'Enter healing amount'; return; }
    error = '';
    try {
      const res = await Combatants.heal(target.id as string, {
        amount: dmgAmount,
      });
      dmgResult = {
        damage_raw: -res.amount,
        damage_applied: -res.amount,
        hp_before: res.hp_before,
        hp_after: res.hp_after,
        temp_hp_after: res.temp_hp_after,
        concentration_broken: false,
        concentration_roll: null,
        damage_resisted: false,
        damage_vulnerable: false,
        damage_immune: false,
      } as import('$lib/types').DamageResult;
      await loadList();
    } catch (e) { error = (e as Error).message; dmgResult = null; }
  }

  async function doDeathSave(combatant: Combatant) {
    error = '';
    try {
      const res = await Combatants.deathSave(combatant.id as string);
      deathSaveResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; deathSaveResult = null; }
  }

  async function doSkillCheck(combatant: Combatant) {
    error = '';
    try {
      const res = await Combatants.skillCheck(combatant.id as string, {
        skill: skillName,
        dc: skillDc,
        advantage: skillAdv,
        disadvantage: skillDis,
      });
      skillResult = res;
    } catch (e) { error = (e as Error).message; skillResult = null; }
  }

  async function doSave(combatant: Combatant) {
    error = '';
    try {
      const res = await Combatants.save(combatant.id as string, {
        ability: saveAbility,
        dc: saveDc,
        advantage: saveAdv,
        disadvantage: saveDis,
      });
      saveResult = res;
    } catch (e) { error = (e as Error).message; saveResult = null; }
  }

  async function loadComputedStats(c: Combatant) {
    try { activeComputedStats = await Combatants.computedStats(c.id as string); }
    catch { activeComputedStats = null; }
  }

  async function doGrapple(attacker: Combatant) {
    if (!grappleTarget) { error = 'Select a target'; return; }
    error = '';
    try {
      const res = await Combatants.grapple(attacker.id as string, grappleTarget);
      grappleResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; grappleResult = null; }
  }

  async function doShove(attacker: Combatant) {
    if (!shoveTarget) { error = 'Select a target'; return; }
    error = '';
    try {
      const res = await Combatants.shove(attacker.id as string, shoveTarget, shoveKnockProne);
      shoveResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; shoveResult = null; }
  }

  async function doStandUp(c: Combatant) {
    error = '';
    try {
      await Combatants.standUp(c.id as string);
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function doGrappleEscape(escapee: Combatant) {
    if (!escapeGrapplerId) { error = 'Select your grappler'; return; }
    error = '';
    try {
      const res = await Combatants.grappleEscape(escapee.id as string, escapeGrapplerId);
      escapeResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; escapeResult = null; }
  }

  async function doReady(c: Combatant) {
    if (!readyTrigger) { error = 'Enter trigger condition'; return; }
    error = '';
    try {
      await Combatants.ready(c.id as string, readyTrigger, readyAction,
        attackTarget || undefined, readyTriggerEvent || undefined, readyWatchTarget || undefined);
      showReadyForm = false;
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function doDelay(c: Combatant) {
    error = '';
    try {
      // Insert after current turn index
      const currentIdx = currentEnc?.turn_index ?? 0;
      await Combatants.delay(c.id as string, currentIdx);
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function doTriggerReady(c: Combatant) {
    error = '';
    try {
      await Combatants.triggerReady(c.id as string);
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function doClassFeature(c: Combatant, feature: string, targetId?: string) {
    error = '';
    try {
      const res = await Combatants.classFeature(c.id as string, feature, targetId);
      classFeatureResult = res;
      await loadList();
      setTimeout(() => classFeatureResult = null, 5000);
    } catch (e) { error = (e as Error).message; classFeatureResult = null; }
  }

  async function loadFlanking() {
    if (!selectedId) return;
    try {
      const res = await Combatants.flanking(selectedId);
      flankingPairs = res.flanking_pairs;
    } catch { flankingPairs = []; }
  }

  async function checkCover(attackerId: string, targetId: string) {
    if (!selectedId) return;
    try { coverResult = await Combatants.cover(selectedId, attackerId, targetId); }
    catch { coverResult = null; }
  }

  async function doCastSpell(caster: Combatant) {
    if (!castSpellSlug) { error = 'Enter spell slug'; return; }
    if (castTargets.length === 0) { error = 'Select at least one target'; return; }
    error = '';
    try {
      const body: Record<string, unknown> = {
        spell_slug: castSpellSlug,
        target_ids: castTargets,
        damage_expression: castDamageExpr || undefined,
        half_on_save: castHalfOnSave,
        cast_as_ritual: castAsRitual || undefined,
        use_spell_attack: castUseSpellAttack || undefined,
      };
      if (castUpcastLevel !== '') body.upcast_level = Number(castUpcastLevel);
      if (castSaveDc !== '') body.save_dc = Number(castSaveDc);
      const res = await Combatants.castSpell(caster.id as string, body as Parameters<typeof Combatants.castSpell>[1]);
      castResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; castResult = null; }
  }

  async function doParseMultiattack(attacker: Combatant) {
    if (!multiattackParseTarget) { error = 'Select a target for parsed attacks'; return; }
    error = '';
    try {
      const parsed = await Combatants.parseMultiattack(attacker.id as string);
      multiattackTargets = parsed.attacks.map((a) => ({
        target_id: multiattackParseTarget,
        attack_expr: a.attack_expression ?? '',
        damage_expr: a.damage_expression ?? '',
        damage_type: a.damage_type,
        weapon_id: undefined,
      }));
    } catch (e) { error = (e as Error).message; }
  }

  async function doMultiattack(attacker: Combatant) {
    if (multiattackTargets.length === 0) { error = 'Add at least one target'; return; }
    error = '';
    try {
      const res = await Combatants.multiattack(attacker.id as string, {
        targets: multiattackTargets.map((t) => ({
          target_id: t.target_id,
          attack_expression: t.attack_expr || undefined,
          damage_expression: t.damage_expr || undefined,
          damage_type: t.damage_type,
          weapon_id: t.weapon_id || undefined,
        })),
      });
      multiattackResult = res;
      await loadList();
      setTimeout(() => multiattackResult = null, 5000);
    } catch (e) { error = (e as Error).message; multiattackResult = null; }
  }

  async function doOverlayDamage() {
    if (!overlayDmgId) { error = 'Select an overlay'; return; }
    if (!overlayDmgExpr) { error = 'Enter damage expression'; return; }
    if (!selectedId) return;
    error = '';
    try {
      const body: Record<string, unknown> = {
        overlay_id: overlayDmgId,
        damage_expression: overlayDmgExpr,
        damage_type: overlayDmgType,
        save_ability: overlaySaveAbility,
        half_on_save: overlayHalfOnSave,
      };
      if (overlaySaveDc !== '') body.save_dc = Number(overlaySaveDc);
      const res = await Combatants.overlayDamage(selectedId, body as Parameters<typeof Combatants.overlayDamage>[1]);
      overlayDmgResult = res;
      await loadList();
      setTimeout(() => overlayDmgResult = null, 5000);
    } catch (e) { error = (e as Error).message; overlayDmgResult = null; }
  }

  async function doSurpriseRound() {
    if (!selectedId) return;
    error = '';
    try {
      await Combatants.surpriseRound(selectedId, surprisedCombatantIds);
      await loadList();
      showSurpriseForm = false;
      surprisedCombatantIds = [];
    } catch (e) { error = (e as Error).message; }
  }

  async function doSurpriseAuto() {
    if (!selectedId) return;
    error = '';
    surpriseAutoResult = null;
    try {
      const res = await Combatants.surpriseAuto(selectedId, surprisedCombatantIds);
      surpriseAutoResult = res;
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function doReact(c: Combatant) {
    error = '';
    try {
      await Combatants.react(c.id as string, reactType, reactLabel || undefined);
      await loadList();
      showReactForm = false;
      reactLabel = '';
    } catch (e) { error = (e as Error).message; }
  }

  function setActiveForm(c: Combatant) {
    formCombatant = c.id as string;
  }

  async function rollDice(expression: string, label?: string) {
    error = '';
    try {
      const res = await Dice.roll(cid, expression, label);
      diceHistory = [res, ...diceHistory].slice(0, 20);
    } catch (e) { error = (e as Error).message; }
  }

  async function loadDiceHistory() {
    try { diceHistory = await Dice.history(cid, 20); }
    catch { diceHistory = []; }
  }

  async function doDodge(c: Combatant) {
    try { await Combatants.dodge(c.id as string); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }
  async function doDisengage(c: Combatant, useBa = false) {
    try { await Combatants.disengage(c.id as string, useBa); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }
  async function doDash(c: Combatant, useBa = false) {
    try { await Combatants.dash(c.id as string, useBa); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }
  async function doHide(c: Combatant, useBa = false) {
    try { await Combatants.hide(c.id as string, useBa); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }
  async function doHelp(c: Combatant, targetId: string) {
    try { await Combatants.help(c.id as string, targetId); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }
  async function doOppAttack(attackerId: string, targetId: string) {
    try {
      await Combatants.opportunityAttack(attackerId, targetId);
      await loadList();
      oppAttackPrompt = oppAttackPrompt.filter((p) => !(p.attacker_id === attackerId && p.target_id === targetId));
    } catch (e) { error = (e as Error).message; }
  }
  async function loadDifficulty() {
    if (!selectedId) return;
    try { encounterDifficulty = await Combatants.difficulty(selectedId); }
    catch { encounterDifficulty = null; }
  }

  async function loadCombatEvents() {
    if (!selectedId) return;
    combatEventsLoading = true;
    try { combatEvents = await Combatants.events(selectedId); }
    catch { combatEvents = []; }
    finally { combatEventsLoading = false; }
  }

  /** Check if a combatant wields a reach weapon (for OA range extension) */
  function hasReachWeapon(c: Combatant): boolean {
    if (c.character_id) {
      const ch = partyChars.find((p) => p.id === c.character_id);
      if (!ch) return false;
      const weapons = (ch.sheet as Record<string, unknown>)?.weapons as Array<{ properties?: string }> | undefined;
      return weapons?.some((w) => (w.properties ?? '').toLowerCase().includes('reach')) ?? false;
    }
    if (c.npc_id) {
      const npc = allNpcs.find((n) => n.id === c.npc_id);
      if (!npc?.stats) return false;
      const weapons = (npc.stats as Record<string, unknown>)?.weapons as Array<{ properties?: string }> | undefined;
      return weapons?.some((w) => (w.properties ?? '').toLowerCase().includes('reach')) ?? false;
    }
    return false;
  }

  /** OA reach in grid cells: 1.5 (5ft) normally, 2.5 (10ft) for reach weapons */
  function oaReachCells(c: Combatant): number {
    return hasReachWeapon(c) ? 2.5 : 1.5;
  }

  // Detect opportunity attacks after token move (frontend-side since we have map dims)
  function checkOpportunityAttacks(movedCombatant: Combatant, oldX: number, oldY: number, newX: number, newY: number) {
    if (!mapEl || !currentEnc) return;
    const g = (currentEnc.map_grid_size as number) ?? 50;
    // Compute approximate distance in grid cells
    const dx = ((newX - oldX) / 100) * mapW;
    const dy = ((newY - oldY) / 100) * mapH;
    const movedDist = Math.sqrt(dx*dx + dy*dy);
    if (movedDist < g * 0.5) return; // too small to matter

    const enemies = combatants.filter((c) => c.id !== movedCombatant.id && c.token_on_map && c.hp_current > 0);
    const prompts: Array<{ attacker_id: string; attacker_name: string; target_id: string }> = [];
    for (const enemy of enemies) {
      if (!enemy.token_x || !enemy.token_y) continue;
      // Old distance in px
      const ex = (enemy.token_x / 100) * mapW;
      const ey = (enemy.token_y / 100) * mapH;
      const oldCx = (oldX / 100) * mapW;
      const oldCy = (oldY / 100) * mapH;
      const oldDist = Math.sqrt((oldCx - ex)**2 + (oldCy - ey)**2);
      // Per-enemy reach: 1.5 cells (5ft) normally, 2.5 cells (10ft) for reach weapons
      const reach = oaReachCells(enemy);
      if (oldDist <= g * reach) {
        prompts.push({ attacker_id: enemy.id as string, attacker_name: enemy.display_name, target_id: movedCombatant.id as string });
      }
    }
    if (prompts.length > 0) {
      oppAttackPrompt = [...oppAttackPrompt, ...prompts];
    }
  }

  async function setMapImage(url: string | null) {
    const id = selectedId; if (!id) return;
    if (url === null && !confirm($_('initiative.clear_map_confirm'))) return;
    await guarded('map:setImage', async () => {
      if (url) await Encounters.update(id, { map_image: url });
      else await Encounters.update(id, { clear_map_image: true });
      await loadList();
    });
  }

  async function setGrid(n: number) {
    const id = selectedId; if (!id) return;
    try { await Encounters.update(id, { map_grid_size: n }); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }

  async function placeTokenAtCentre(c: Combatant, on: boolean) {
    if (!campaign().isMaster) return;
    if (!on && !confirm($_('initiative.remove_token_confirm'))) return;
    await guarded(`token:place:${c.id}:${on}`, async () => {
      if (on) {
        await Encounters.combatants.update(c.id as string, {
          token_on_map: true,
          token_x: c.token_x == null ? 50 : c.token_x,
          token_y: c.token_y == null ? 50 : c.token_y,
        });
      } else {
        await Encounters.combatants.update(c.id as string, { token_on_map: false });
      }
      await loadList();
    });
  }

  async function placeAllTokens() {
    if (!campaign().isMaster) return;
    if (!confirm($_('initiative.place_all_tokens_confirm'))) return;
    await guarded('map:placeAll', async () => {
      // Arrange party on the left, NPCs on the right, evenly spaced.
      const players = combatants.filter((c) => c.ref_type === 'character');
      const npcs    = combatants.filter((c) => c.ref_type !== 'character');
      async function layout(list: Combatant[], xPct: number) {
        if (list.length === 0) return;
        const step = 80 / Math.max(list.length, 1);
        for (let i = 0; i < list.length; i++) {
          const y = 10 + step * (i + 0.5);
          await Encounters.combatants.update(list[i].id as string, { token_x: xPct, token_y: y, token_on_map: true });
        }
      }
      await layout(players, 20);
      await layout(npcs, 80);
      await loadList();
    });
  }

  async function saveTokenImage(c: Combatant, url: string | null) {
    try {
      if (url) await Encounters.combatants.update(c.id as string, { token_image: url });
      else await Encounters.combatants.update(c.id as string, { clear_token_image: true });
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  function tokenBg(c: Combatant): string {
    if (c.token_color) return c.token_color as string;
    if (c.ref_type === 'character') {
      const ch = partyChars.find((p) => p.id === c.character_id);
      const seed = (ch?.id as string | undefined) ?? (c.id as string);
      let h = 0;
      for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) & 0xffff;
      return `hsl(${h % 360} 55% 40%)`;
    }
    return '#8b1a1a';
  }

  function tokenInitial(c: Combatant): string {
    return ((c.display_name as string) || '?').trim().charAt(0).toUpperCase();
  }

  const tokensOnMap = $derived(combatants.filter((c) => c.token_on_map && c.token_x != null && c.token_y != null));

  function hpRatio(c: Combatant): number {
    const mx = (c.hp_max as number) || 1;
    return Math.max(0, Math.min(1, (c.hp_current as number) / mx));
  }
  function hpColor(r: number): string {
    if (r >= 0.66) return '#6b8a4f';
    if (r >= 0.33) return '#c9a84c';
    return '#a93535';
  }
</script>

<section class="council">
  <!-- header -->
  <header class="council-head">
    <div class="hdr-icon"><Swords size={28} style="color:#a6855c;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('initiative.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('initiative.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
      <button type="button" class="audio-toggle mr-2" onclick={() => audioEnabled = !audioEnabled} title={audioEnabled ? $_('initiative.sound_on') : $_('initiative.sound_off')}>
        {audioEnabled ? '🔊' : '🔇'}
      </button>
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('initiative.new_encounter')}`} title={$_('initiative.new_encounter')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2">
              <input required placeholder={$_('initiative.encounter_name_ph')} bind:value={newName}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
  </header>

  <div class="rule"></div>

  {#if error}<p class="err">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}
  {#if reactionWindowNotice}
    <ReactionNotice notice={reactionWindowNotice} onClose={() => reactionWindowNotice = null} />
  {/if}

  {#if encs.length === 0}
    <p class="empty">{$_('initiative.empty')}</p>
  {:else}
    <!-- encounter tabs -->
    <EncounterTabs encounters={encs} selectedId={selectedId} onSelect={(id) => selectedId = id} />

    {#if selectedId && currentEnc}
      {@const active = currentEnc.status === 'active'}
      {@const activeC = activeCtxCombatant}
      {@const total = combatants.length}

      <!-- opportunity attack prompts -->
      {#if oppAttackPrompt.length > 0}
        <div class="opp-prompts">
          {#each oppAttackPrompt as p (p.attacker_id + '-' + p.target_id)}
            <div class="opp-prompt">
              <span>{$_('initiative.label_opp_attack_prompt', { values: { attacker: p.attacker_name, target: combatants.find((c) => c.id === p.target_id)?.display_name ?? '' } })}</span>
              <button type="button" class="opp-btn" onclick={() => guarded(`opp:${p.attacker_id}:${p.target_id}`, () => doOppAttack(p.attacker_id, p.target_id))} disabled={isInFlight(`opp:${p.attacker_id}:${p.target_id}`)}>{$_('initiative.opp_attack')}</button>
              <button type="button" class="opp-btn skip" onclick={() => oppAttackPrompt = oppAttackPrompt.filter((x) => !(x.attacker_id === p.attacker_id && x.target_id === p.target_id))}>{$_('initiative.label_skip')}</button>
            </div>
          {/each}
        </div>
      {/if}

      <!-- banner -->
      <Banner
        encounter={currentEnc}
        combatants={combatants}
        isMaster={campaign().isMaster}
        encounterDifficulty={encounterDifficulty}
        flankingPairs={flankingPairs}
        pendingCombatants={pendingCombatants}
        isInFlight={isInFlight}
        onLairAction={async () => { await guarded('lair:action', async () => { await Combatants.lairAction(selectedId!); await loadList(); }); }}
        onLoadDifficulty={loadDifficulty}
        onLoadFlanking={loadFlanking}
        onShowCombatLog={() => { showCombatLog = true; loadCombatEvents(); }}
        onStart={start}
        onPrev={prev}
        onNext={next}
        onEnd={end}
        onRemove={removeEncounter}
      />

      {#if active && activeC}
        {@const canToggle = campaign().isMaster || (activeC.ref_type === 'character' && partyChars.find((p) => p.id === activeC.character_id)?.owner_id === auth.user?.id)}
        {@const spd = activeC.ref_type === 'character' ? charSpeed(activeC) : 30}
        <div class="spotlight">
          <div class="spot-crown"><Crown size={18} style="color:#c9a84c;" /></div>
          <div class="spot-body">
            <div class="spot-title">
              {$_('initiative.active_turn').replace('{{name}}', activeC.display_name as string)}
            </div>
            <div class="spot-stats">
              <span><Dice5 size={11} /> {activeC.initiative}</span>
              {#if campaign().isMaster || (activeC.hp_max as number) > 0}
                <span class="sep">·</span>
                <span><Heart size={11} /> {activeC.hp_current}/{activeC.hp_max}{(activeC.temp_hp as number) > 0 ? ` (+${activeC.temp_hp})` : ''}</span>
              {/if}
              {#if campaign().isMaster || (activeC.ac as number) > 0}
                <span class="sep">·</span>
                <span><Shield size={11} />
                  {#if activeComputedStats && activeComputedStats.ac !== activeC.ac}
                    {activeC.ac}→{activeComputedStats.ac}
                  {:else}
                    {activeC.ac}
                  {/if}
                </span>
              {/if}
              {#if activeComputedStats}
                {#if activeComputedStats.attack_advantage}<span class="stat-badge adv">Adv</span>{/if}
                {#if activeComputedStats.attack_disadvantage}<span class="stat-badge dis">Dis</span>{/if}
                {#if activeComputedStats.save_advantage}<span class="stat-badge sadv">{$_('initiative.badge_sadv')}</span>{/if}
                {#if activeComputedStats.save_disadvantage}<span class="stat-badge sdis">{$_('initiative.badge_sdis')}</span>{/if}
                {#if activeComputedStats.speed_halved}<span class="stat-badge slow">{$_('initiative.badge_slow')}</span>{/if}
                {#if activeComputedStats.incapacitated}<span class="stat-badge incap">{$_('initiative.badge_incap')}</span>{/if}
                {#if activeComputedStats.resistances.length > 0}
                  <span class="stat-badge res" title={activeComputedStats.resistances.join(', ')}>{$_('initiative.badge_res')}</span>
                {/if}
                {#if activeComputedStats.immunities.length > 0}
                  <span class="stat-badge imm" title={activeComputedStats.immunities.join(', ')}>{$_('initiative.badge_imm')}</span>
                {/if}
                {#if activeComputedStats.exhaustion > 0}
                  <span class="stat-badge exhaust" title={$_('initiative.exhaustion', { values: { n: activeComputedStats.exhaustion } })}>Ex {activeComputedStats.exhaustion}</span>
                {/if}
                {#if activeComputedStats.passive_scores.length > 0}
                  {@const pp = activeComputedStats.passive_scores.find(([s]) => s === 'perception')}
                  {#if pp}<span class="stat-badge pp" title={$_('initiative.passive_perception', { values: { n: pp[1] } })}>PP {pp[1]}</span>{/if}
                {/if}
              {/if}
            </div>
            <!-- action economy chips -->
            <div class="action-chips">
              <button type="button" class="act-chip {activeC.action_used ? 'used' : ''}" onclick={() => canToggle && guarded(`useAction:action:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'action'); await loadList(); })} disabled={!canToggle || isInFlight(`useAction:action:${activeC.id}`)}>
                ⚔️ {$_('initiative.action_action')}
              </button>
              <button type="button" class="act-chip {activeC.bonus_action_used ? 'used' : ''}" onclick={() => canToggle && guarded(`useAction:ba:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'bonus_action'); await loadList(); })} disabled={!canToggle || isInFlight(`useAction:ba:${activeC.id}`)}>
                ⚡ {$_('initiative.action_bonus')}
              </button>
              <button type="button" class="act-chip {activeC.reaction_used ? 'used' : ''}" onclick={() => canToggle && guarded(`useAction:reaction:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'reaction'); await loadList(); })} disabled={!canToggle || isInFlight(`useAction:reaction:${activeC.id}`)}>
                ↩️ {$_('initiative.action_reaction')}
              </button>
              <span class="act-chip move-chip">👣 {activeC.movement_used_ft}/{spd}ft</span>
              {#if activeC.legendary_actions_max > 0}
                <span class="legendary-dots" title={$_('initiative.action_legendary')}>
                  {#each Array(activeC.legendary_actions_max) as _, i (i)}
                    <button type="button" class="ldot {i < activeC.legendary_actions_used ? 'spent' : ''}" onclick={() => campaign().isMaster && guarded(`legendary:${activeC.id}:${i}`, async () => { await Combatants.legendaryAction(activeC.id as string); await loadList(); })} disabled={!campaign().isMaster || isInFlight(`legendary:${activeC.id}:${i}`)}>⚡</button>
                  {/each}
                </span>
              {/if}
              {#if activeC.legendary_resistances_max > 0}
                <button type="button" class="act-chip lr-chip" onclick={() => campaign().isMaster && guarded(`lr:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'legendary_resistance'); await loadList(); })} disabled={!campaign().isMaster || isInFlight(`lr:${activeC.id}`)}>
                  🛡️ LR: {activeC.legendary_resistances_max - activeC.legendary_resistances_used}/{activeC.legendary_resistances_max}
                </button>
              {/if}
            </div>
            <!-- death save prompt -->
            {#if activeC.hp_current <= 0 && activeC.hp_max > 0}
              <div class="death-save-banner">
                <span class="ds-title">💀 {activeC.display_name} {$_('initiative.ds_title_dying').replace('{{name}}', '')}</span>
                <div class="ds-track">
                  <span>{$_('initiative.ds_successes')}</span>
                  {#each [1,2,3] as i}
                    <span class="ds-dot {deathSaveResult ? (deathSaveResult.successes_after >= i ? 'ds-filled' : '') : ''}">●</span>
                  {/each}
                  <span>{$_('initiative.ds_failures')}</span>
                  {#each [1,2,3] as i}
                    <span class="ds-dot ds-fail {deathSaveResult ? (deathSaveResult.failures_after >= i ? 'ds-filled' : '') : ''}">●</span>
                  {/each}
                </div>
                <button type="button" class="ca-submit" onclick={() => guarded(`deathsave:${activeC.id}`, () => doDeathSave(activeC))} disabled={isInFlight(`deathsave:${activeC.id}`)}>
                  <Dices size={14} /> {$_('initiative.ds_roll')}
                </button>
                {#if deathSaveResult}
                  <div class="ca-result {deathSaveResult.stabilized ? 'hit' : deathSaveResult.died ? 'miss' : ''}">
                    {#if deathSaveResult.nat20}
                      <span>{$_('initiative.ds_nat20')}</span>
                    {:else if deathSaveResult.nat1}
                      <span>{$_('initiative.ds_nat1').replace('{{f}}', String(deathSaveResult.failures_after))}</span>
                    {:else if deathSaveResult.passed}
                      <span>{$_('initiative.ds_success')} ({deathSaveResult.successes_after}/3)</span>
                    {:else}
                      <span>{$_('initiative.ds_failure')} ({deathSaveResult.failures_after}/3)</span>
                    {/if}
                    {#if deathSaveResult.stabilized}<span>{$_('initiative.ds_stabilized')}</span>{/if}
                    {#if deathSaveResult.died}<span>{$_('initiative.ds_died')}</span>{/if}
                  </div>
                {/if}
              </div>
            {/if}

            <!-- combat actions -->
            {#if campaign().isMaster || canToggle}
              <div class="combat-actions">
                <button type="button" class="ca-btn" onclick={() => { showAttackForm = !showAttackForm; showDmgForm = false; showSaveForm = false; showCastForm = false; showSkillForm = false; showHelpForm = false; }}>
                  <Swords size={12} /> Attack
                </button>
                <button type="button" class="ca-btn" onclick={() => { showDmgForm = !showDmgForm; showAttackForm = false; showSaveForm = false; showCastForm = false; showSkillForm = false; showHelpForm = false; }}>
                  <Heart size={12} /> Damage
                </button>
                <button type="button" class="ca-btn" onclick={() => { showSaveForm = !showSaveForm; showAttackForm = false; showDmgForm = false; showCastForm = false; showSkillForm = false; showHelpForm = false; }}>
                  <Shield size={12} /> Save
                </button>
                <button type="button" class="ca-btn" onclick={() => { showSkillForm = !showSkillForm; showAttackForm = false; showDmgForm = false; showSaveForm = false; showCastForm = false; showHelpForm = false; }}>
                  <Brain size={12} /> Skill
                </button>
                <button type="button" class="ca-btn" onclick={() => { showCastForm = !showCastForm; showAttackForm = false; showDmgForm = false; showSaveForm = false; showSkillForm = false; showHelpForm = false; }}>
                  <Sparkles size={12} /> Cast
                </button>
                <button type="button" class="ca-btn" onclick={() => guarded(`dodge:${activeC.id}`, () => doDodge(activeC))} disabled={isInFlight(`dodge:${activeC.id}`)} title={$_('initiative.title_dodge')}>
                  <Shield size={12} /> Dodge
                </button>
                {#if hasRogueClass(activeC)}
                  <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`disengage:ba:${activeC.id}`, () => doDisengage(activeC, true))} disabled={isInFlight(`disengage:ba:${activeC.id}`)} title={$_('initiative.title_disengage_ba')}>
                    <Wind size={12} /> Disengage (BA)
                  </button>
                  <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`dash:ba:${activeC.id}`, () => doDash(activeC, true))} disabled={isInFlight(`dash:ba:${activeC.id}`)} title={$_('initiative.title_dash_ba')}>
                    <Wind size={12} /> Dash (BA)
                  </button>
                  <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`hide:ba:${activeC.id}`, () => doHide(activeC, true))} disabled={isInFlight(`hide:ba:${activeC.id}`)} title={$_('initiative.title_hide_ba')}>
                    <Wind size={12} /> Hide (BA)
                  </button>
                {:else}
                  <button type="button" class="ca-btn" onclick={() => guarded(`disengage:${activeC.id}`, () => doDisengage(activeC))} disabled={isInFlight(`disengage:${activeC.id}`)} title={$_('initiative.title_disengage')}>
                    <Wind size={12} /> Disengage
                  </button>
                  <button type="button" class="ca-btn" onclick={() => guarded(`dash:${activeC.id}`, () => doDash(activeC))} disabled={isInFlight(`dash:${activeC.id}`)} title={$_('initiative.title_dash')}>
                    <Wind size={12} /> Dash
                  </button>
                  <button type="button" class="ca-btn" onclick={() => guarded(`hide:${activeC.id}`, () => doHide(activeC))} disabled={isInFlight(`hide:${activeC.id}`)} title={$_('initiative.title_hide')}>
                    <Wind size={12} /> Hide
                  </button>
                {/if}
                <button type="button" class="ca-btn" onclick={() => { showHelpForm = !showHelpForm; showAttackForm = false; showDmgForm = false; showSaveForm = false; showSkillForm = false; showCastForm = false; }} title={$_('initiative.title_help')}>
                  <Hand size={12} /> Help
                </button>
                <button type="button" class="ca-btn" onclick={() => { showGrappleForm = !showGrappleForm; showShoveForm = false; showReadyForm = false; showEscapeForm = false; }} title={$_('initiative.title_grapple')}>
                  <Swords size={12} /> Grapple
                </button>
                {#if activeC.conditions?.some(c => c.split(':')[0].toLowerCase() === 'grappled')}
                  <button type="button" class="ca-btn" onclick={() => { showEscapeForm = !showEscapeForm; showGrappleForm = false; showShoveForm = false; showReadyForm = false; }} title={$_('initiative.title_escape')}>
                    <Wind size={12} /> Escape
                  </button>
                {/if}
                {#if activeC.conditions?.some(c => c.split(':')[0].toLowerCase() === 'prone')}
                  <button type="button" class="ca-btn" onclick={() => guarded(`standup:${activeC.id}`, () => doStandUp(activeC))} disabled={isInFlight(`standup:${activeC.id}`)} title={$_('initiative.title_stand_up')}>
                    <Wind size={12} /> Stand Up
                  </button>
                {/if}
                <button type="button" class="ca-btn" onclick={() => { showShoveForm = !showShoveForm; showGrappleForm = false; showReadyForm = false; showEscapeForm = false; }} title={$_('initiative.title_shove')}>
                  <Swords size={12} /> Shove
                </button>
                <button type="button" class="ca-btn" onclick={() => { showReadyForm = !showReadyForm; showGrappleForm = false; showShoveForm = false; }} title={$_('initiative.title_ready')}>
                  <Shield size={12} /> Ready
                </button>
                {#if activeC.readied_action}
                  <button type="button" class="ca-btn" onclick={() => guarded(`trigger:${activeC.id}`, () => doTriggerReady(activeC))} disabled={isInFlight(`trigger:${activeC.id}`)} title={$_('initiative.title_trigger_ready', { values: { trigger: activeC.readied_action.trigger } })}>
                    <Swords size={12} /> Trigger Ready
                  </button>
                {/if}
                <button type="button" class="ca-btn" onclick={() => guarded(`delay:${activeC.id}`, () => doDelay(activeC))} disabled={isInFlight(`delay:${activeC.id}`)} title={$_('initiative.title_delay')}>
                  <Hourglass size={12} /> Delay
                </button>
                <button type="button" class="ca-btn" onclick={() => { showMultiattackForm = !showMultiattackForm; showOverlayDmgForm = false; showSurpriseForm = false; showReactForm = false; }} title={$_('initiative.title_multiattack')}>
                  <Swords size={12} /> Multi
                </button>
                <button type="button" class="ca-btn" onclick={() => { showReactForm = !showReactForm; showMultiattackForm = false; showOverlayDmgForm = false; showSurpriseForm = false; }} title={$_('initiative.title_react')}>
                  <Shield size={12} /> React
                </button>
                {#if campaign().isMaster}
                  <button type="button" class="ca-btn" onclick={() => { showOverlayDmgForm = !showOverlayDmgForm; showMultiattackForm = false; showSurpriseForm = false; showReactForm = false; }} title={$_('initiative.title_overlay_dmg')}>
                    <Sparkles size={12} /> Overlay Dmg
                  </button>
                  <button type="button" class="ca-btn" onclick={() => { showSurpriseForm = !showSurpriseForm; showMultiattackForm = false; showOverlayDmgForm = false; showReactForm = false; }} title={$_('initiative.title_surprise')}>
                    <Brain size={12} /> Surprise
                  </button>
                {/if}
              </div>

              <!-- Class features -->
              <div class="ca-row mt-1">
                <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`feature:action_surge:${activeC.id}`, () => doClassFeature(activeC, 'action_surge'))} disabled={isInFlight(`feature:action_surge:${activeC.id}`)} title={$_('initiative.title_action_surge')}>
                  Action Surge
                </button>
                <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`feature:second_wind:${activeC.id}`, () => doClassFeature(activeC, 'second_wind'))} disabled={isInFlight(`feature:second_wind:${activeC.id}`)} title={$_('initiative.title_second_wind')}>
                  Second Wind
                </button>
                <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`feature:rage:${activeC.id}`, () => doClassFeature(activeC, 'rage'))} disabled={isInFlight(`feature:rage:${activeC.id}`)} title={$_('initiative.title_rage')}>
                  Rage
                </button>
                <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`feature:ud:${activeC.id}`, () => doClassFeature(activeC, 'uncanny_dodge'))} disabled={isInFlight(`feature:ud:${activeC.id}`)} title={$_('initiative.title_uncanny_dodge')}>
                  Uncanny Dodge
                </button>
                <button type="button" class="ca-btn ca-btn-sm" onclick={() => guarded(`feature:loh:${activeC.id}`, () => doClassFeature(activeC, 'lay_on_hands', attackTarget || activeC.id as string))} disabled={isInFlight(`feature:loh:${activeC.id}`)} title={$_('initiative.title_lay_on_hands')}>
                  Lay on Hands
                </button>
              </div>
              {#if classFeatureResult}
                <div class="ca-result hit mt-1">
                  <span>{classFeatureResult.message}</span>
                  {#if classFeatureResult.hp_after !== null && classFeatureResult.hp_after !== undefined}<span>HP: {classFeatureResult.hp_after}</span>{/if}
                </div>
              {/if}

              {#if showAttackForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_target')}</span>
                    <select bind:value={attackTarget}>
                      <option value="">Select target…</option>
                      {#each combatants as t (t.id)}
                        <option value={t.id}>{t.display_name}</option>
                      {/each}
                    </select>
                  </label>
                  {#if activeC.character_id}
                    {@const activeChar = partyChars.find((p) => p.id === activeC.character_id)}
                    {@const weapons = (activeChar?.sheet?.weapons ?? []) as Array<{ id: string; name: string; attack_bonus?: number; damage?: string; damage_type?: string }>}
                    {#if weapons.length > 0}
                      <label class="ca-field">
                        <span>{$_('initiative.label_weapon')}</span>
                        <select bind:value={attackWeaponId}>
                          <option value="">Manual…</option>
                          {#each weapons as w (w.id)}
                            <option value={w.id}>{w.name} {w.attack_bonus ? `(+${w.attack_bonus})` : ''} {w.damage ? `[${w.damage}]` : ''}</option>
                          {/each}
                        </select>
                      </label>
                    {/if}
                  {/if}
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_attack')}</span><input type="text" bind:value={attackExpr} placeholder="1d20+7" /></label>
                    <label class="ca-field"><span>{$_('initiative.label_damage')}</span><input type="text" bind:value={damageExpr} placeholder="1d8+4" /></label>
                  </div>
                  <div class="ca-row">
                    <label class="ca-field">
                      <span>{$_('initiative.label_dmg_type')}</span>
                      <select bind:value={damageType}>
                        <option value="slashing">Slashing</option>
                        <option value="piercing">Piercing</option>
                        <option value="bludgeoning">Bludgeoning</option>
                        <option value="fire">Fire</option>
                        <option value="cold">Cold</option>
                        <option value="lightning">Lightning</option>
                        <option value="thunder">Thunder</option>
                        <option value="acid">Acid</option>
                        <option value="poison">Poison</option>
                        <option value="necrotic">Necrotic</option>
                        <option value="radiant">Radiant</option>
                        <option value="psychic">Psychic</option>
                        <option value="force">Force</option>
                      </select>
                    </label>
                    <label class="ca-check"><input type="checkbox" bind:checked={attackAdv} /> Adv</label>
                    <label class="ca-check"><input type="checkbox" bind:checked={attackDis} /> Dis</label>
                    <label class="ca-check" title={$_('initiative.title_power_atk')}><input type="checkbox" bind:checked={powerAttack} /> {$_('initiative.label_power_atk')}</label>
                    <label class="ca-check" title={$_('initiative.title_reckless')}><input type="checkbox" bind:checked={recklessAttack} /> {$_('initiative.label_reckless')}</label>
                    <label class="ca-check" title={$_('initiative.title_skip_ammo')}><input type="checkbox" bind:checked={skipAmmo} /> {$_('initiative.label_skip_ammo')}</label>
                    <label class="ca-field" title={$_('initiative.title_bless')}>
                      <span>{$_('initiative.label_bless')}</span>
                      <input type="number" min="0" max="4" bind:value={blessDice} style="width:3rem" />
                    </label>
                    <label class="ca-field" title={$_('initiative.title_bardic_inspiration')}>
                      <span>{$_('initiative.label_bard')}</span>
                      <select bind:value={bardicInspirationDie} style="width:4rem">
                        <option value={0}>—</option>
                        <option value={6}>d6</option>
                        <option value={8}>d8</option>
                        <option value={10}>d10</option>
                        <option value={12}>d12</option>
                      </select>
                    </label>
                    <label class="ca-field">
                      <span>{$_('initiative.label_cover')}</span>
                      <select bind:value={coverType}>
                        <option value="none">None</option>
                        <option value="half">Half (+2)</option>
                        <option value="three_quarters">3/4 (+5)</option>
                      </select>
                    </label>
                    <button type="button" class="ca-btn" onclick={() => attackTarget && checkCover(activeC.id as string, attackTarget)}>{$_('initiative.label_check_cover')}</button>
                  </div>
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_extra_dmg')}</span><input type="text" bind:value={extraDamageExpr} placeholder={$_('initiative.ph_extra_dmg')} /></label>
                    {#if extraDamageExpr}
                      <label class="ca-field">
                        <span>{$_('initiative.label_extra_type')}</span>
                        <select bind:value={extraDamageType}>
                          <option value="piercing">Piercing</option>
                          <option value="slashing">Slashing</option>
                          <option value="bludgeoning">Bludgeoning</option>
                          <option value="radiant">Radiant</option>
                          <option value="necrotic">Necrotic</option>
                          <option value="fire">Fire</option>
                          <option value="cold">Cold</option>
                          <option value="lightning">Lightning</option>
                          <option value="thunder">Thunder</option>
                          <option value="psychic">Psychic</option>
                          <option value="force">Force</option>
                        </select>
                      </label>
                    {/if}
                  </div>
                  {#if coverResult && coverResult.attacker_id === activeC.id && coverResult.target_id === attackTarget}
                    <div class="ca-result">
                      <span>{$_('initiative.label_cover_result', { values: { type: coverResult.cover_type, bonus: coverResult.cover_bonus } })}</span>
                      {#if coverResult.blockers.length > 0}<span>{$_('initiative.label_blocked_by', { values: { blockers: coverResult.blockers.join(', ') } })}</span>{/if}
                    </div>
                  {/if}
                  <button type="button" class="ca-submit" onclick={() => guarded(`attack:${activeC.id}`, () => doAttack(activeC))} disabled={isInFlight(`attack:${activeC.id}`)}>
                    <Swords size={12} /> {$_('initiative.label_roll_attack_btn')}
                  </button>
                  {#if attackResult}
                    <div class="ca-result {attackResult.hit ? 'hit' : 'miss'}">
                      {#if attackResult.critical}<span class="ca-crit">{$_('initiative.label_crit')}</span>{/if}
                      {#if attackResult.hit}
                        <span>{$_('initiative.label_hit_msg', { values: { total: attackResult.attack_total, ac: attackResult.target_ac } })}</span>
                        <span>{$_('initiative.label_damage_msg', { values: { amount: attackResult.damage_applied } })} {#if attackResult.damage_resisted}({$_('initiative.label_damage_resisted')}){/if}{#if attackResult.damage_vulnerable}({$_('initiative.label_damage_vulnerable')}){/if}{#if attackResult.damage_immune}({$_('initiative.label_damage_immune')}){/if}{#if attackResult.extra_damage_applied} + {attackResult.extra_damage_applied} {attackResult.extra_damage_type ?? ''}{/if}</span>
                        {#if attackResult.concentration_broken}<span class="ca-conc">{$_('initiative.label_conc_broken')}</span>{/if}
                        {#if attackResult.instant_death}<span class="ca-crit">{$_('initiative.label_instadeath')}</span>{/if}
                      {:else}
                        <span>{$_('initiative.label_miss_msg', { values: { total: attackResult.attack_total, ac: attackResult.target_ac } })}</span>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showDmgForm}
                <div class="ca-form">
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_amount')}</span><input type="number" bind:value={dmgAmount} min="0" /></label>
                    <label class="ca-field">
                      <span>{$_('initiative.label_dmg_type')}</span>
                      <select bind:value={damageType}>
                        <option value="slashing">{$_('initiative.damage_type_slashing')}</option>
                        <option value="piercing">{$_('initiative.damage_type_piercing')}</option>
                        <option value="bludgeoning">{$_('initiative.damage_type_bludgeoning')}</option>
                        <option value="fire">{$_('initiative.damage_type_fire')}</option>
                        <option value="cold">{$_('initiative.damage_type_cold')}</option>
                        <option value="lightning">{$_('initiative.damage_type_lightning')}</option>
                        <option value="thunder">{$_('initiative.damage_type_thunder')}</option>
                        <option value="acid">{$_('initiative.damage_type_acid')}</option>
                        <option value="poison">{$_('initiative.damage_type_poison')}</option>
                        <option value="necrotic">{$_('initiative.damage_type_necrotic')}</option>
                        <option value="radiant">{$_('initiative.damage_type_radiant')}</option>
                        <option value="psychic">{$_('initiative.damage_type_psychic')}</option>
                        <option value="force">{$_('initiative.damage_type_force')}</option>
                      </select>
                    </label>
                  </div>
                  <div class="ca-row">
                    <button type="button" class="ca-submit dmg" onclick={() => guarded(`damage:${activeC.id}`, () => doDamage(activeC))} disabled={isInFlight(`damage:${activeC.id}`)}>{$_('initiative.label_apply_damage')}</button>
                    <button type="button" class="ca-submit heal" onclick={() => guarded(`heal:${activeC.id}`, () => doHeal(activeC))} disabled={isInFlight(`heal:${activeC.id}`)}>{$_('initiative.label_apply_healing')}</button>
                  </div>
                  {#if dmgResult}
                    <div class="ca-result">
                       <span>{$_('initiative.label_applied_dmg', { values: { amount: Math.abs(dmgResult.damage_applied), kind: $_('initiative.label_kind_' + (dmgResult.damage_applied < 0 ? 'healing' : 'damage')) } })}</span>
                      {#if dmgResult.damage_resisted}<span>({$_('initiative.label_damage_resisted')})</span>{/if}
                      {#if dmgResult.concentration_broken}<span class="ca-conc">{$_('initiative.label_conc_broken')}</span>{/if}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showSaveForm}
                <div class="ca-form">
                  <div class="ca-row">
                    <label class="ca-field">
                      <span>{$_('initiative.label_ability')}</span>
                      <select bind:value={saveAbility}>
                        <option value="str">STR</option>
                        <option value="dex">DEX</option>
                        <option value="con">CON</option>
                        <option value="int">INT</option>
                        <option value="wis">WIS</option>
                        <option value="cha">CHA</option>
                      </select>
                    </label>
                    <label class="ca-field"><span>DC</span><input type="number" bind:value={saveDc} min="1" max="40" /></label>
                    <label class="ca-check"><input type="checkbox" bind:checked={saveAdv} /> Adv</label>
                    <label class="ca-check"><input type="checkbox" bind:checked={saveDis} /> Dis</label>
                  </div>
                  <button type="button" class="ca-submit" onclick={() => guarded(`save:${activeC.id}`, () => doSave(activeC))} disabled={isInFlight(`save:${activeC.id}`)}>
                    <Shield size={12} /> Roll Save
                  </button>
                  {#if saveResult}
                    <div class="ca-result {saveResult.passed ? 'hit' : 'miss'}">
                      <span>{saveResult.passed ? 'Passed!' : 'Failed!'} {saveResult.save_total} vs DC {saveResult.dc} (rolled {saveResult.natural_roll})</span>
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showSkillForm}
                <div class="ca-form">
                  <div class="ca-row">
                    <label class="ca-field">
                      <span>{$_('initiative.label_skill')}</span>
                      <select bind:value={skillName}>
                        <option value="acrobatics">Acrobatics (DEX)</option>
                        <option value="animal_handling">Animal Handling (WIS)</option>
                        <option value="arcana">Arcana (INT)</option>
                        <option value="athletics">Athletics (STR)</option>
                        <option value="deception">Deception (CHA)</option>
                        <option value="history">History (INT)</option>
                        <option value="insight">Insight (WIS)</option>
                        <option value="intimidation">Intimidation (CHA)</option>
                        <option value="investigation">Investigation (INT)</option>
                        <option value="medicine">Medicine (WIS)</option>
                        <option value="nature">Nature (INT)</option>
                        <option value="perception">Perception (WIS)</option>
                        <option value="performance">Performance (CHA)</option>
                        <option value="persuasion">Persuasion (CHA)</option>
                        <option value="religion">Religion (INT)</option>
                        <option value="sleight_of_hand">Sleight of Hand (DEX)</option>
                        <option value="stealth">Stealth (DEX)</option>
                        <option value="survival">Survival (WIS)</option>
                      </select>
                    </label>
                    <label class="ca-field"><span>DC</span><input type="number" bind:value={skillDc} min="1" max="40" /></label>
                    <label class="ca-check"><input type="checkbox" bind:checked={skillAdv} /> Adv</label>
                    <label class="ca-check"><input type="checkbox" bind:checked={skillDis} /> Dis</label>
                  </div>
                  <button type="button" class="ca-submit" onclick={() => guarded(`skill:${activeC.id}`, () => doSkillCheck(activeC))} disabled={isInFlight(`skill:${activeC.id}`)}>
                    <Brain size={12} /> Roll Skill Check
                  </button>
                  {#if skillResult}
                    <div class="ca-result {skillResult.passed === true ? 'hit' : skillResult.passed === false ? 'miss' : ''}">
                      <span>{$_('initiative.label_skill_result', { values: { skill: skillResult.skill, total: skillResult.total, rolled: skillResult.natural_roll } })}</span>
                      {#if skillResult.passed === true}<span>{$_('initiative.label_passed')}</span>{:else if skillResult.passed === false}<span>{$_('initiative.label_failed')}</span>{/if}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showCastForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_spell')}</span>
                    <input type="text" bind:value={castSpellFilter} placeholder={$_('initiative.ph_search_spells')} class="mb-1" />
                    {#if castSpellFilter}
                      {@const filtered = allSpells.filter((s) => s.name.toLowerCase().includes(castSpellFilter.toLowerCase()) || s.slug.toLowerCase().includes(castSpellFilter.toLowerCase()))}
                      <select bind:value={castSpellSlug} size={4}>
                        <option value="">— Select a spell —</option>
                        {#each filtered as s (s.slug)}
                          <option value={s.slug}>{s.name} (Lv{s.level}) {s.concentration ? '•' : ''}</option>
                        {/each}
                      </select>
                    {:else}
                      <select bind:value={castSpellSlug} size={4}>
                        <option value="">— Select a spell —</option>
                        {#each allSpells.slice(0, 50) as s (s.slug)}
                          <option value={s.slug}>{s.name} (Lv{s.level}) {s.concentration ? '•' : ''}</option>
                        {/each}
                      </select>
                    {/if}
                  </label>
                  {#if castSpellSlug}
                    {@const sp = allSpells.find((s) => s.slug === castSpellSlug)}
                    {#if sp}
                      <div class="text-xs mb-2" style="color:#a6855c;">
                        {sp.casting_time ?? ''} • {sp.range_text ?? ''} • {sp.components ?? ''}
                        {#if sp.concentration}• Concentration{/if}
                      </div>
                    {/if}
                  {/if}
                  <label class="ca-field">
                    <span>{$_('initiative.label_targets')}</span>
                    <select multiple bind:value={castTargets} size={3}>
                      {#each combatants as t (t.id)}
                        <option value={t.id}>{t.display_name}</option>
                      {/each}
                    </select>
                  </label>
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_damage')}</span><input type="text" bind:value={castDamageExpr} placeholder="8d6" /></label>
                    <label class="ca-check" title={$_('initiative.title_spell_attack')}><input type="checkbox" bind:checked={castUseSpellAttack} /> {$_('initiative.label_spell_attack')}</label>
                    {#if !castUseSpellAttack}<label class="ca-check"><input type="checkbox" bind:checked={castHalfOnSave} /> {$_('initiative.label_half_on_save')}</label>{/if}
                  </div>
                  {#if allSpells.find((s) => s.slug === castSpellSlug)?.level === 0 && castDamageExpr}
                    {@const lvl = activeC.character_id ? (Number((partyChars.find(p => p.id === activeC.character_id)?.sheet as Record<string,unknown>)?.level ?? 1)) : 1}
                    {@const mult = lvl >= 17 ? 4 : lvl >= 11 ? 3 : lvl >= 5 ? 2 : 1}
                    {#if mult > 1}
                      <div class="text-xs" style="color:#a6855c;">Cantrip scaled ×{mult} at level {lvl}: server will roll {castDamageExpr.replace(/^(\d+)/, (_, n) => String(Number(n) * mult))}</div>
                    {/if}
                  {/if}
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_upcast_level')}</span><input type="number" min={1} max={9} bind:value={castUpcastLevel} placeholder={$_('initiative.ph_upcast_level')} /></label>
                    <label class="ca-field"><span>{$_('initiative.label_save_dc')}</span><input type="number" bind:value={castSaveDc} placeholder={$_('initiative.ph_save_dc')} /></label>
                  </div>
                  {#if allSpells.find((s) => s.slug === castSpellSlug)?.ritual}
                    <label class="ca-check"><input type="checkbox" bind:checked={castAsRitual} /> Cast as Ritual (no slot)</label>
                  {/if}
                  <button type="button" class="ca-submit" onclick={() => guarded(`cast:${activeC.id}`, () => doCastSpell(activeC))} disabled={isInFlight(`cast:${activeC.id}`)}>
                    <Sparkles size={12} /> Cast Spell
                  </button>
                  {#if castResult}
                    <div class="ca-result">
                      <span class="ca-crit">{castResult.spell_name}</span>
                      {#each castResult.targets as t (t.target_id)}
                        <span>
                          {t.target_name}:
                          {#if t.hit === false}Miss ({t.attack_total})
                          {:else if t.hit === true}{#if t.critical}CRIT! {/if}Hit ({t.attack_total}) — {t.damage_applied} dmg
                          {:else}{t.damage_applied} dmg {#if t.save_passed === true}(saved){:else if t.save_passed === false}(failed){/if}
                          {/if}
                          {#if t.concentration_broken} [conc broken]{/if}
                        </span>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showHelpForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_target')}</span>
                    <select bind:value={helpTarget}>
                      <option value="">Select ally…</option>
                      {#each combatants as t (t.id)}
                        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                      {/each}
                    </select>
                  </label>
                  <button type="button" class="ca-submit" onclick={() => { if (helpTarget) { doHelp(activeC, helpTarget); helpTarget = ''; showHelpForm = false; } else { error = 'Select a target'; } }}>
                    <Hand size={12} /> Help Ally
                  </button>
                </div>
              {/if}

              {#if showGrappleForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_target')}</span>
                    <select bind:value={grappleTarget}>
                      <option value="">Select target…</option>
                      {#each combatants as t (t.id)}
                        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                      {/each}
                    </select>
                  </label>
                  <button type="button" class="ca-submit" onclick={() => guarded(`grapple:${activeC.id}`, () => doGrapple(activeC))} disabled={isInFlight(`grapple:${activeC.id}`)}>{$_('initiative.label_grapple_submit')}</button>
                  {#if grappleResult}
                    <div class="ca-result {grappleResult.success ? 'hit' : 'miss'}">
                      <span>{grappleResult.success ? 'Success!' : 'Failed!'} {grappleResult.attacker_total} vs {grappleResult.defender_total}</span>
                      {#if grappleResult.grapple_applied}<span>Target grappled!</span>{/if}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showEscapeForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_select_grappler')}</span>
                    <select bind:value={escapeGrapplerId}>
                      <option value="">Select grappler…</option>
                      {#each combatants as t (t.id)}
                        {#if t.id !== activeC.id && t.conditions?.some(c => c.split(':')[0].toLowerCase() === 'grappling')}<option value={t.id}>{t.display_name}</option>{/if}
                      {/each}
                    </select>
                  </label>
                  <button type="button" class="ca-submit" onclick={() => guarded(`escape:${activeC.id}`, () => doGrappleEscape(activeC))} disabled={isInFlight(`escape:${activeC.id}`)}>{$_('initiative.label_escape_grapple')}</button>
                  {#if escapeResult}
                    <div class="ca-result {escapeResult.escaped ? 'hit' : 'miss'}">
                      <span>{escapeResult.escaped ? 'Escaped!' : 'Failed!'} {escapeResult.escapee_total} vs {escapeResult.grappler_total}</span>
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showShoveForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_target')}</span>
                    <select bind:value={shoveTarget}>
                      <option value="">Select target…</option>
                      {#each combatants as t (t.id)}
                        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                      {/each}
                    </select>
                  </label>
                  <label class="ca-check"><input type="checkbox" bind:checked={shoveKnockProne} /> Knock prone (uncheck = push 5ft)</label>
                  <button type="button" class="ca-submit" onclick={() => guarded(`shove:${activeC.id}`, () => doShove(activeC))} disabled={isInFlight(`shove:${activeC.id}`)}>{$_('initiative.label_shove_submit')}</button>
                  {#if shoveResult}
                    <div class="ca-result {shoveResult.success ? 'hit' : 'miss'}">
                      <span>{shoveResult.success ? 'Success!' : 'Failed!'} {shoveResult.attacker_total} vs {shoveResult.defender_total}</span>
                      {#if shoveResult.knocked_prone}<span>Target knocked prone!</span>{/if}
                      {#if shoveResult.pushed_away}<span>Target pushed 5ft!</span>{/if}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showReadyForm}
                <div class="ca-form">
                  <label class="ca-field"><span>{$_('initiative.label_enter_trigger')}</span><input type="text" bind:value={readyTrigger} placeholder={$_('initiative.ph_ready_trigger')} /></label>
                  <div class="ca-row">
                    <label class="ca-field">
                      <span>{$_('initiative.label_auto_trigger')}</span>
                      <select bind:value={readyTriggerEvent}>
                        <option value="">Manual only</option>
                        <option value="target_enters_range">Target enters range (move)</option>
                        <option value="target_attacks">Target attacks</option>
                        <option value="target_casts">Target casts a spell</option>
                      </select>
                    </label>
                    {#if readyTriggerEvent}
                      <label class="ca-field">
                        <span>{$_('initiative.label_watch')}</span>
                        <select bind:value={readyWatchTarget}>
                          <option value="">Anyone</option>
                          {#each combatants as t (t.id)}
                            {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                          {/each}
                        </select>
                      </label>
                    {/if}
                  </div>
                  <label class="ca-field">
                    <span>{$_('initiative.action_action')}</span>
                    <select bind:value={readyAction}>
                      <option value="attack">Attack</option>
                      <option value="cast spell">Cast Spell</option>
                      <option value="dash">Dash</option>
                      <option value="disengage">Disengage</option>
                      <option value="dodge">Dodge</option>
                      <option value="help">Help</option>
                    </select>
                  </label>
                  <button type="button" class="ca-submit" onclick={() => guarded(`ready:${activeC.id}`, () => doReady(activeC))} disabled={isInFlight(`ready:${activeC.id}`)}>{$_('initiative.label_ready_action')}</button>
                </div>
              {/if}

              {#if showMultiattackForm}
                <div class="ca-form">
                  {#if activeC.npc_id}
                    <div class="ca-row" style="align-items:flex-end;">
                      <label class="ca-field">
                        <span>{$_('initiative.label_parse_npc_multiattack')}</span>
                        <select bind:value={multiattackParseTarget}>
                          <option value="">Select target for parsed attacks…</option>
                          {#each combatants as t (t.id)}
                            {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                          {/each}
                        </select>
                      </label>
                      <button type="button" class="ca-btn" disabled={isInFlight(`parse:ma:${activeC.id}`)} onclick={() => guarded(`parse:ma:${activeC.id}`, () => doParseMultiattack(activeC))} title={$_('initiative.title_parse_multiattack')}>{$_('initiative.label_parse')}</button>
                    </div>
                    <hr class="my-1" style="border-color:#3a2313;" />
                  {/if}
                  <div class="ca-row" style="align-items:flex-end;">
                    <label class="ca-field">
                      <span>{$_('initiative.label_add_target')}</span>
                      <select bind:value={attackTarget}>
                        <option value="">Select…</option>
                        {#each combatants as t (t.id)}
                          {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
                        {/each}
                      </select>
                    </label>
                    <label class="ca-field"><span>{$_('initiative.badge_atk')}</span><input type="text" bind:value={attackExpr} placeholder="1d20+7" /></label>
                    <label class="ca-field"><span>{$_('initiative.badge_dmg')}</span><input type="text" bind:value={damageExpr} placeholder="1d8+4" /></label>
                    <label class="ca-field">
                      <span>{$_('initiative.label_dmg_type')}</span>
                      <select bind:value={damageType}>
                        <option value="slashing">Slashing</option>
                        <option value="piercing">Piercing</option>
                        <option value="bludgeoning">Bludgeoning</option>
                        <option value="fire">Fire</option>
                        <option value="cold">Cold</option>
                        <option value="lightning">Lightning</option>
                        <option value="thunder">Thunder</option>
                        <option value="acid">Acid</option>
                        <option value="poison">Poison</option>
                        <option value="necrotic">Necrotic</option>
                        <option value="radiant">Radiant</option>
                        <option value="psychic">Psychic</option>
                        <option value="force">Force</option>
                      </select>
                    </label>
                    <button type="button" class="ca-btn" onclick={() => {
                      if (!attackTarget) return;
                      multiattackTargets = [...multiattackTargets, { target_id: attackTarget, attack_expr: attackExpr, damage_expr: damageExpr, damage_type: damageType, weapon_id: attackWeaponId || undefined }];
                      attackTarget = ''; attackExpr = ''; damageExpr = '';
                    }}>+ Add</button>
                  </div>
                  {#if multiattackTargets.length > 0}
                    <div class="text-xs mb-1" style="color:#a6855c;">
                      {#each multiattackTargets as mt, i (i)}
                        <span class="inline-flex items-center gap-1 mr-2">
                          {combatants.find((c) => c.id === mt.target_id)?.display_name ?? mt.target_id}: {mt.attack_expr} / {mt.damage_expr} {mt.damage_type}
                          <button type="button" class="text-[10px]" style="color:#a93535;" onclick={() => multiattackTargets = multiattackTargets.filter((_, idx) => idx !== i)}>✕</button>
                        </span>
                      {/each}
                    </div>
                  {/if}
                  <button type="button" class="ca-submit" onclick={() => guarded(`multiattack:${activeC.id}`, () => doMultiattack(activeC))} disabled={isInFlight(`multiattack:${activeC.id}`)}>
                    <Swords size={12} /> Roll Multiattack
                  </button>
                  {#if multiattackResult}
                    <div class="ca-result hit">
                      <span>Hit {multiattackResult.targets_hit}/{multiattackResult.results.length} — {multiattackResult.total_damage} total dmg</span>
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showOverlayDmgForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_overlay')}</span>
                    <select bind:value={overlayDmgId}>
                      <option value="">Select overlay…</option>
                      {#each overlays.filter((o) => o.active) as o (o.id)}
                        <option value={o.id}>{o.label || o.zone_type || 'Overlay'} ({o.shape})</option>
                      {/each}
                    </select>
                  </label>
                  <div class="ca-row">
                    <label class="ca-field"><span>{$_('initiative.label_damage')}</span><input type="text" bind:value={overlayDmgExpr} placeholder="8d6" /></label>
                    <label class="ca-field">
                      <span>{$_('initiative.label_dmg_type')}</span>
                      <select bind:value={overlayDmgType}>
                        <option value="fire">Fire</option>
                        <option value="cold">Cold</option>
                        <option value="lightning">Lightning</option>
                        <option value="thunder">Thunder</option>
                        <option value="acid">Acid</option>
                        <option value="poison">Poison</option>
                        <option value="necrotic">Necrotic</option>
                        <option value="radiant">Radiant</option>
                        <option value="psychic">Psychic</option>
                        <option value="force">Force</option>
                        <option value="slashing">Slashing</option>
                        <option value="piercing">Piercing</option>
                        <option value="bludgeoning">Bludgeoning</option>
                      </select>
                    </label>
                  </div>
                  <div class="ca-row">
                    <label class="ca-field">
                      <span>{$_('initiative.label_save_short')}</span>
                      <select bind:value={overlaySaveAbility}>
                        <option value="dex">DEX</option>
                        <option value="con">CON</option>
                        <option value="wis">WIS</option>
                        <option value="str">STR</option>
                        <option value="int">INT</option>
                        <option value="cha">CHA</option>
                      </select>
                    </label>
                    <label class="ca-field"><span>{$_('initiative.badge_dc')}</span><input type="number" bind:value={overlaySaveDc} placeholder="15" /></label>
                    <label class="ca-check"><input type="checkbox" bind:checked={overlayHalfOnSave} /> ½ on save</label>
                  </div>
                  <button type="button" class="ca-submit" onclick={() => guarded('overlay:damage', () => doOverlayDamage())} disabled={isInFlight('overlay:damage')}>
                    <Sparkles size={12} /> Apply Overlay Damage
                  </button>
                  {#if overlayDmgResult}
                    <div class="ca-result">
                      {#each overlayDmgResult.targets_affected as ta (ta.target_id)}
                        <span>{ta.target_name}: {ta.damage_applied} dmg {#if ta.save_passed === true}(saved){:else if ta.save_passed === false}(failed){/if}</span>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showSurpriseForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_surprised_combatants')}</span>
                    <select multiple bind:value={surprisedCombatantIds} size={4}>
                      {#each combatants as t (t.id)}
                        <option value={t.id}>{t.display_name}</option>
                      {/each}
                    </select>
                  </label>
                  <button type="button" class="ca-submit" onclick={() => guarded('surprise:round', () => doSurpriseRound())} disabled={isInFlight('surprise:round')}>
                    <Brain size={12} /> Apply Surprise Round
                  </button>
                  <button type="button" class="ca-submit" onclick={() => guarded('surprise:auto', () => doSurpriseAuto())} disabled={isInFlight('surprise:auto')}>
                    <Sparkles size={12} /> Auto Surprise (Stealth vs PP)
                  </button>
                  {#if surpriseAutoResult}
                    <div class="ca-result">
                      <span>{$_('initiative.label_stealth_rolls')}</span>
                      {#each surpriseAutoResult.stealth_rolls as sr}
                        <span>{sr.name}: {sr.stealth_total} (nat {sr.natural})</span>
                      {/each}
                      <span>vs PP:</span>
                      {#each surpriseAutoResult.perceptions as p}
                        <span>{p.name}: PP {p.passive_perception} → {p.surprised ? 'SURPRISED' : 'alert'}</span>
                      {/each}
                    </div>
                  {/if}
                </div>
              {/if}

              {#if showReactForm}
                <div class="ca-form">
                  <label class="ca-field">
                    <span>{$_('initiative.label_reaction')}</span>
                    <select bind:value={reactType}>
                      <option value="shield">Shield (+5 AC)</option>
                      <option value="counterspell">Counterspell</option>
                      <option value="opportunity_attack">Opportunity Attack</option>
                      <option value="custom">Custom</option>
                    </select>
                  </label>
                  {#if reactType === 'shield'}
                    {#if activeC.last_hit_attack_total}
                      <div class="ca-result" style="background:rgba(200,160,60,0.15)">
                        <span>Hit received: roll {activeC.last_hit_attack_total} vs AC {activeC.ac}</span>
                        <span>{$_('initiative.label_pending_damage', { values: { amount: activeC.last_hit_damage ?? 0 } })}</span>
                        {#if (activeC.last_hit_attack_total ?? 0) < activeC.ac + 5}
                          <span style="color:#2a8a2a">{$_('initiative.label_shield_negate')}</span>
                        {:else}
                          <span style="color:#8b6914">{$_('initiative.label_shield_still_lands')}</span>
                        {/if}
                      </div>
                    {:else}
                      <div class="ca-result" style="color:#8b1a1a;font-size:0.75rem">{$_('initiative.label_no_pending_hit')}</div>
                    {/if}
                  {/if}
                  {#if reactType === 'counterspell'}
                    {@const casting = combatants.find(c => c.spell_being_cast)}
                    {#if casting}
                      <div class="ca-result" style="background:rgba(200,160,60,0.15)">
                        <span>{casting.display_name} is casting {casting.spell_being_cast}</span>
                      </div>
                    {:else}
                      <div class="ca-result" style="color:#8b1a1a;font-size:0.75rem">{$_('initiative.label_no_spell_being_cast')}</div>
                    {/if}
                  {/if}
                  {#if reactType === 'custom'}
                    <label class="ca-field"><span>{$_('initiative.label_react_label')}</span><input type="text" bind:value={reactLabel} placeholder={$_('initiative.ph_react_label')} /></label>
                  {/if}
                  <button type="button" class="ca-submit" onclick={() => guarded(`react:${activeC.id}`, () => doReact(activeC))} disabled={isInFlight(`react:${activeC.id}`)}>
                    <Shield size={12} /> Use Reaction
                  </button>
                </div>
              {/if}
            {/if}
          </div>
        </div>
      {/if}

      {#if active && waitingCount > 0}
        <div class="waiting">
          <Hourglass size={12} />
          {waitingCount === 1
            ? $_('initiative.waiting_one')
            : $_('initiative.waiting_many').replace('{{n}}', String(waitingCount))}
        </div>
      {/if}

      <nav class="view-tabs">
        <button type="button" class="view-tab {view === 'roster' ? 'active' : ''}" onclick={() => view = 'roster'}>
          <ListOrdered size={13} /> {$_('initiative.tab_roster')}
        </button>
        <button type="button" class="view-tab {view === 'map' ? 'active' : ''}" onclick={() => view = 'map'}>
          <MapIcon size={13} /> {$_('initiative.tab_map')}
        </button>
      </nav>

      {#if view === 'roster'}
      <div class="flex items-center gap-2 mb-3">
        <Search size={14} class="text-neutral-500 shrink-0" />
        <input placeholder={$_('initiative.ph_filter_combatants')} bind:value={rosterSearch}
          class="flex-1 max-w-xs rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 text-sm" />
      </div>
      {#if myPending.length}
        {@const _myRollsProps = { myPending, partyChars, rolling, initBonus, onRoll: rollInitiativeFor }}
        <MyRolls {..._myRollsProps} />
      {/if}

      <!-- roster -->
      <Roster
        combatants={combatants}
        currentEnc={currentEnc}
        isActiveEncounter={active}
        isMaster={campaign().isMaster}
        allNpcs={allNpcs}
        effectsFor={effectsFor}
        hpRatio={hpRatio}
        hpColor={hpColor}
        isInFlight={isInFlight}
        guarded={guarded}
        onApplyDamage={applyDamage}
        onSetTemp={setTemp}
        onGotoTurn={gotoTurn}
        onShowEffectPanel={(c) => effectPanelCombatant = c}
        onShowStatBlock={(c) => statBlockCombatant = c}
      />

      {#if campaign().isMaster}
        <div class="add-combatant-wrap">
          <CollapsibleAdd label={`+ ${$_('initiative.add_combatant')}`} title={$_('initiative.add_combatant')} alignEnd={false}>
            {#snippet children({ close })}
              <form onsubmit={(e) => { e.preventDefault(); addCombatant(close); }} class="add-combatant-form">
                {#if allNpcs.length > 0}
                  <label class="field field-wide">
                    <span>{$_('initiative.label_npc')}</span>
                    <select value={newCombNpcId ?? ''} onchange={(e) => selectNpcForCombatant((e.currentTarget as HTMLSelectElement).value || null)}>
                      <option value="">Custom…</option>
                      {#each allNpcs as n (n.id)}<option value={n.id}>{n.name}</option>{/each}
                    </select>
                  </label>
                {/if}
                <label class="field field-wide">
                  <span>{$_('initiative.c_name')}</span>
                  <input required bind:value={newComb.display_name} />
                </label>
                <label class="field">
                  <span>{$_('initiative.c_init')}</span>
                  <input type="number" bind:value={newComb.initiative} />
                </label>
                <label class="field">
                  <span>{$_('initiative.c_hp')}</span>
                  <input type="number" bind:value={newComb.hp_max} />
                </label>
                <label class="field">
                  <span>{$_('initiative.c_ac')}</span>
                  <input type="number" bind:value={newComb.ac} />
                </label>
                <div class="field-submit">
                  <button class="btn-create">{$_('common.create')}</button>
                </div>
              </form>
            {/snippet}
          </CollapsibleAdd>
        </div>
      {/if}
      {:else}
        <!-- battle map -->
        {@const gridSize = (currentEnc.map_grid_size as number) ?? 50}
        {@const showGrid = (currentEnc.show_grid as boolean) ?? false}
        {@const gridType = (currentEnc.grid_type as string) ?? 'square'}
        {@const mapImg = currentEnc.map_image as string | null}
        {#if campaign().isMaster}
          <div class="map-toolbar">
            <MapIcon size={14} style="color:#a6855c;" />
            <span class="tb-label">{$_('initiative.map_image')}</span>
            <ImageUpload value={mapImg ?? null} kind="map" size={36} onchange={(url) => setMapImage(url)} />
            {#if mapImg}
              <button type="button" class="tb-btn" onclick={() => setMapImage(null)} disabled={isInFlight('map:setImage')}>
                <Trash2 size={12} /> {$_('initiative.map_clear')}
              </button>
            {/if}
            <span class="tb-spacer"></span>
            <label class="tb-check">
              <input type="checkbox" checked={showGrid}
                onchange={(e) => Encounters.update(selectedId!, { show_grid: (e.currentTarget as HTMLInputElement).checked }).then(loadList)} />
              <Grid size={12} /> {$_('initiative.map_show_grid')}
            </label>
            {#if showGrid}
            <label class="tb-grid-type">
              <span>{$_('initiative.map_grid_type')}</span>
              <select value={gridType}
                onchange={(e) => Encounters.update(selectedId!, { grid_type: (e.currentTarget as HTMLSelectElement).value }).then(loadList)}
                class="tb-grid-sel">
                <option value="square">{$_('initiative.map_grid_square')}</option>
                <option value="hex">{$_('initiative.map_grid_hex')}</option>
              </select>
            </label>
            <label class="tb-grid"><Grid size={12} /> {$_('initiative.map_grid')}
              <input type="number" min="20" max="200" step="2" value={gridSize}
                onchange={(e) => setGrid(+(e.currentTarget as HTMLInputElement).value)} />
            </label>
            {/if}
            <button type="button" class="tb-btn" onclick={placeAllTokens} disabled={isInFlight('map:placeAll')}>
              <UsersIcon size={12} /> {$_('initiative.token_place_all')}
            </button>
            {#if campaign().isMaster}
              <div class="tb-zone-btns">
                <span class="tb-label">{$_('initiative.label_zones')}</span>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_circle')}
                  onclick={() => createZoneOverlay('circle', 'difficult_terrain', 'rgba(139,69,19,0.25)')}>
                  <Circle size={12} />
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_cone')}
                  onclick={() => createZoneOverlay('cone', 'fire', 'rgba(255,69,0,0.25)')}>
                  <Triangle size={12} />
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_line')}
                  onclick={() => createZoneOverlay('line', 'poison', 'rgba(0,128,0,0.25)')}>
                  <Minus size={12} />
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_cube')}
                  onclick={() => createZoneOverlay('cube', 'magical_darkness', 'rgba(30,30,30,0.35)')}>
                  <Square size={12} />
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_hazard')}
                  onclick={() => createZoneOverlay('circle', 'hazard', 'rgba(200,50,50,0.35)')}>
                  ⚠
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_fog')}
                  onclick={() => createZoneOverlay('circle', 'fog_of_war', 'rgba(0,0,0,0.7)')}>
                  🌫
                </button>
                <button type="button" class="tb-btn" title={$_('initiative.title_zone_wall')}
                  onclick={() => createZoneOverlay('line', 'wall', 'rgba(74,55,40,0.6)')}>
                  🧱
                </button>
              </div>
              {#if overlays.some(o => o.zone_type === 'hazard')}
                <div class="tb-zone-btns mt-1">
                  <span class="tb-label">{$_('initiative.label_hazard')}</span>
                  <label class="ca-field"><span>Dmg</span><input type="text" bind:value={hazardDmgExpr} placeholder="1d6" style="width:5rem" /></label>
                  <select bind:value={hazardDmgType} style="font-size:0.7rem">
                    <option value="fire">Fire</option><option value="acid">Acid</option>
                    <option value="cold">Cold</option><option value="lightning">Lightning</option>
                    <option value="poison">Poison</option><option value="bludgeoning">Bludgeon</option>
                    <option value="necrotic">Necrotic</option>
                  </select>
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <div class="battle-wrap">
          <div bind:this={mapEl}
               class="battle {campaign().isMaster ? 'is-master' : ''}"
               onpointermove={onTokenDragMove}
               onpointerup={endTokenDrag}
               onpointercancel={endTokenDrag}
               role="presentation">
            {#if mapImg}
              <img src={mapImg} alt="" draggable="false" class="battle-img" />
            {:else}
              <div class="battle-empty">
                <MapIcon size={34} style="color:#a6855c;opacity:0.45;" />
                <p>{$_('initiative.map_empty')}</p>
              </div>
            {/if}
            {#if showGrid}
              {#if gridType === 'hex'}
                {@const R = gridSize / 2}
                {@const h = R * Math.sqrt(3)}
                {@const tw = gridSize * 1.5}
                <!-- Tile: width = tw (= 1.5*gridPx), height = 2h.
                     Contains 4 hex centers so the pattern tiles cleanly: -->
                <!-- Even col (x=R): two rows at y = h/2 and y = 3h/2 -->
                <!-- Odd col  (x=R+tw/2): two rows at y = h   and y = 2h (wraps to y=0) -->
                {@const hexPts = (cx: number, cy: number) => [0,1,2,3,4,5].map(i => {
                  const a = (Math.PI / 180) * (60 * i);
                  return `${cx + R * Math.cos(a)},${cy + R * Math.sin(a)}`;
                }).join(' ')}
                <svg class="grid-overlay" xmlns="http://www.w3.org/2000/svg"
                  width={mapW || 0} height={mapH || 0}>
                  <defs>
                    <pattern id="hex-pat" width={tw} height={2 * h} patternUnits="userSpaceOnUse">
                      <polygon points={hexPts(R, h/2)}       fill="none" stroke="rgba(44,24,16,0.3)" stroke-width="1"/>
                      <polygon points={hexPts(R, 3*h/2)}     fill="none" stroke="rgba(44,24,16,0.3)" stroke-width="1"/>
                      <polygon points={hexPts(R + tw/2, h)}  fill="none" stroke="rgba(44,24,16,0.3)" stroke-width="1"/>
                      <polygon points={hexPts(R + tw/2, 0)}  fill="none" stroke="rgba(44,24,16,0.3)" stroke-width="1"/>
                      <polygon points={hexPts(R + tw/2, 2*h)} fill="none" stroke="rgba(44,24,16,0.3)" stroke-width="1"/>
                    </pattern>
                  </defs>
                  <rect width="100%" height="100%" fill="url(#hex-pat)" />
                </svg>
              {:else}
                <div class="grid-overlay grid-square" style="--g: {gridSize}px;"></div>
              {/if}
            {/if}

            <!-- AoE / zone overlays -->
            {#if overlays.length > 0}
              <svg class="overlay-layer" xmlns="http://www.w3.org/2000/svg"
                style="position:absolute;inset:0;width:100%;height:100%;z-index:3;pointer-events:none;"
                viewBox="0 0 100 100" preserveAspectRatio="none">
                {#each overlays.filter((o) => o.active) as o (o.id)}
                  {#if o.zone_type === 'fog_of_war' && o.shape === 'circle'}
                    {@const r = o.radius_ft ? ftToPctX(o.radius_ft) : 10}
                    <circle cx="{o.origin_x}%" cy="{o.origin_y}%" r="{r}%"
                      fill="rgba(0,0,0,0.75)"
                      stroke="rgba(0,0,0,0.4)"
                      stroke-width="0.5" />
                  {:else if o.zone_type === 'fog_of_war' && o.shape === 'cube'}
                    {@const side = o.length_ft ? ftToPctX(o.length_ft) : 15}
                    <rect x="{o.origin_x - side / 2}%"
                      y="{o.origin_y - side / 2 * (mapW / mapH)}%"
                      width="{side}%" height="{side * (mapW / mapH)}%"
                      fill="rgba(0,0,0,0.75)"
                      stroke="rgba(0,0,0,0.4)"
                      stroke-width="0.5" />
                  {:else if o.shape === 'circle'}
                    {@const r = o.radius_ft ? ftToPctX(o.radius_ft) : 5}
                    <circle cx="{o.origin_x}%" cy="{o.origin_y}%" r="{r}%"
                      fill={o.color}
                      stroke={o.color.replace(/[\d.]+\)$/, '0.6)')}
                      stroke-width="0.2" />
                  {:else if o.zone_type === 'wall'}
                    <line x1="{o.origin_x}%" y1="{o.origin_y}%"
                      x2="{o.end_x ?? (o.origin_x + 10)}%" y2="{o.end_y ?? o.origin_y}%"
                      stroke="#4a3728" stroke-width="1.2" stroke-linecap="round"
                      stroke-dasharray="0" />
                  {:else if o.shape === 'cone'}
                    {@const len = o.length_ft ? ftToPctX(o.length_ft) : 5}
                    {@const ang = (o.angle_deg ?? 0) * (Math.PI / 180)}
                    {@const spread = 53.13 * (Math.PI / 180)}
                    {@const p1x = o.origin_x}
                    {@const p1y = o.origin_y}
                    {@const p2x = o.origin_x + len * Math.cos(ang - spread / 2)}
                    {@const p2y = o.origin_y + len * Math.sin(ang - spread / 2) * (mapW / mapH)}
                    {@const p3x = o.origin_x + len * Math.cos(ang + spread / 2)}
                    {@const p3y = o.origin_y + len * Math.sin(ang + spread / 2) * (mapW / mapH)}
                    <polygon points="{p1x},{p1y} {p2x},{p2y} {p3x},{p3y}"
                      fill={o.color}
                      stroke={o.color.replace(/[\d.]+\)$/, '0.6)')}
                      stroke-width="0.2" />
                  {:else if o.shape === 'line'}
                    {@const ex = o.end_x ?? o.origin_x}
                    {@const ey = o.end_y ?? o.origin_y}
                    {@const w = o.width_ft ? ftToPctX(o.width_ft) : 0.5}
                    <line x1="{o.origin_x}%" y1="{o.origin_y}%" x2="{ex}%" y2="{ey}%"
                      stroke={o.color} stroke-width="{w}%" stroke-linecap="round" />
                  {:else if o.shape === 'cube'}
                    {@const side = o.length_ft ? ftToPctX(o.length_ft) : 5}
                    <rect x="{o.origin_x - side / 2}%"
                      y="{o.origin_y - side / 2 * (mapW / mapH)}%"
                      width="{side}%" height="{side * (mapW / mapH)}%"
                      fill={o.color}
                      stroke={o.color.replace(/[\d.]+\)$/, '0.6)')}
                      stroke-width="0.2" />
                  {:else if o.shape === 'polygon'}
                    {#if o.points && o.points.length > 0}
                      <polygon points={o.points.map((p: {x:number;y:number}) => `${p.x},${p.y}`).join(' ')}
                        fill={o.color}
                        stroke={o.color.replace(/[\d.]+\)$/, '0.6)')}
                        stroke-width="0.2" />
                    {/if}
                  {/if}
                  {#if o.label}
                    <text x="{o.origin_x}%" y="{o.origin_y - 1}%" text-anchor="middle"
                      font-size="1.5" fill="rgba(255,255,255,0.9)" font-weight="bold"
                      style="text-shadow:0 0 2px rgba(0,0,0,0.8);">{o.label}</text>
                  {/if}
                {/each}
              </svg>
            {/if}

            <!-- movement arrow — local only, shown only to the dragger -->
            {#if dragId && dragStartPct && dragCurrentPct}
              <svg class="move-arrow-svg" xmlns="http://www.w3.org/2000/svg"
                width={mapW || 0} height={mapH || 0}>
                <defs>
                  <filter id="arrow-glow">
                    <feGaussianBlur stdDeviation="2" result="blur" />
                    <feMerge><feMergeNode in="blur"/><feMergeNode in="SourceGraphic"/></feMerge>
                  </filter>
                  <marker id="arrowhead" markerWidth="10" markerHeight="8" refX="9" refY="4" orient="auto">
                    <polygon points="0 0, 10 4, 0 8" fill="#f7e2a5" />
                  </marker>
                </defs>
                {#if mapEl}
                  {@const r = mapEl.getBoundingClientRect()}
                  {@const draggingC = combatants.find((cb) => cb.id === dragId)}
                  {@const spd = draggingC ? charSpeed(draggingC) : 30}
                  {@const dashFt = draggingC ? dashBonusFt(draggingC) : 0}
                  {@const forcedFt = draggingC ? forcedMovementFt(draggingC) : 0}
                  {@const g2 = currentEnc ? (currentEnc.map_grid_size as number) ?? 50 : 50}
                  {@const speedPx = maxMovePx(spd + dashFt, g2)}
                  {@const forcedPx = maxMovePx(forcedFt, g2)}
                  {@const totalMaxPx = isFinite(speedPx) ? speedPx + (isFinite(forcedPx) ? forcedPx : 0) : Infinity}
                  {@const capActive = !campaign().isMaster && currentEnc?.status === 'active' && draggingC?.ref_type === 'character'}
                  {@const curDistPx = distPx(dragCurrentPct.x, dragCurrentPct.y, dragStartPct.x, dragStartPct.y, r.width, r.height)}
                  {@const arrowEnd = (capActive && isFinite(totalMaxPx) && curDistPx > totalMaxPx)
                    ? clampToRange(dragCurrentPct.x, dragCurrentPct.y, dragStartPct.x, dragStartPct.y, totalMaxPx, r.width, r.height)
                    : dragCurrentPct}
                  <!-- normal speed (+ dash bonus) range circle -->
                  {#if capActive && isFinite(speedPx)}
                    <circle
                      cx="{(dragStartPct.x / 100) * r.width}"
                      cy="{(dragStartPct.y / 100) * r.height}"
                      r="{speedPx}"
                      fill="rgba(201,168,76,0.06)"
                      stroke="rgba(201,168,76,0.7)"
                      stroke-width="2"
                      stroke-dasharray="8 4" />
                  {/if}
                  <!-- forced movement bonus range circle -->
                  {#if capActive && forcedFt > 0 && isFinite(forcedPx)}
                    <circle
                      cx="{(dragStartPct.x / 100) * r.width}"
                      cy="{(dragStartPct.y / 100) * r.height}"
                      r="{speedPx + forcedPx}"
                      fill="rgba(76,168,201,0.04)"
                      stroke="rgba(76,168,201,0.55)"
                      stroke-width="2"
                      stroke-dasharray="4 6" />
                  {/if}
                  <!-- dark outline for contrast -->
                  <line
                    x1="{dragStartPct.x}%" y1="{dragStartPct.y}%"
                    x2="{arrowEnd.x}%" y2="{arrowEnd.y}%"
                    stroke="rgba(0,0,0,0.55)" stroke-width="6"
                    stroke-linecap="round" />
                  <!-- arrow line -->
                  <line
                    x1="{dragStartPct.x}%" y1="{dragStartPct.y}%"
                    x2="{arrowEnd.x}%" y2="{arrowEnd.y}%"
                    stroke="#f7e2a5" stroke-width="3.5"
                    stroke-linecap="round"
                    filter="url(#arrow-glow)"
                    marker-end="url(#arrowhead)" />
                {/if}
                <!-- start dot -->
                <circle cx="{dragStartPct.x}%" cy="{dragStartPct.y}%" r="6" fill="none" stroke="rgba(0,0,0,0.4)" stroke-width="3" />
                <circle cx="{dragStartPct.x}%" cy="{dragStartPct.y}%" r="6" fill="#f7e2a5" filter="url(#arrow-glow)" />
              </svg>
            {/if}

            {#each tokensOnMap as c (c.id)}
              {@const isMine = canMoveToken(c)}
              {@const isActiveT = rolledCombs[currentEnc.turn_index as number]?.id === c.id && currentEnc.status === 'active'}
              {@const dragging = dragId === c.id}
              {@const portrait = c.portrait_url as string | null | undefined}
              {@const effs = effectsFor(c)}
              {@const displayPos = dragging
                ? { x: c.token_x as number, y: c.token_y as number }
                : (showGrid && mapW > 0 && mapH > 0
                    ? snapPos(c.token_x as number, c.token_y as number, currentEnc)
                    : { x: c.token_x as number, y: c.token_y as number })}
              {@const hasAura = campaign().isMaster && effs.length > 0}
              <div class="tok-wrap {dragging ? 'dragging' : ''} {isActiveT ? 'is-active' : ''} {hasAura ? 'has-aura' : ''}"
                   role="application"
                   style="left: {displayPos.x}%; top: {displayPos.y}%;"
                   oncontextmenu={(e) => { e.preventDefault(); ctxMenu = { x: e.clientX, y: e.clientY, combatant: c }; }}>
                {#if hasAura}
                  <div class="tok-aura" title={effs.map(e => e.name).join(', ')}></div>
                {/if}
                {#if portrait}
                  <button type="button"
                    class="tok img {c.ref_type === 'character' ? 'player' : 'npc'} {isMine ? 'movable' : ''}"
                    onpointerdown={(e) => startTokenDrag(e, c)}
                    aria-label={c.display_name as string}>
                    <img src={portrait} alt="" draggable="false" />
                  </button>
                {:else}
                  <button type="button"
                    class="tok {c.ref_type === 'character' ? 'player' : 'npc'} {isMine ? 'movable' : ''}"
                    style="background: {tokenBg(c)};"
                    onpointerdown={(e) => startTokenDrag(e, c)}
                    aria-label={c.display_name as string}>
                    {tokenInitial(c)}
                  </button>
                {/if}
                <span class="tok-label">
                  {c.display_name}
                  {#if tokenMovedThisRound(c)}<span class="tok-moved">✓</span>{/if}
                </span>
                {#if effs.length > 0}
                  <span class="tok-effects">
                    {#each effs.slice(0, 4) as eff (eff.id)}
                      <EffectBadge effect={eff} size="sm" />
                    {/each}
                    {#if effs.length > 4}<span class="tok-more">+{effs.length - 4}</span>{/if}
                  </span>
                {/if}
                {#if (c.hp_max as number) > 0}
                  <span class="tok-hp">
                    <span class="tok-hp-bar" style="width: {hpRatio(c) * 100}%; background: {hpColor(hpRatio(c))};"></span>
                  </span>
                {/if}
                {#if isMine}
                  <div class="tok-upload" role="group" onpointerdown={(e) => e.stopPropagation()}
                    title={$_('initiative.token_image_upload')}>
                    <ImageUpload value={portrait ?? null} kind="pin" size={22}
                      onchange={(url) => saveTokenImage(c, url)} />
                  </div>
                {/if}
                {#if campaign().isMaster}
                  <button type="button" class="tok-remove"
                    title={$_('initiative.token_remove_from_map')}
                    disabled={isInFlight(`token:off:${c.id}`)}
                    onclick={(e) => { e.stopPropagation(); guarded(`token:off:${c.id}`, () => placeTokenAtCentre(c, false)); }}>
                    <X size={10} />
                  </button>
                {/if}
              </div>
            {/each}
          </div>

          {#if ctxMenu}
            {@const cm = ctxMenu}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div class="ctx-backdrop" onclick={() => ctxMenu = null} oncontextmenu={() => ctxMenu = null}>
              <div class="ctx-menu" style="left: {cm.x}px; top: {cm.y}px;">
                <span class="ctx-title">{cm.combatant.display_name}</span>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showAttackForm = true; ctxMenu = null; }}>🗡️ Attack</button>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showDmgForm = true; ctxMenu = null; }}>💥 Damage</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`dodge:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`dodge:ctx:${cm.combatant.id}`, async () => { doDodge(cm.combatant); ctxMenu = null; })}>🛡️ Dodge</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`disengage:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`disengage:ctx:${cm.combatant.id}`, async () => { doDisengage(cm.combatant, false); ctxMenu = null; })}>🏃 Disengage</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`dash:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`dash:ctx:${cm.combatant.id}`, async () => { doDash(cm.combatant); ctxMenu = null; })}>💨 Dash</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`hide:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`hide:ctx:${cm.combatant.id}`, async () => { doHide(cm.combatant); ctxMenu = null; })}>👻 Hide</button>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showCastForm = true; ctxMenu = null; }}>🔮 Cast Spell</button>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showGrappleForm = true; ctxMenu = null; }}>🤝 Grapple</button>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showShoveForm = true; ctxMenu = null; }}>💪 Shove</button>
                <button type="button" class="ctx-item"
                  onclick={() => { setActiveForm(cm.combatant); showHelpForm = true; ctxMenu = null; }}>🤲 Help</button>
                <div class="ctx-divider"></div>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`standup:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`standup:ctx:${cm.combatant.id}`, async () => { doStandUp(cm.combatant); ctxMenu = null; })}>🔝 Stand Up</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`deathsave:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`deathsave:ctx:${cm.combatant.id}`, async () => { doDeathSave(cm.combatant); ctxMenu = null; })}>💀 Death Save</button>
                <button type="button" class="ctx-item"
                  disabled={isInFlight(`heal:ctx:${cm.combatant.id}`)}
                  onclick={() => guarded(`heal:ctx:${cm.combatant.id}`, async () => { setActiveForm(cm.combatant); ctxMenu = null; doHeal(cm.combatant); })}>❤️‍🩹 Heal</button>
                {#if campaign().isMaster}
                  <div class="ctx-divider"></div>
                  <button type="button" class="ctx-item"
                    disabled={isInFlight(`token:off:${cm.combatant.id}`)}
                    onclick={() => guarded(`token:off:${cm.combatant.id}`, async () => { placeTokenAtCentre(cm.combatant, false); ctxMenu = null; })}>🗑️ Remove from Map</button>
                {/if}
              </div>
            </div>
          {/if}

          {#if campaign().isMaster && overlays.length > 0}
            <div class="overlay-list">
              <span class="ol-title">{$_('initiative.label_zones_aoe')}</span>
              {#each overlays.filter((o) => o.active) as o (o.id)}
                <div class="ol-item">
                  <span class="ol-dot" style="background:{o.color};"></span>
                  <span class="ol-name">{o.label || o.shape}</span>
                  <span class="ol-meta">{o.zone_type || o.kind}</span>
                  <button type="button" class="ol-del" disabled={isInFlight(`overlay:remove:${o.id}`)} onclick={() => guarded(`overlay:remove:${o.id}`, () => removeOverlay(o.id))} title={$_('initiative.title_remove_overlay')}>
                    <X size={10} />
                  </button>
                </div>
              {/each}
            </div>
          {/if}
          <footer class="battle-legend">
            <span class="legend-entry"><span class="leg-dot player"></span> {$_('initiative.legend_player')}</span>
            <span class="legend-entry"><span class="leg-dot npc"></span> {$_('initiative.legend_npc')}</span>
            <span class="legend-hint">
              {campaign().isMaster ? $_('initiative.map_master_drag_hint') : $_('initiative.map_drag_hint')}
            </span>
          </footer>

          {#if campaign().isMaster}
            {@const offMap = combatants.filter((c) => !c.token_on_map)}
            {#if offMap.length}
              <section class="tray">
                <header class="tray-head"><UsersIcon size={12} /> {$_('initiative.token_to_map')}</header>
                <div class="tray-list">
                  {#each offMap as c (c.id)}
                    <button type="button" class="tray-chip" disabled={isInFlight(`token:on:${c.id}`)} onclick={() => guarded(`token:on:${c.id}`, () => placeTokenAtCentre(c, true))}>
                      {#if c.portrait_url}
                        <span class="tok tray-tok img {c.ref_type === 'character' ? 'player' : 'npc'}">
                          <img src={c.portrait_url as string} alt="" draggable="false" />
                        </span>
                      {:else}
                        <span class="tok tray-tok {c.ref_type === 'character' ? 'player' : 'npc'}" style="background: {tokenBg(c)};">
                          {tokenInitial(c)}
                        </span>
                      {/if}
                      <span>{c.display_name}</span>
                    </button>
                  {/each}
                </div>
              </section>
            {/if}
          {/if}
        </div>
      {/if}
    {/if}
  {/if}
</section>

{#if effectPanelCombatant}
  <EffectPanel
    combatant={effectPanelCombatant}
    effects={effectsFor(effectPanelCombatant)}
    encounterId={selectedId!}
    isMaster={campaign().isMaster}
    isMe={effectPanelCombatant.ref_type === 'character' && partyChars.find((p) => p.id === effectPanelCombatant?.character_id)?.owner_id === auth.user?.id}
    onchange={loadEffects}
    onclose={() => effectPanelCombatant = null}
  />
{/if}

{#if statBlockCombatant}
  {@const npc = allNpcs.find((n) => n.id === statBlockCombatant?.npc_id)}
  {#if npc?.stats}
    <Modal onClose={() => statBlockCombatant = null} title={npc.name}>
      <NpcStatBlock stats={npc.stats} />
    </Modal>
  {/if}
{/if}

{#if showCombatLog && currentEnc}
  <CombatLog
    encounter={currentEnc}
    combatants={combatants}
    events={combatEvents}
    loading={combatEventsLoading}
    onClose={() => showCombatLog = false}
  />
{/if}

<!-- Floating Dice Roller -->
<DiceRoller cid={cid} />

<style>
  .council { max-width: 90rem; margin: 0 auto; padding: 1rem 1.25rem; }
  @media (max-width: 639px) { .council { padding: 0.5rem 0.6rem; } }

  /* header */
  .council-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 1rem;
  }
  .hdr-icon, .hdr-right { display: flex; justify-content: center; align-items: center; }
  .audio-toggle {
    font-size: 1rem;
    background: none;
    border: none;
    cursor: pointer;
    opacity: 0.7;
  }
  .audio-toggle:hover { opacity: 1; }
  .hdr-center { text-align: center; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.6rem, 3vw, 2.4rem);
    font-weight: 900;
    letter-spacing: 0.08em;
    color: #2c1810;
    line-height: 1;
  }
  .hdr-sub {
    margin-top: 0.25rem;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.85rem;
    color: #6d510f;
  }
  .fleuron { color: #8b6914; margin: 0 0.4rem; font-style: normal; }

  .rule {
    height: 3px;
    margin: 0.85rem 0 1rem;
    background: linear-gradient(90deg, transparent 0%, #8b6914 8%, #c9a84c 50%, #8b6914 92%, transparent 100%);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦";
    position: absolute; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    color: #6d510f;
    background: #f4e4c1;
    padding: 0 0.5rem;
    font-size: 0.9rem;
  }

  .empty { text-align: center; padding: 3rem 1rem; font-style: italic; color: #8b6355; }
  .err { color: #c95a5a; margin-top: 0.5rem; font-size: 0.85rem; }

  /* encounter tabs — moved to lib/combat/EncounterTabs.svelte */

  /* banner — moved to lib/combat/Banner.svelte */

  /* spotlight */
  .spotlight {
    display: flex; align-items: center; gap: 0.75rem;
    margin-top: 0.85rem;
    padding: 0.75rem 1rem;
    border: 2px solid #c9a84c;
    border-radius: 0.45rem;
    background:
      radial-gradient(circle at 20% 30%, rgba(201,168,76,0.35) 0%, transparent 60%),
      linear-gradient(180deg, rgba(244,228,193,0.1), transparent 55%),
      #2c1810;
    box-shadow: 0 0 0 1px rgba(201,168,76,0.25), 0 6px 16px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,248,220,0.15);
    color: #f7e2a5;
  }
  .spot-crown { flex: none; }
  .spot-body { flex: 1; min-width: 0; }
  .spot-title {
    font-family: 'IM Fell English SC', serif;
    font-size: 1.1rem;
    letter-spacing: 0.08em;
    color: #f7e2a5;
  }
  .spot-stats {
    margin-top: 0.25rem;
    font-family: 'Special Elite', monospace;
    font-size: 0.75rem;
    color: rgba(244,228,193,0.7);
    display: inline-flex; align-items: center; gap: 0.3rem; flex-wrap: wrap;
  }
  .spot-stats .sep { opacity: 0.5; }
  .stat-badge {
    display: inline-flex; align-items: center;
    padding: 0.05rem 0.3rem;
    border-radius: 0.15rem;
    font-size: 0.55rem;
    font-family: 'Cinzel', serif;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    margin-left: 0.15rem;
  }
  .stat-badge.adv { background: #2a5a2a; color: #90ee90; }
  .stat-badge.dis { background: #5a2a2a; color: #ff9999; }
  .stat-badge.sadv { background: #2a4a5a; color: #90d0ee; }
  .stat-badge.sdis { background: #4a2a5a; color: #d090ee; }
  .stat-badge.slow { background: #5a4a2a; color: #f0d090; }
  .stat-badge.incap { background: #3a3a3a; color: #ccc; }
  .stat-badge.res { background: #5a3a1a; color: #f0c080; }
  .stat-badge.imm { background: #1a3a5a; color: #90c0f0; }
  .stat-badge.exhaust { background: #5a3a3a; color: #ffcccc; }
  .stat-badge.pp { background: #3a3a5a; color: #c0c0f0; }

  .combat-actions {
    margin-top: 0.4rem;
    display: flex; align-items: center; gap: 0.3rem; flex-wrap: wrap;
  }
  .ca-btn {
    display: inline-flex; align-items: center; gap: 0.25rem;
    padding: 0.2rem 0.5rem;
    border-radius: 0.25rem;
    background: rgba(44,24,16,0.6);
    color: #c9a84c;
    border: 1px solid rgba(201,168,76,0.35);
    font-family: 'Cinzel', serif;
    font-size: 0.65rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .ca-btn:hover { background: rgba(44,24,16,0.85); }
  .ca-btn-sm {
    font-size: 0.6rem;
    padding: 0.15rem 0.35rem;
    letter-spacing: 0.05em;
  }
  .ca-form {
    margin-top: 0.4rem;
    padding: 0.5rem;
    background: rgba(20,12,6,0.6);
    border: 1px solid rgba(201,168,76,0.25);
    border-radius: 0.3rem;
    display: flex; flex-direction: column; gap: 0.35rem;
  }
  .ca-row {
    display: flex; align-items: flex-end; gap: 0.35rem; flex-wrap: wrap;
  }
  .ca-field {
    display: flex; flex-direction: column; gap: 0.1rem;
    font-size: 0.6rem;
    color: #8b6914;
    font-family: 'Cinzel', serif;
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .ca-field input, .ca-field select {
    background: rgba(244,228,193,0.9);
    border: 1px solid #6d510f;
    border-radius: 0.2rem;
    padding: 0.2rem 0.35rem;
    font-size: 0.7rem;
    color: #2c1810;
    font-family: 'Special Elite', monospace;
  }
  .ca-check {
    display: inline-flex; align-items: center; gap: 0.2rem;
    font-size: 0.6rem;
    color: #c9a84c;
    cursor: pointer;
  }
  .ca-submit {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.3rem 0.6rem;
    border-radius: 0.25rem;
    background: #6d510f;
    color: #f4e4c1;
    border: 1px solid #8b6914;
    font-family: 'Cinzel', serif;
    font-size: 0.65rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
    align-self: flex-start;
  }
  .ca-submit:hover { background: #8b6914; }
  .ca-submit.dmg { background: #8b2020; border-color: #b84040; }
  .ca-submit.dmg:hover { background: #b84040; }
  .ca-submit.heal { background: #206b20; border-color: #40b840; }
  .ca-submit.heal:hover { background: #40b840; }
  .ca-result {
    margin-top: 0.2rem;
    padding: 0.3rem 0.5rem;
    border-radius: 0.2rem;
    background: rgba(44,24,16,0.5);
    font-size: 0.7rem;
    color: #f4e4c1;
    font-family: 'Special Elite', monospace;
    display: flex; flex-direction: column; gap: 0.15rem;
  }
  .ca-result.hit { border-left: 3px solid #40b840; }
  .ca-result.miss { border-left: 3px solid #b84040; }
  .ca-crit { color: #ffcc00; font-weight: bold; font-size: 0.8rem; }
  .ca-conc { color: #ff6666; font-style: italic; }

  .death-save-banner {
    margin: 0.5rem 0;
    padding: 0.6rem 0.8rem;
    border-radius: 0.35rem;
    background: linear-gradient(135deg, rgba(80,20,20,0.9), rgba(44,24,16,0.95));
    border: 1.5px solid #8b4040;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    align-items: center;
  }
  .ds-title { color: #ff9999; font-family: 'Cinzel', serif; font-weight: bold; font-size: 0.9rem; }
  .ds-track { display: flex; align-items: center; gap: 0.3rem; font-size: 0.75rem; color: #f4e4c1; }
  .ds-dot { color: #555; font-size: 1rem; }
  .ds-dot.ds-fail { color: #555; }
  .ds-dot.ds-filled { color: #40b840; }
  .ds-dot.ds-fail.ds-filled { color: #b84040; }

  .opp-prompts {
    margin-top: 0.5rem;
    display: flex; flex-direction: column; gap: 0.3rem;
  }
  .opp-prompt {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.7rem;
    background: rgba(139,26,26,0.2);
    border: 1px dashed rgba(201,168,76,0.5);
    border-radius: 0.3rem;
    color: #f4e4c1;
    font-size: 0.75rem;
  }
  .opp-btn {
    padding: 0.2rem 0.5rem;
    border-radius: 0.2rem;
    background: #8b1a1a;
    color: #f4e4c1;
    border: 1px solid #b84040;
    font-family: 'Cinzel', serif;
    font-size: 0.6rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    cursor: pointer;
    margin-left: auto;
  }
  .opp-btn:hover { background: #b84040; }
  .opp-btn.skip { background: #3a3a3a; border-color: #666; }
  .opp-btn.skip:hover { background: #555; }

  /* diff/flank panel — moved to lib/combat/Banner.svelte */

  .waiting {
    margin-top: 0.5rem;
    padding: 0.4rem 0.7rem;
    border-radius: 0.3rem;
    background: rgba(139,26,26,0.15);
    border: 1px dashed rgba(201,168,76,0.4);
    color: #c9a84c;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.8rem;
    display: inline-flex; align-items: center; gap: 0.4rem;
  }

  /* my-rolls card — moved to lib/combat/MyRolls.svelte */

  /* roster — moved to lib/combat/Roster.svelte */

  .add-combatant-wrap { margin-top: 1rem; }
  .add-combatant-form {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 0.7rem;
    align-items: end;
  }
  @media (max-width: 640px) {
    .add-combatant-form { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  }
  .field { display: flex; flex-direction: column; gap: 0.25rem; min-width: 0; }
  .field.field-wide { grid-column: span 3; }
  @media (max-width: 640px) {
    .field.field-wide { grid-column: span 2; }
  }
  .field > span {
    font-family: 'IM Fell English SC', serif;
    font-size: 0.7rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .field > input {
    width: 100%;
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important;
    border-radius: 0.3rem !important;
    padding: 0.4rem 0.6rem !important;
    font-family: 'Crimson Text', serif;
    font-size: 0.9rem;
  }
  .field > input:focus {
    border-color: #c9a84c !important;
    box-shadow: 0 0 0 2px rgba(201,168,76,0.25) !important;
    outline: none;
  }
  .field-submit {
    grid-column: span 6;
    display: flex; justify-content: flex-end;
  }
  @media (max-width: 640px) { .field-submit { grid-column: span 2; } }
  .btn-create {
    padding: 0.5rem 1.4rem;
    border-radius: 0.35rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.8rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.35);
  }
  .btn-create:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }

  /* view tabs */
  .view-tabs {
    display: inline-flex;
    gap: 0;
    margin: 0.85rem 0 0.75rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    overflow: hidden;
  }
  .view-tab {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.9rem;
    background: rgba(244,228,193,0.7);
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    border: 0;
    border-right: 1px solid rgba(139,105,20,0.35);
  }
  .view-tab:last-child { border-right: 0; }
  .view-tab:hover { background: rgba(201,168,76,0.3); color: #2c1810; }
  .view-tab.active {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55);
  }

  /* battle map toolbar */
  .map-toolbar {
    display: flex; align-items: center; gap: 0.65rem; flex-wrap: wrap;
    padding: 0.55rem 0.85rem;
    margin-bottom: 0.85rem;
    border: 1.5px solid rgba(139,105,20,0.5);
    border-radius: 0.35rem;
    background: rgba(244,228,193,0.85);
  }
  .tb-label {
    font-family: 'IM Fell English SC', serif;
    font-size: 0.75rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .tb-spacer { flex: 1; }
  .tb-btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.3rem 0.65rem;
    border-radius: 0.3rem;
    background: #3a2313;
    color: #f4e4c1;
    border: 1px solid #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .tb-btn:hover { background: #4e3909; }
  .tb-zone-btns {
    display: inline-flex; align-items: center; gap: 0.25rem;
    margin-left: 0.5rem; padding-left: 0.5rem;
    border-left: 1px solid #6d510f;
  }
  .tb-zone-btns .tb-label {
    color: #8b6914; font-family: 'Cinzel', serif;
    font-size: 0.65rem; letter-spacing: 0.08em; text-transform: uppercase;
    margin-right: 0.25rem;
  }
  .tb-check {
    display: inline-flex; align-items: center; gap: 0.35rem;
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
  .overlay-list {
    margin-top: 0.4rem;
    padding: 0.4rem 0.6rem;
    background: rgba(30,18,10,0.7);
    border: 1px solid #6d510f;
    border-radius: 0.35rem;
    display: flex; flex-wrap: wrap; align-items: center; gap: 0.35rem;
  }
  .ol-title {
    color: #8b6914; font-family: 'Cinzel', serif;
    font-size: 0.6rem; letter-spacing: 0.08em; text-transform: uppercase;
    margin-right: 0.3rem;
  }
  .ol-item {
    display: inline-flex; align-items: center; gap: 0.3rem;
    background: #3a2313;
    border: 1px solid #6d510f;
    border-radius: 0.25rem;
    padding: 0.15rem 0.4rem;
    font-size: 0.65rem;
    color: #f4e4c1;
  }
  .ol-dot { width: 0.5rem; height: 0.5rem; border-radius: 50%; display: inline-block; }
  .ol-name { font-family: 'Cinzel', serif; }
  .ol-meta { color: #8b6914; font-size: 0.55rem; text-transform: uppercase; }
  .ol-del {
    background: none; border: none; color: #b84040; cursor: pointer; padding: 0; margin-left: 0.15rem;
    display: inline-flex; align-items: center;
  }
  .ol-del:hover { color: #e06060; }
  .tb-grid-type {
    display: inline-flex; align-items: center; gap: 0.35rem;
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .tb-grid-sel {
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important;
    border-radius: 0.25rem !important;
    padding: 0.2rem 0.4rem !important;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
  }
  .tb-grid {
    display: inline-flex; align-items: center; gap: 0.35rem;
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .tb-grid input {
    width: 3.5rem;
    padding: 0.2rem 0.4rem !important;
    background: #f4e4c1 !important;
    color: #2c1810 !important;
    border: 1px solid rgba(139,105,20,0.5) !important;
    border-radius: 0.25rem !important;
    font-family: 'Special Elite', monospace;
    font-size: 0.8rem;
  }

  /* battle map */
  .battle-wrap {
    border: 2px solid #8b6914;
    border-radius: 0.45rem;
    background:
      linear-gradient(180deg, rgba(139,105,20,0.08), transparent 45%),
      #241810;
    box-shadow: 0 10px 26px rgba(0,0,0,0.55);
    overflow: hidden;
  }
  .battle {
    position: relative;
    width: 100%;
    /* height follows image natural ratio */
    min-height: 20rem;
    overflow: hidden;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    user-select: none;
    margin: 0.55rem 0;
    border-radius: 0.3rem;
    box-shadow: inset 0 0 0 1px rgba(139,105,20,0.35), inset 0 0 60px rgba(139,105,20,0.35);
  }
  .battle-img {
    display: block;
    width: 100%;
    height: auto;
    pointer-events: none;
  }
  .grid-overlay, .move-arrow-svg {
    position: absolute; inset: 0;
    pointer-events: none;
  }
  .move-arrow-svg { z-index: 5; }
  .grid-square {
    background-image:
      linear-gradient(rgba(44,24,16,0.3) 1px, transparent 1px),
      linear-gradient(90deg, rgba(44,24,16,0.3) 1px, transparent 1px);
    background-size: var(--g, 50px) var(--g, 50px);
  }
  .battle-empty {
    position: absolute; inset: 0;
    display: grid; place-items: center;
    gap: 0.6rem;
    font-family: 'IM Fell English SC', serif;
    font-style: italic;
    color: #8b6355;
  }
  .battle-empty p { margin: 0; }

  .tok-wrap {
    position: absolute;
    /* Anchor the token circle's center at (left, top) — shift up by half
       the circle height (1.2rem) so the circle is centered, with the
       label rendered below the anchor point. */
    transform: translate(-50%, -1.2rem);
    display: flex; flex-direction: column; align-items: center;
    gap: 0.2rem;
  }
  .tok-wrap.dragging { z-index: 30; }
  .tok-wrap.is-active .tok {
    box-shadow: 0 0 0 3px #c9a84c, 0 0 12px rgba(201,168,76,0.8), 0 3px 8px rgba(0,0,0,0.55);
  }
  .tok-wrap.has-aura .tok { z-index: 2; }
  .tok-aura {
    position: absolute; top: 50%; left: 50%;
    width: 3.2rem; height: 3.2rem;
    transform: translate(-50%, -1.8rem);
    border-radius: 9999px;
    border: 2px solid rgba(201,168,76,0.5);
    background: rgba(201,168,76,0.06);
    animation: aura-pulse 3s ease-in-out infinite;
    pointer-events: none;
    z-index: 1;
  }
  @keyframes aura-pulse {
    0%, 100% { opacity: 0.4; transform: translate(-50%, -1.8rem) scale(1); }
    50% { opacity: 0.8; transform: translate(-50%, -1.8rem) scale(1.08); }
  }
  .tok {
    width: 2.4rem; height: 2.4rem;
    border-radius: 9999px;
    display: grid; place-items: center;
    color: #f4e4c1;
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1rem;
    border: 2px solid #2c1810;
    box-shadow: 0 3px 8px rgba(0,0,0,0.55), inset 0 2px 0 rgba(255,248,220,0.25);
    touch-action: none;
    user-select: none;
    padding: 0;
  }
  .tok.player { outline: 2px solid #c9a84c; outline-offset: 1px; }
  .tok.npc    { outline: 2px solid #8b1a1a; outline-offset: 1px; }
  .tok.movable { cursor: grab; }
  .tok.movable:active { cursor: grabbing; }
  .tok.img { padding: 0; background: #1a0f08 !important; overflow: hidden; }
  .tok.img img { width: 100%; height: 100%; object-fit: cover; border-radius: 9999px; pointer-events: none; }
  .tok-upload {
    position: absolute;
    left: -0.35rem; top: -0.35rem;
    width: 1.3rem; height: 1.3rem;
    border-radius: 9999px;
    display: grid; place-items: center;
    background: rgba(26,15,8,0.9);
    border: 1px solid #c9a84c;
    opacity: 0;
    transition: opacity 0.1s;
    overflow: hidden;
  }
  .tok-wrap:hover .tok-upload { opacity: 1; }
  .tok-upload :global(button),
  .tok-upload :global(.drop) {
    width: 100% !important;
    height: 100% !important;
    border-radius: 9999px !important;
  }
  .tok-moved { color: #6b8a4f; margin-left: 0.25rem; font-style: normal; }
  .tok-label {
    padding: 0.1rem 0.45rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.68rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: #f4e4c1;
    background: rgba(26,15,8,0.88);
    border: 1px solid rgba(201,168,76,0.45);
    border-radius: 0.25rem;
    white-space: nowrap;
    pointer-events: none;
  }
  .tok-hp {
    display: block;
    width: 2.4rem;
    height: 3px;
    background: rgba(26,15,8,0.6);
    border-radius: 9999px;
    overflow: hidden;
  }
  .tok-hp-bar { display: block; height: 100%; transition: width 0.2s; }
  .tok-remove {
    position: absolute;
    top: -0.3rem; right: -0.3rem;
    width: 1rem; height: 1rem;
    border-radius: 9999px;
    display: grid; place-items: center;
    background: #8b1a1a;
    color: #f4e4c1;
    border: 1px solid #4e0a0a;
    opacity: 0;
    transition: opacity 0.1s;
  }
  .tok-wrap:hover .tok-remove { opacity: 1; }

  .battle-legend {
    display: flex; align-items: center; gap: 1rem;
    flex-wrap: wrap;
    padding: 0.55rem 0.85rem;
    border-top: 1px dashed rgba(201,168,76,0.35);
    background: rgba(26,15,8,0.35);
    color: #f4e4c1;
    font-family: 'Cinzel', serif;
    font-size: 0.72rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .legend-entry { display: inline-flex; align-items: center; gap: 0.4rem; }
  .leg-dot {
    width: 0.7rem; height: 0.7rem;
    border-radius: 9999px;
    border: 1.5px solid #2c1810;
  }
  .leg-dot.player { background: #6d510f; outline: 2px solid #c9a84c; outline-offset: 1px; }
  .leg-dot.npc    { background: #8b1a1a; outline: 2px solid #8b1a1a; outline-offset: 1px; }
  .legend-hint {
    margin-left: auto;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    text-transform: none;
    letter-spacing: 0;
    color: rgba(244,228,193,0.7);
  }

  .tray {
    padding: 0.55rem 0.85rem;
    border-top: 1px dashed rgba(201,168,76,0.35);
    background: rgba(26,15,8,0.35);
  }
  .tray-head {
    display: flex; align-items: center; gap: 0.35rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #c9a84c;
    margin-bottom: 0.4rem;
  }
  .tray-list { display: flex; gap: 0.4rem; flex-wrap: wrap; }
  .tray-chip {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.25rem 0.55rem 0.25rem 0.3rem;
    background: rgba(244,228,193,0.1);
    border: 1px solid rgba(201,168,76,0.35);
    border-radius: 9999px;
    color: #f4e4c1;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.06em;
  }
  .tray-chip:hover { background: rgba(201,168,76,0.2); }
  .tray-tok { width: 1.5rem; height: 1.5rem; font-size: 0.7rem; border-width: 1px; }

  /* effect badges — moved to lib/combat/Roster.svelte */

  .tok-effects {
    position: absolute; bottom: -0.4rem; left: 50%; transform: translateX(-50%);
    display: inline-flex; align-items: center; gap: 0.15rem;
    z-index: 5;
  }
  .tok-more {
    font-size: 0.5rem; color: #f4e4c1; font-weight: 700;
    background: rgba(0,0,0,0.5); border-radius: 999px;
    padding: 0.02rem 0.25rem;
  }

  .ctx-backdrop {
    position: fixed; inset: 0; z-index: 1000;
    background: transparent;
  }
  .ctx-menu {
    position: fixed; z-index: 1001;
    background: #2c1810; border: 1px solid #c9a84c;
    border-radius: 0.4rem; padding: 0.3rem 0;
    min-width: 10rem;
    box-shadow: 0 8px 24px rgba(0,0,0,0.5);
  }
  .ctx-title {
    display: block; padding: 0.35rem 0.7rem;
    font-family: 'Cinzel', serif; font-weight: 700;
    font-size: 0.75rem; color: #c9a84c;
    border-bottom: 1px solid rgba(201,168,76,0.3);
    margin-bottom: 0.2rem;
  }
  .ctx-item {
    display: block; width: 100%;
    padding: 0.3rem 0.7rem;
    background: none; border: none;
    color: #f4e4c1; font-size: 0.75rem;
    text-align: left; cursor: pointer;
    transition: background 0.15s;
  }
  .ctx-item:hover { background: rgba(201,168,76,0.15); }
  .ctx-divider {
    border-top: 1px solid rgba(201,168,76,0.2);
    margin: 0.2rem 0.4rem;
  }

  /* action economy */
  .action-chips {
    display: flex; flex-wrap: wrap; gap: 0.35rem;
    margin-top: 0.4rem; align-items: center;
  }
  .act-chip {
    display: inline-flex; align-items: center; gap: 0.2rem;
    padding: 0.2rem 0.5rem; border-radius: 0.25rem;
    font-size: 0.7rem; font-weight: 600;
    background: rgba(74,124,89,0.2); color: #7abf8a;
    border: 1px solid rgba(74,124,89,0.4);
    cursor: pointer; line-height: 1;
  }
  .act-chip:disabled { cursor: default; opacity: 0.7; }
  .act-chip.used {
    background: rgba(100,100,100,0.15); color: #8a8a8a;
    border-color: rgba(100,100,100,0.3);
    text-decoration: line-through;
  }
  .act-chip.move-chip {
    background: rgba(107,123,138,0.15); color: #a0b0c0;
    border-color: rgba(107,123,138,0.3);
    cursor: default;
  }
  .legendary-dots {
    display: inline-flex; align-items: center; gap: 0.15rem;
  }
  .ldot {
    background: transparent; border: none; cursor: pointer;
    font-size: 0.75rem; opacity: 1; filter: none; padding: 0;
    line-height: 1;
  }
  .ldot:disabled { cursor: default; }
  .ldot.spent { opacity: 0.25; filter: grayscale(1); }
  .lr-chip {
    background: rgba(201,168,76,0.15); color: #c9a84c;
    border-color: rgba(201,168,76,0.4);
  }

  /* act-indicators — moved to lib/combat/Roster.svelte */

  /* lair-chip — moved to lib/combat/Banner.svelte */

  /* Dice panel — moved to lib/combat/DiceRoller.svelte */
</style>
