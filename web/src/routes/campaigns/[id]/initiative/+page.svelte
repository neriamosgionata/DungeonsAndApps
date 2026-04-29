<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { Encounters, Characters, Dice } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import { _ } from 'svelte-i18n';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import { Dice5, Play, SkipBack, SkipForward, Square, Crown, Heart, Shield, Swords, Hourglass, X, Trash2, Map as MapIcon, Grid, ListOrdered, Users as UsersIcon } from '@lucide/svelte';

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);
  let encs = $state<Record<string, unknown>[]>([]);
  let selectedId = $state<string | null>(null);
  let combatants = $state<Record<string, unknown>[]>([]);
  let error = $state('');

  let newName = $state('');
  let newComb = $state({ display_name: '', initiative: 10, hp_max: 10, hp_current: 10, ac: 10 });
  let partyChars = $state<Record<string, unknown>[]>([]);
  let rolling = $state<Record<string, boolean>>({});

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

  async function loadList() {
    encs = await Encounters.list(cid);
    if (!selectedId && encs.length) selectedId = encs[0].id as string;
    if (selectedId) combatants = await Encounters.combatants.list(selectedId);
  }

  async function loadParty() {
    try { partyChars = await Characters.list(cid); } catch { partyChars = []; }
  }

  onMount(loadList);
  onMount(loadParty);

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
      if (t === 'combatant_moved') {
        const id = (ev as Record<string, unknown>).id as string;
        const nx = (ev as Record<string, unknown>).x as number;
        const ny = (ev as Record<string, unknown>).y as number;
        if (id !== dragId) {
          combatants = combatants.map((c) => c.id === id ? { ...c, token_x: nx, token_y: ny, token_on_map: true } : c);
        }
        return;
      }
      if (t.startsWith('combatant_') || t === 'next_turn' || t === 'encounter_started' || t === 'encounter_ended' || t === 'encounter_updated') {
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
      await Encounters.combatants.add(selectedId, { ...newComb, ref_type: 'npc', npc_id: null });
      newComb = { display_name: '', initiative: 10, hp_max: 10, hp_current: 10, ac: 10 };
      close();
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function rollInitiativeFor(comb: Record<string, unknown>) {
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

  async function start() { if (selectedId) { await Encounters.start(selectedId); await loadList(); } }
  async function end()   { if (selectedId) { await Encounters.end(selectedId); await loadList(); } }
  async function next()  { if (selectedId) { await Encounters.nextTurn(selectedId); await loadList(); } }
  async function prev()  { if (selectedId) { await Encounters.prevTurn(selectedId); await loadList(); } }
  async function gotoTurn(idx: number) { if (selectedId) { await Encounters.gotoTurn(selectedId, idx); await loadList(); } }

  function initBonus(sheet: Record<string, unknown>): number {
    const explicit = sheet.initiative as number | undefined;
    if (typeof explicit === 'number') return explicit;
    const ab = (sheet.abilities ?? {}) as Record<string, number | undefined>;
    const dex = ab.dex ?? 10;
    return Math.floor((dex - 10) / 2);
  }

  async function applyDamage(c: Record<string, unknown>, delta: number) {
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
  async function setTemp(c: Record<string, unknown>, v: number) {
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

  function snapPos(x: number, y: number, enc: Record<string, unknown> | undefined): { x: number; y: number } {
    if (!enc || !mapEl) return { x, y };
    if (!(enc.show_grid as boolean)) return { x, y };
    const r = mapEl.getBoundingClientRect();
    const g = (enc.map_grid_size as number) ?? 50;
    if ((enc.grid_type as string) === 'hex') return snapToHex(x, y, g, r.width, r.height);
    return snapToSquare(x, y, g, r.width, r.height);
  }

  // ---- movement cap ----
  function charSpeed(c: Record<string, unknown>): number {
    if (c.ref_type !== 'character') return Infinity;
    const ch = partyChars.find((p) => p.id === c.character_id);
    if (!ch) return 30;
    const sheet = (ch.sheet as Record<string, unknown> | undefined) ?? {};
    return (sheet.speed as number | undefined) ?? 30;
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

  function canMoveToken(c: Record<string, unknown>): boolean {
    if (campaign().isMaster) return true;
    if (c.ref_type !== 'character') return false;
    const ch = partyChars.find((p) => p.id === c.character_id);
    if (!ch || ch.owner_id !== auth.user?.id) return false;
    // Before combat starts: free placement anywhere.
    if (currentEnc?.status !== 'active') return true;
    // Combat active: once per round, speed-capped.
    const movedRound = c.token_moved_round as number | null | undefined;
    const currentRound = (currentEnc?.round as number | undefined) ?? 0;
    if (movedRound != null && movedRound >= currentRound) return false;
    return true;
  }

  function tokenMovedThisRound(c: Record<string, unknown>): boolean {
    if (campaign().isMaster) return false;
    const movedRound = c.token_moved_round as number | null | undefined;
    const currentRound = (currentEnc?.round as number | undefined) ?? 0;
    return movedRound != null && movedRound >= currentRound && !!c.token_on_map;
  }

  function startTokenDrag(ev: PointerEvent, c: Record<string, unknown>) {
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
      const g = (currentEnc.map_grid_size as number) ?? 50;
      const maxPx = maxMovePx(speed, g);
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
        const maxPx = maxMovePx(charSpeed(moved), g);
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
      try { await Encounters.combatants.move(id, final.x, final.y); }
      catch (e) { error = (e as Error).message; await loadList(); }
    }
  }

  async function setMapImage(url: string | null) {
    if (!selectedId) return;
    try {
      if (url) await Encounters.update(selectedId, { map_image: url });
      else await Encounters.update(selectedId, { clear_map_image: true });
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function setGrid(n: number) {
    if (!selectedId) return;
    try { await Encounters.update(selectedId, { map_grid_size: n }); await loadList(); }
    catch (e) { error = (e as Error).message; }
  }

  async function placeTokenAtCentre(c: Record<string, unknown>, on: boolean) {
    if (!campaign().isMaster) return;
    try {
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
    } catch (e) { error = (e as Error).message; }
  }

  async function placeAllTokens() {
    if (!campaign().isMaster) return;
    // Arrange party on the left, NPCs on the right, evenly spaced.
    const players = combatants.filter((c) => c.ref_type === 'character');
    const npcs    = combatants.filter((c) => c.ref_type !== 'character');
    async function layout(list: Record<string, unknown>[], xPct: number) {
      if (list.length === 0) return;
      const step = 80 / Math.max(list.length, 1);
      for (let i = 0; i < list.length; i++) {
        const y = 10 + step * (i + 0.5);
        await Encounters.combatants.update(list[i].id as string, { token_x: xPct, token_y: y, token_on_map: true });
      }
    }
    try {
      await layout(players, 20);
      await layout(npcs, 80);
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  async function saveTokenImage(c: Record<string, unknown>, url: string | null) {
    try {
      if (url) await Encounters.combatants.update(c.id as string, { token_image: url });
      else await Encounters.combatants.update(c.id as string, { clear_token_image: true });
      await loadList();
    } catch (e) { error = (e as Error).message; }
  }

  function tokenBg(c: Record<string, unknown>): string {
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

  function tokenInitial(c: Record<string, unknown>): string {
    return ((c.display_name as string) || '?').trim().charAt(0).toUpperCase();
  }

  const tokensOnMap = $derived(combatants.filter((c) => c.token_on_map && c.token_x != null && c.token_y != null));

  function hpRatio(c: Record<string, unknown>): number {
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
    <div class="hdr-icon"><Swords size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('initiative.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('initiative.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
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

  {#if encs.length === 0}
    <p class="empty">{$_('initiative.empty')}</p>
  {:else}
    <!-- encounter tabs -->
    <nav class="enc-tabs">
      {#each encs as e (e.id)}
        <button type="button"
          class="enc-tab {selectedId === e.id ? 'active' : ''}"
          onclick={() => selectedId = e.id as string}>
          <span>{e.name}</span>
          <span class="enc-status status-{e.status}">{$_(`initiative.status_${e.status}`)}</span>
        </button>
      {/each}
    </nav>

    {#if selectedId && currentEnc}
      {@const active = currentEnc.status === 'active'}
      {@const activeC = rolledCombs[currentEnc.turn_index as number]}
      {@const total = combatants.length}

      <!-- banner -->
      <div class="banner">
        <div class="banner-title">
          <Swords size={16} />
          <span>{currentEnc.name}</span>
        </div>
        <div class="banner-meta">
          <span class="meta-chip"><Crown size={12} /> {$_('initiative.round')} <b>{currentEnc.round}</b></span>
          <span class="meta-chip"><Hourglass size={12} /> {$_('initiative.turn_of').replace('{{n}}', String((currentEnc.turn_index as number) + 1)).replace('{{total}}', String(total))}</span>
        </div>
        {#if campaign().isMaster}
          <div class="banner-actions">
            {#if currentEnc.status === 'planned'}
              <button onclick={start} class="btn btn-start"
                disabled={pendingCombatants.length > 0}
                title={pendingCombatants.length > 0
                  ? pendingCombatants.map((c) => c.display_name).join(', ')
                  : undefined}>
                <Play size={14} /> {$_('initiative.start')}
                {#if pendingCombatants.length > 0}
                  <span class="start-pending">({pendingCombatants.length})</span>
                {/if}
              </button>
            {:else if active}
              <button onclick={prev} class="btn btn-ghost" title={$_('initiative.prev_turn_title')}><SkipBack size={14} /> {$_('initiative.prev')}</button>
              <button onclick={next} class="btn btn-next" title={$_('initiative.next_turn_title')}><SkipForward size={14} /> {$_('initiative.next')}</button>
              <button onclick={end} class="btn btn-end"><Square size={14} /> {$_('initiative.end')}</button>
            {/if}
            <button onclick={removeEncounter} class="btn btn-danger" title={$_('initiative.delete')}>
              <Trash2 size={14} />
            </button>
          </div>
        {/if}
      </div>

      {#if active && activeC}
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
                <span><Shield size={11} /> {activeC.ac}</span>
              {/if}
            </div>
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
      {#if myPending.length}
        <section class="my-rolls">
          <header class="my-rolls-head"><Dice5 size={14} /> <span>{$_('initiative.my_pending')}</span></header>
          <ul>
            {#each myPending as c (c.id)}
              {@const ch = partyChars.find((p) => p.id === c.character_id)}
              {@const sh = (ch?.sheet ?? {}) as Record<string, unknown>}
              {@const bonus = initBonus(sh)}
              <li class="my-roll">
                <span class="my-roll-name">{c.display_name}</span>
                <span class="my-roll-bonus">init {bonus >= 0 ? `+${bonus}` : bonus}</span>
                <button onclick={() => rollInitiativeFor(c)} disabled={rolling[c.character_id as string]} class="my-roll-btn">
                  <Dice5 size={14} />
                  {rolling[c.character_id as string] ? '…' : `1d20${bonus >= 0 ? '+' : ''}${bonus}`}
                </button>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      <!-- roster -->
      <section class="roster">
        <div class="roster-head">
          <span class="col-order">{$_('initiative.col_order')}</span>
          <span class="col-name">{$_('initiative.col_name')}</span>
          <span class="col-init">{$_('initiative.col_init')}</span>
          <span class="col-hp">{$_('initiative.col_hp')}</span>
          <span class="col-ac">{$_('initiative.col_ac')}</span>
          <span class="col-actions">{$_('initiative.col_actions')}</span>
        </div>

        {#each combatants as c, i (c.id)}
          {@const pending = !c.initiative_rolled}
          {@const isActive = i === currentEnc.turn_index && active && !pending}
          {@const r = hpRatio(c)}
          <div class="row {isActive ? 'row-active' : ''} {pending ? 'row-pending' : ''}">
            <span class="col-order">
              {#if isActive}<Crown size={14} style="color:#c9a84c;" />{:else}{pending ? '—' : c.turn_order}{/if}
            </span>
            <span class="col-name">
              {#if campaign().isMaster && active && !pending}
                <button onclick={() => gotoTurn(i)} class="name-btn">{c.display_name}</button>
              {:else}
                <span class="name-plain">{c.display_name}</span>
              {/if}
              {#if pending}
                <span class="awaiting">{$_('initiative.awaiting_init')}</span>
              {/if}
            </span>
            <span class="col-init">{pending ? '—' : c.initiative}</span>
            <span class="col-hp">
              {#if campaign().isMaster && c.ref_type !== 'character'}
                <div class="hp-ctl">
                  <button type="button" onclick={() => applyDamage(c, -1)} class="hp-btn hp-dmg" title={$_('initiative.col_hp')}>−</button>
                  <span class="hp-val" style="color: {hpColor(r)};">
                    {c.hp_current}<span class="hp-sep">/{c.hp_max}</span>
                    {#if (c.temp_hp as number) > 0}<span class="temp-hp" title={$_('initiative.temp_hp_title')}>+{c.temp_hp}</span>{/if}
                  </span>
                  <button type="button" onclick={() => applyDamage(c, 1)} class="hp-btn hp-heal" title={$_('initiative.col_hp')}>+</button>
                  <label class="temp-wrap" title={$_('initiative.temp_hp_title')}>
                    <span class="temp-tag">{$_('initiative.temp_short')}</span>
                    <input type="number" min="0" value={(c.temp_hp as number | undefined) ?? 0}
                      onchange={(e) => setTemp(c, +(e.currentTarget as HTMLInputElement).value)}
                      class="temp-input" />
                  </label>
                </div>
                <div class="hp-bar"><span style="width: {r * 100}%; background: {hpColor(r)};"></span></div>
              {:else if (c.hp_max as number) > 0}
                <div>
                  <span class="hp-val" style="color: {hpColor(r)};">{c.hp_current}<span class="hp-sep">/{c.hp_max}</span>{#if (c.temp_hp as number) > 0}<span class="temp-hp">+{c.temp_hp}</span>{/if}</span>
                  <div class="hp-bar"><span style="width: {r * 100}%; background: {hpColor(r)};"></span></div>
                </div>
              {:else}
                <span class="hp-hidden">—</span>
              {/if}
            </span>
            <span class="col-ac">{#if campaign().isMaster || (c.ac as number) > 0}<Shield size={11} /> {c.ac}{:else}—{/if}</span>
            <span class="col-actions">
              {#if campaign().isMaster}
                {#if active}
                  <button title={$_('initiative.jump_to_turn')} onclick={() => gotoTurn(i)} class="icon-btn"><Play size={13} /></button>
                {/if}
                <button title={$_('initiative.remove_combatant')} class="icon-btn danger"
                  onclick={() => Encounters.combatants.delete(c.id as string).then(loadList)}><X size={14} /></button>
              {/if}
            </span>
          </div>
        {/each}
      </section>

      {#if campaign().isMaster}
        <div class="add-combatant-wrap">
          <CollapsibleAdd label={`+ ${$_('initiative.add_combatant')}`} title={$_('initiative.add_combatant')} alignEnd={false}>
            {#snippet children({ close })}
              <form onsubmit={(e) => { e.preventDefault(); addCombatant(close); }} class="add-combatant-form">
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
            <MapIcon size={14} style="color:#8b6914;" />
            <span class="tb-label">{$_('initiative.map_image')}</span>
            <ImageUpload value={mapImg ?? null} kind="map" size={36} onchange={(url) => setMapImage(url)} />
            {#if mapImg}
              <button type="button" class="tb-btn" onclick={() => setMapImage(null)}>
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
            <button type="button" class="tb-btn" onclick={placeAllTokens}>
              <UsersIcon size={12} /> {$_('initiative.token_place_all')}
            </button>
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
                <MapIcon size={34} style="color:#8b6914;opacity:0.45;" />
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
                  {@const g2 = currentEnc ? (currentEnc.map_grid_size as number) ?? 50 : 50}
                  {@const maxPx = maxMovePx(spd, g2)}
                  {@const capActive = !campaign().isMaster && currentEnc?.status === 'active' && draggingC?.ref_type === 'character'}
                  {@const curDistPx = distPx(dragCurrentPct.x, dragCurrentPct.y, dragStartPct.x, dragStartPct.y, r.width, r.height)}
                  {@const arrowEnd = (capActive && isFinite(maxPx) && curDistPx > maxPx)
                    ? clampToRange(dragCurrentPct.x, dragCurrentPct.y, dragStartPct.x, dragStartPct.y, maxPx, r.width, r.height)
                    : dragCurrentPct}
                  <!-- range circle (as px-radius in SVG user space, no axis distortion) -->
                  {#if capActive && isFinite(maxPx)}
                    <circle
                      cx="{(dragStartPct.x / 100) * r.width}"
                      cy="{(dragStartPct.y / 100) * r.height}"
                      r="{maxPx}"
                      fill="rgba(201,168,76,0.06)"
                      stroke="rgba(201,168,76,0.7)"
                      stroke-width="2"
                      stroke-dasharray="8 4" />
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
              {@const displayPos = dragging
                ? { x: c.token_x as number, y: c.token_y as number }
                : (showGrid && mapW > 0 && mapH > 0
                    ? snapPos(c.token_x as number, c.token_y as number, currentEnc)
                    : { x: c.token_x as number, y: c.token_y as number })}
              <div class="tok-wrap {dragging ? 'dragging' : ''} {isActiveT ? 'is-active' : ''}"
                   style="left: {displayPos.x}%; top: {displayPos.y}%;">
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
                    onclick={(e) => { e.stopPropagation(); placeTokenAtCentre(c, false); }}>
                    <X size={10} />
                  </button>
                {/if}
              </div>
            {/each}
          </div>
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
                    <button type="button" class="tray-chip" onclick={() => placeTokenAtCentre(c, true)}>
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

<style>
  .council { max-width: 90rem; margin: 0 auto; padding: 1rem 1.25rem; }

  /* header */
  .council-head {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 1rem;
  }
  .hdr-icon, .hdr-right { display: flex; justify-content: center; }
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

  /* encounter tabs */
  .enc-tabs { display: flex; flex-wrap: wrap; gap: 0.4rem; margin-bottom: 1rem; }
  .enc-tab {
    display: inline-flex; align-items: center; gap: 0.5rem;
    padding: 0.4rem 0.9rem;
    border-radius: 0.35rem;
    border: 1.5px solid #4e3909;
    background: rgba(139,105,20,0.1);
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.78rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .enc-tab:hover { background: rgba(201,168,76,0.25); color: #2c1810; }
  .enc-tab.active {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 4px rgba(0,0,0,0.45);
  }
  .enc-status {
    font-size: 0.6rem;
    padding: 0.1rem 0.4rem;
    border-radius: 9999px;
    border: 1px solid currentColor;
    letter-spacing: 0.12em;
  }
  .status-planned { color: #8b6355; }
  .status-active  { color: #6b8a4f; background: rgba(107,138,79,0.15); }
  .status-ended   { color: #a93535; }
  .enc-tab.active .enc-status { color: #1a0f08; border-color: rgba(26,15,8,0.5); }

  /* banner */
  .banner {
    display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap;
    padding: 0.75rem 1rem;
    border: 2px solid #8b6914;
    border-radius: 0.45rem;
    background:
      linear-gradient(180deg, rgba(139,105,20,0.15), transparent 55%),
      #241810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.08), 0 4px 10px rgba(0,0,0,0.45);
    color: #f4e4c1;
  }
  .banner-title {
    display: inline-flex; align-items: center; gap: 0.55rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 1.1rem;
    color: #f7e2a5;
    letter-spacing: 0.08em;
  }
  .banner-title :global(svg) { color: #c9a84c; }
  .banner-meta { display: inline-flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .meta-chip {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.2rem 0.55rem;
    border-radius: 9999px;
    border: 1px solid rgba(201,168,76,0.4);
    background: rgba(201,168,76,0.12);
    color: #c9a84c;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
  }
  .meta-chip b { color: #f4e4c1; font-weight: 700; }
  .banner-actions { margin-left: auto; display: inline-flex; gap: 0.4rem; flex-wrap: wrap; }

  .btn {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.4rem 0.75rem;
    border-radius: 0.35rem;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    border: 1px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.2), 0 2px 4px rgba(0,0,0,0.35);
  }
  .btn-start { background-image: linear-gradient(180deg, #8aa86f, #5f7a48 60%, #3a5226); color: #f4e4c1; border-color: #3a5226; }
  .btn-start:hover:not(:disabled) { background-image: linear-gradient(180deg, #a5c489, #6f8e53 60%, #415c2b); }
  .btn-start:disabled { opacity: 0.5; cursor: not-allowed; }
  .start-pending { font-size: 0.7rem; opacity: 0.8; }
  .btn-next {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
  }
  .btn-next:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .btn-ghost { background: rgba(244,228,193,0.08); color: #f4e4c1; border-color: #6d510f; }
  .btn-ghost:hover { background: rgba(244,228,193,0.18); }
  .btn-end { background-image: linear-gradient(180deg, #b35454, #8b1a1a 60%, #4e0a0a); color: #f4e4c1; border-color: #4e0a0a; }
  .btn-end:hover { background-image: linear-gradient(180deg, #c56868, #a03030 60%, #5e1212); }
  .btn-danger { padding: 0.4rem 0.55rem; background: rgba(139,26,26,0.2); color: #c95a5a; border-color: rgba(139,26,26,0.55); }
  .btn-danger:hover { background: rgba(139,26,26,0.35); color: #f4e4c1; }

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

  /* my-rolls card */
  .my-rolls {
    margin-top: 1rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: 0 4px 10px rgba(0,0,0,0.25);
    overflow: hidden;
  }
  .my-rolls-head {
    display: flex; align-items: center; gap: 0.4rem;
    padding: 0.55rem 0.9rem;
    border-bottom: 1px dashed rgba(139,105,20,0.4);
    font-family: 'IM Fell English SC', serif;
    font-size: 0.8rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .my-rolls ul { margin: 0; padding: 0; list-style: none; }
  .my-roll {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.55rem 0.9rem;
    border-top: 1px dashed rgba(139,105,20,0.2);
    color: #2c1810;
  }
  .my-roll:first-child { border-top: 0; }
  .my-roll-name { font-family: 'Cinzel', serif; font-weight: 700; flex: 1; }
  .my-roll-bonus {
    font-family: 'Special Elite', monospace;
    font-size: 0.78rem;
    color: #6d510f;
  }
  .my-roll-btn {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.35rem 0.75rem;
    border-radius: 0.35rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.35);
  }
  .my-roll-btn:hover { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .my-roll-btn:disabled { opacity: 0.5; }

  /* roster */
  .roster {
    margin-top: 1rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: 0 4px 10px rgba(0,0,0,0.25);
    overflow: hidden;
  }
  .roster-head, .row {
    display: grid;
    grid-template-columns: 2.2rem 1fr 3.2rem 11rem 3.2rem 4.5rem;
    align-items: center;
    gap: 0.5rem;
    padding: 0.55rem 0.9rem;
  }
  @media (max-width: 700px) {
    .roster-head, .row { grid-template-columns: 2rem 1fr 3rem 8rem 2.5rem 3.2rem; font-size: 0.78rem; }
  }
  .roster-head {
    background: rgba(139,105,20,0.18);
    border-bottom: 1px solid rgba(139,105,20,0.45);
    font-family: 'IM Fell English SC', serif;
    font-size: 0.72rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .row {
    border-top: 1px dashed rgba(139,105,20,0.2);
    color: #2c1810;
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
  }
  .row:first-child { border-top: 0; }
  .row.row-pending { opacity: 0.65; }
  .row.row-active {
    background:
      linear-gradient(90deg, rgba(201,168,76,0.18), transparent 60%);
    box-shadow: inset 4px 0 0 #c9a84c;
  }

  .col-order { text-align: center; font-family: 'IM Fell English SC', serif; color: #6d510f; font-weight: 800; }
  .col-init { text-align: center; font-family: 'Special Elite', monospace; font-size: 0.95rem; font-weight: 700; color: #6d510f; }
  .col-ac {
    text-align: center;
    display: inline-flex; align-items: center; justify-content: center; gap: 0.3rem;
    font-family: 'Special Elite', monospace;
    color: #6d510f;
  }
  .col-actions { display: inline-flex; justify-content: flex-end; gap: 0.2rem; }
  .col-name { min-width: 0; display: flex; align-items: center; gap: 0.5rem; }
  .name-btn {
    background: transparent;
    color: #2c1810;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    text-align: left;
    padding: 0;
  }
  .name-btn:hover { color: #6d510f; text-decoration: underline; }
  .name-plain { font-family: 'Cinzel', serif; font-weight: 700; }
  .awaiting {
    display: inline-block;
    padding: 0.05rem 0.4rem;
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #8b1a1a;
    background: rgba(139,26,26,0.12);
    border: 1px dashed rgba(139,26,26,0.4);
    border-radius: 9999px;
  }

  /* HP */
  .col-hp { min-width: 0; }
  .hp-ctl { display: inline-flex; align-items: center; gap: 0.3rem; flex-wrap: nowrap; }
  .hp-btn {
    width: 1.4rem; height: 1.4rem;
    border-radius: 0.25rem;
    display: grid; place-items: center;
    border: 1px solid #4e3909;
    font-weight: 700;
    font-size: 0.75rem;
  }
  .hp-dmg  { background: rgba(139,26,26,0.2); color: #8b1a1a; }
  .hp-dmg:hover  { background: rgba(139,26,26,0.35); color: #f4e4c1; }
  .hp-heal { background: rgba(107,138,79,0.2); color: #4a6530; }
  .hp-heal:hover { background: rgba(107,138,79,0.4); color: #f4e4c1; }
  .hp-val { font-family: 'Special Elite', monospace; font-weight: 700; font-variant-numeric: tabular-nums; }
  .hp-sep { opacity: 0.55; font-weight: 500; }
  .temp-hp {
    margin-left: 0.25rem;
    font-size: 0.7rem;
    color: #2f6058;
  }
  .temp-wrap {
    display: inline-flex; align-items: stretch;
    margin-left: 0.3rem;
    border: 1px solid rgba(139,105,20,0.5);
    border-radius: 0.25rem;
    overflow: hidden;
  }
  .temp-tag {
    padding: 0 0.35rem;
    display: grid; place-items: center;
    background: #8b6914;
    color: #f4e4c1;
    font-family: 'Cinzel', serif;
    font-size: 0.55rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  .temp-input {
    width: 2.2rem;
    border: 0 !important;
    background: rgba(244,228,193,0.85) !important;
    color: #2c1810 !important;
    padding: 0.1rem 0.25rem !important;
    border-radius: 0 !important;
    font-size: 0.72rem !important;
    text-align: center;
  }
  .hp-bar {
    margin-top: 0.25rem;
    width: 100%;
    height: 4px;
    background: rgba(78,57,9,0.2);
    border-radius: 9999px;
    overflow: hidden;
    border: 1px solid rgba(78,57,9,0.25);
  }
  .hp-bar span { display: block; height: 100%; transition: width 0.2s, background 0.2s; }

  .icon-btn {
    width: 1.75rem; height: 1.75rem;
    display: grid; place-items: center;
    border-radius: 0.3rem;
    color: #6d510f;
    background: rgba(139,105,20,0.08);
    border: 1px solid transparent;
  }
  .icon-btn:hover { background: rgba(201,168,76,0.25); border-color: rgba(139,105,20,0.4); color: #2c1810; }
  .icon-btn.danger { color: #8b1a1a; background: rgba(139,26,26,0.08); }
  .icon-btn.danger:hover { background: rgba(139,26,26,0.25); color: #f4e4c1; }

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
  .tb-check {
    display: inline-flex; align-items: center; gap: 0.35rem;
    color: #6d510f;
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    cursor: pointer;
  }
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
</style>
