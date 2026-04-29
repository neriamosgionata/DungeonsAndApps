<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { Encounters, Characters, Dice } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import Stepper from '$lib/components/Stepper.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import { _ } from 'svelte-i18n';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { Dice5, Play, SkipBack, SkipForward, Square, Crown, Heart, Shield } from '@lucide/svelte';
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

  // chars that are combatants but haven't rolled initiative yet
  const pendingCombatants = $derived(combatants.filter((c) => c.ref_type === 'character' && !c.initiative_rolled));
  const myPending = $derived(pendingCombatants.filter((c) => {
    const ch = partyChars.find((p) => p.id === c.character_id);
    return ch && ch.owner_id === auth.user?.id;
  }));

  let off: (() => void) | undefined;
  onMount(() => {
    off = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t.startsWith('combatant_') || t === 'next_turn' || t === 'encounter_started' || t === 'encounter_ended') {
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
    // delta > 0 = heal; delta < 0 = damage. Damage hits temp_hp first.
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
</script>

<section class="mx-auto max-w-5xl px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-xl font-semibold">{$_('initiative.title')}</h2>
    {#if campaign().isMaster}
      <CollapsibleAdd label={`+ ${$_('initiative.new_encounter')}`} title={$_('initiative.new_encounter')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="flex gap-2">
            <input required placeholder={$_('initiative.encounter_name_ph')} bind:value={newName}
              class="flex-1 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>

  {#if encs.length}
    <div class="mt-4 flex gap-2 flex-wrap">
      {#each encs as e (e.id)}
        <button onclick={() => selectedId = e.id as string}
          class="rounded-md px-3 py-1 text-sm {selectedId === e.id ? 'bg-violet-600 text-white' : 'bg-neutral-800'}">
          {e.name} <span class="opacity-60">({e.status})</span>
        </button>
      {/each}
    </div>
  {/if}

  {#if selectedId && currentEnc}
    {@const active = currentEnc.status === 'active'}
    {@const rolledCombs = combatants.filter((c) => c.initiative_rolled)}
    {@const activeC = rolledCombs[currentEnc.turn_index as number]}
    {@const waitingCount = combatants.length - rolledCombs.length}
    <div class="mt-6 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
      <div class="flex items-center gap-3 flex-wrap">
        <span class="font-semibold">{currentEnc.name}</span>
        <span class="inline-flex items-center gap-2 text-sm" style="color:#8b6355;">
          <span class="inline-flex items-center gap-1"><Crown size={14} /> round {currentEnc.round}</span>
          <span>·</span>
          <span>turn {(currentEnc.turn_index as number) + 1}/{combatants.length}</span>
        </span>
        <div class="ml-auto flex gap-2">
          {#if campaign().isMaster}
            {#if currentEnc.status === 'planned'}
              <button onclick={start} class="inline-flex items-center gap-1.5 rounded bg-emerald-600 px-3 py-1 text-sm text-white">
                <Play size={14} /> Start
              </button>
            {:else if active}
              <button onclick={prev} class="inline-flex items-center gap-1.5 rounded bg-neutral-800 px-3 py-1 text-sm"
                title="Previous turn"><SkipBack size={14} /> Prev</button>
              <button onclick={next} class="inline-flex items-center gap-1.5 rounded bg-violet-600 px-3 py-1 text-sm text-white"
                title="Next turn"><SkipForward size={14} /> Next</button>
              <button onclick={end} class="inline-flex items-center gap-1.5 rounded bg-red-600 px-3 py-1 text-sm text-white">
                <Square size={14} /> End
              </button>
            {/if}
            <button onclick={removeEncounter} class="rounded bg-red-600 px-3 py-1 text-sm text-white">
              {$_('initiative.delete')}
            </button>
          {/if}
        </div>
      </div>

      {#if active && activeC}
        <div class="mt-3 flex items-center gap-3 rounded-md px-3 py-2"
          style="background:rgba(201,168,76,0.15); border:1px solid rgba(201,168,76,0.4);">
          <Crown size={16} style="color:#c9a84c;" />
          <div class="flex-1 min-w-0">
            <div class="font-semibold truncate">It's <span style="color:#8b6914;">{activeC.display_name}</span>'s turn</div>
            <div class="text-xs" style="color:#8b6355;">
              init {activeC.initiative} · <Heart size={11} class="inline" /> {activeC.hp_current}/{activeC.hp_max}{(activeC.temp_hp as number) > 0 ? ` (+${activeC.temp_hp})` : ''} · <Shield size={11} class="inline" /> {activeC.ac}
            </div>
          </div>
        </div>
      {/if}
      {#if active && waitingCount > 0}
        <div class="mt-2 text-xs italic" style="color:#8b1a1a;">
          Waiting for {waitingCount} initiative roll{waitingCount === 1 ? '' : 's'}…
        </div>
      {/if}
    </div>

    {#if myPending.length}
      <div class="mt-4 rounded-lg border border-neutral-800 bg-neutral-900 p-3">
        <h3 class="text-sm font-semibold">{$_('initiative.roll_party') ?? 'Roll initiative'}</h3>
        <ul class="mt-2 space-y-1">
          {#each myPending as c (c.id)}
            {@const ch = partyChars.find((p) => p.id === c.character_id)}
            {@const sh = (ch?.sheet ?? {}) as Record<string, unknown>}
            {@const bonus = initBonus(sh)}
            <li class="flex items-center gap-2 text-sm">
              <span class="flex-1 truncate">
                <span class="font-semibold">{c.display_name}</span>
                <span class="text-xs" style="color:#8b6355;">
                  · init {bonus >= 0 ? `+${bonus}` : bonus}
                </span>
              </span>
              <button onclick={() => rollInitiativeFor(c)} disabled={rolling[c.character_id as string]}
                class="inline-flex items-center gap-1.5 rounded bg-violet-600 px-3 py-1 text-xs text-white disabled:opacity-50">
                <Dice5 size={14} /> {rolling[c.character_id as string] ? '…' : `1d20${bonus >= 0 ? '+' : ''}${bonus}`}
              </button>
            </li>
          {/each}
        </ul>
      </div>
    {/if}
    {#if campaign().isMaster}
    <div class="mt-4">
      <CollapsibleAdd label={`+ ${$_('initiative.add_combatant')}`} title={$_('initiative.add_combatant')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); addCombatant(close); }} class="grid grid-cols-4 gap-2">
            <input required placeholder={$_('initiative.c_name')} bind:value={newComb.display_name} class="col-span-2 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <input type="number" placeholder={$_('initiative.c_init')} bind:value={newComb.initiative} class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <input type="number" placeholder={$_('initiative.c_hp')} bind:value={newComb.hp_max} class="rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <input type="number" placeholder={$_('initiative.c_ac')} bind:value={newComb.ac} class="col-span-2 rounded bg-neutral-900 border border-neutral-700 px-2 py-1" />
            <div class="col-span-2 flex justify-end">
              <button class="rounded bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    </div>
    {/if}

    <table class="mt-4 w-full text-sm">
      <thead class="text-neutral-400"><tr>
        <th class="text-left py-2 w-8"></th>
        <th class="text-left py-2 w-8">#</th><th class="text-left">Name</th><th>Init</th><th>HP</th><th>AC</th><th></th>
      </tr></thead>
      <tbody>
        {#each combatants as c, i (c.id)}
          {@const pending = !c.initiative_rolled}
          {@const isActive = i === currentEnc.turn_index && active && !pending}
          <tr class="border-t border-neutral-800 {isActive ? 'bg-amber-500/15' : ''} {pending ? 'opacity-60' : ''}"
            style={isActive ? 'box-shadow: inset 3px 0 0 #8b1a1a;' : ''}>
            <td class="py-2 text-center">
              {#if isActive}<Crown size={14} style="color:#c9a84c;" />{/if}
            </td>
            <td class="py-2">{pending ? '—' : c.turn_order}</td>
            <td>
              {#if campaign().isMaster && active && !pending}
                <button onclick={() => gotoTurn(i)} class="text-left hover:underline">{c.display_name}</button>
              {:else}
                {c.display_name}
              {/if}
              {#if pending}
                <span class="ml-2 text-[10px] uppercase tracking-wider" style="color:#8b1a1a;">
                  awaiting init
                </span>
              {/if}
            </td>
            <td class="text-center">{pending ? '—' : c.initiative}</td>
            <td class="text-center">
              {#if campaign().isMaster}
                <div class="inline-flex items-center gap-1">
                  <button type="button" onclick={() => applyDamage(c, -1)}
                    class="rounded px-1.5 py-0.5 text-xs" style="background:rgba(139,26,26,0.2);color:#c95a5a;">−</button>
                  <span class="tabular-nums">
                    {c.hp_current}<span class="text-neutral-500">/{c.hp_max}</span>
                    {#if (c.temp_hp as number) > 0}
                      <span class="ml-1 text-xs" style="color:#6fa39a;" title="Temp HP">+{c.temp_hp}</span>
                    {/if}
                  </span>
                  <button type="button" onclick={() => applyDamage(c, 1)}
                    class="rounded px-1.5 py-0.5 text-xs" style="background:rgba(111,163,154,0.2);color:#8aa86f;">+</button>
                  <input type="number" min="0" value={(c.temp_hp as number | undefined) ?? 0}
                    onchange={(e) => setTemp(c, +(e.currentTarget as HTMLInputElement).value)}
                    title="Temp HP"
                    class="w-12 rounded bg-neutral-800 border border-neutral-700 px-1 py-0.5 text-xs ml-1" />
                </div>
              {:else}
                <span>
                  {c.hp_current}/{c.hp_max}
                  {#if (c.temp_hp as number) > 0}
                    <span class="ml-1 text-xs" style="color:#6fa39a;">+{c.temp_hp}</span>
                  {/if}
                </span>
              {/if}
            </td>
            <td class="text-center">{c.ac}</td>
            <td class="text-right">
              {#if campaign().isMaster}
                <div class="inline-flex gap-1">
                  {#if active}
                    <button title="Jump to turn" onclick={() => gotoTurn(i)}
                      class="rounded p-1 hover:bg-neutral-800/40" style="color:#8b6914;">
                      <Play size={14} />
                    </button>
                  {/if}
                  <button title="Delete" class="text-red-400"
                    onclick={() => Encounters.combatants.delete(c.id as string).then(loadList)}>×</button>
                </div>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}

  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}
</section>
