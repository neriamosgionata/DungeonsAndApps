<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Spells } from '$lib/api/resources';
  import type { Spell } from '$lib/types';
  import { SPELLCASTER_CLASSES } from '$lib/dnd/classes';

  let q = $state('');
  let level = $state<number | ''>('');
  let clsFilter = $state('');
  let items = $state<Spell[]>([]);
  let selected = $state<Spell | null>(null);
  let loading = $state(false);
  let error = $state('');

  let reqId = 0;
  async function load() {
    const myId = ++reqId;
    loading = true;
    try {
      const results = await Spells.list({
        q: q.trim() || undefined,
        level: level === '' ? undefined : Number(level),
        class: clsFilter || undefined,
      });
      // drop stale response
      if (myId !== reqId) return;
      items = results;
    } catch (e) {
      if (myId === reqId) error = (e as Error).message;
    } finally {
      if (myId === reqId) loading = false;
    }
  }

  // debounce 250ms on q; instant on selects
  let debounceTimer: ReturnType<typeof setTimeout> | undefined;
  function onQueryInput() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(load, 250);
  }

  $effect(() => { void level; void clsFilter; load(); });

  onMount(load);
  
  onDestroy(() => {
    clearTimeout(debounceTimer);
  });
</script>

<section class="mx-auto max-w-6xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold">{$_('spells.title')}</h2>

  <div class="mt-4 flex flex-wrap gap-2">
    <div class="relative flex-1 min-w-48">
      <input placeholder={$_('spells.search_ph')} bind:value={q} oninput={onQueryInput}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 pr-8" />
      {#if q}
        <button type="button" onclick={() => { q = ''; load(); }}
          class="absolute inset-y-0 right-2 text-neutral-500 hover:text-neutral-200">×</button>
      {/if}
    </div>
    <select bind:value={level} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
      <option value="">{$_('spells.any_level')}</option>
      {#each [0,1,2,3,4,5,6,7,8,9] as l (l)}
        <option value={l}>{l === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${l}`}</option>
      {/each}
    </select>
    <select bind:value={clsFilter} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
      <option value="">{$_('spells.any_class')}</option>
      {#each SPELLCASTER_CLASSES as c (c)}
        <option value={c}>{c}</option>
      {/each}
    </select>
  </div>

  <div class="mt-3 flex items-center gap-2 text-xs text-neutral-400">
    {#if loading}<span>{$_('spells.loading')}</span>
    {:else}<span>{items.length} {$_('spells.results')}</span>
    {/if}
    {#if error}<span class="text-red-400">· {error}</span>{/if}
  </div>

  <div class="mt-4 grid gap-4 md:grid-cols-[1fr_1.5fr]">
    <div class="spell-list">
      <ul class="max-h-[70vh] overflow-y-auto py-1">
        {#each items as s (s.slug)}
          <li>
            <button onclick={() => selected = s}
              class="spell-row {selected?.slug === s.slug ? 'active' : ''}">
              <span class="lv">{s.level === 0 ? 'C' : s.level}</span>
              <span class="name">
                {s.name}
                {#if s.source}<span class="src">{s.source}</span>{/if}
              </span>
              <span class="school">{s.school}</span>
            </button>
          </li>
        {/each}
        {#if !loading && items.length === 0}
          <li class="italic px-3 py-2" style="color:#8b6355;">{$_('spells.none')}</li>
        {/if}
      </ul>
    </div>
    <div class="rounded-lg border border-neutral-800 bg-neutral-900 p-5 min-h-80">
      {#if selected}
        <h3 class="text-2xl font-bold text-violet-300">{selected.name}</h3>
        <p class="text-sm text-neutral-400">
          {selected.level === 0 ? $_('spells.cantrip') : `${$_('spells.level')} ${selected.level}`} · {selected.school}
          {selected.ritual ? ' · ritual' : ''}{selected.concentration ? ' · concentration' : ''}
        </p>
        <p class="mt-3 text-sm text-neutral-400">{$_('spells.classes')}: {selected.classes.join(', ')}</p>
        {#if selected.source}
          <p class="mt-1 text-sm text-neutral-400"><b>Source:</b> {selected.source}</p>
        {/if}
        <div class="mt-3 grid grid-cols-2 gap-x-4 gap-y-1 text-xs text-neutral-300">
          {#if selected.casting_time}<div><b>Cast:</b> {selected.casting_time}</div>{/if}
          {#if selected.range_text}<div><b>Range:</b> {selected.range_text}</div>{/if}
          {#if selected.components}<div class="col-span-2"><b>Components:</b> {selected.components}</div>{/if}
          {#if selected.duration}<div><b>Duration:</b> {selected.duration}</div>{/if}
        </div>
        <p class="mt-4 whitespace-pre-wrap text-neutral-200">{selected.description}</p>
        {#if selected.higher_levels}
          <p class="mt-4 text-sm text-neutral-300"><b>{$_('spells.higher')}:</b> {selected.higher_levels}</p>
        {/if}
      {:else}
        <p class="text-neutral-500">{$_('spells.select')}</p>
      {/if}
    </div>
  </div>
</section>

<style>
  .spell-list {
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 6px rgba(0,0,0,0.4);
    color: #2c1810;
  }
  .spell-row {
    width: 100%;
    display: grid;
    grid-template-columns: 1.75rem 1fr auto;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.4rem 0.75rem;
    text-align: left;
    border-bottom: 1px dashed rgba(139,105,20,0.25);
  }
  .spell-row:last-child { border-bottom: 0; }
  .spell-row .lv {
    display: inline-grid; place-items: center;
    width: 1.5rem; height: 1.5rem;
    border-radius: 9999px;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 55%, #6d510f 100%);
    border: 1px solid #4e3909;
    color: #1a0f08;
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 0.7rem;
  }
  .spell-row .name { font-weight: 600; color: #2c1810; }
  .spell-row .src {
    font-size: 0.65rem;
    font-weight: 500;
    margin-left: 0.5rem;
    padding: 0.05rem 0.35rem;
    border-radius: 0.2rem;
    background: rgba(139, 105, 20, 0.18);
    color: #6d510f;
    border: 1px solid rgba(139, 105, 20, 0.4);
    vertical-align: middle;
  }
  .spell-row .school { font-size: 0.7rem; font-style: italic; color: #8b6914; }
  .spell-row:hover { background: rgba(201,168,76,0.15); }
  .spell-row.active { background: rgba(201,168,76,0.3); box-shadow: inset 3px 0 0 #8b1a1a; }
</style>
