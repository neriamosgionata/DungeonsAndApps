<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Factions } from '$lib/api/resources';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { Flag, Eye, EyeOff, Trash2, Pencil, X, Search, Shield, Swords, Handshake } from '@lucide/svelte';

  type Faction = {
    id: string;
    name: string;
    banner_color?: string | null;
    attitude?: string | null;
    description?: string | null;
    visibility: string;
  };

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let items = $state<Faction[]>([]);
  let error = $state('');
  let q = $state('');
  let attFilter = $state<string>('');

  // create form
  let newName = $state('');
  let newColor = $state('#8b6914');
  let newAttitude = $state('neutral');
  let newBody = $state('');
  let newVis = $state('master');

  // edit + reader
  let edit = $state<Faction | null>(null);
  let reading = $state<Faction | null>(null);

  const ATTITUDES = ['allied', 'friendly', 'neutral', 'hostile', 'enemy'];

  async function load() {
    try { items = (await Factions.list(cid)) as unknown as Faction[]; }
    catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      if ((ev.type as string).startsWith('faction_')) load();
    });
  });
  onDestroy(() => offWs?.());

  async function create(close: () => void) {
    try {
      await Factions.create(cid, {
        name: newName, banner_color: newColor, attitude: newAttitude,
        description: newBody || null, visibility: newVis,
      });
      newName = ''; newColor = '#8b6914'; newAttitude = 'neutral'; newBody = ''; newVis = 'master';
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function saveEdit() {
    if (!edit) return;
    try {
      await Factions.update(edit.id, {
        name: edit.name,
        banner_color: edit.banner_color ?? null,
        attitude: edit.attitude ?? null,
        description: edit.description ?? null,
        visibility: edit.visibility,
      });
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function remove(f: Faction) {
    if (!confirm($_('factions.delete_confirm').replace('{{name}}', f.name))) return;
    try { await Factions.delete(f.id); await load(); } catch (e) { error = (e as Error).message; }
  }
  async function cycleVis(f: Faction) {
    const next = f.visibility === 'master' ? 'players' : 'master';
    try { await Factions.update(f.id, { visibility: next }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  function sigil(name: string): string {
    return name.trim().split(/\s+/).map((w) => w[0] ?? '').join('').slice(0, 2).toUpperCase();
  }
  function snippet(body: string | null | undefined, n = 160): string {
    if (!body) return '';
    const cleaned = body.replace(/^\s*#{1,2}\s*.+\n?/g, '').trim();
    if (cleaned.length <= n) return cleaned;
    return cleaned.slice(0, n).trim() + '…';
  }

  // contrast color for text over banner
  function textOn(hex: string): string {
    const h = (hex ?? '#8b6914').replace('#', '');
    const n = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
    const r = parseInt(n.slice(0, 2), 16);
    const g = parseInt(n.slice(2, 4), 16);
    const b = parseInt(n.slice(4, 6), 16);
    // relative luminance
    const l = (0.299 * r + 0.587 * g + 0.114 * b) / 255;
    return l > 0.55 ? '#1a0f08' : '#f4e4c1';
  }
  function attitudeTone(a?: string | null): { bg: string; fg: string; border: string; icon: typeof Shield } {
    const k = (a ?? '').toLowerCase();
    if (k === 'allied' || k === 'friendly')
      return { bg: 'rgba(79,109,54,0.25)', fg: '#4f6d36', border: '#6b8a4f', icon: Handshake };
    if (k === 'hostile' || k === 'enemy')
      return { bg: 'rgba(139,26,26,0.22)', fg: '#8b1a1a', border: '#8b1a1a', icon: Swords };
    return { bg: 'rgba(139,105,20,0.18)', fg: '#6d510f', border: 'rgba(139,105,20,0.5)', icon: Shield };
  }

  // search + filter
  const visible = $derived.by(() => {
    const needle = q.trim().toLowerCase();
    return items.filter((f) => {
      if (attFilter && (f.attitude ?? '').toLowerCase() !== attFilter) return false;
      if (!needle) return true;
      return (
        f.name.toLowerCase().includes(needle) ||
        (f.attitude ?? '').toLowerCase().includes(needle) ||
        (f.description ?? '').toLowerCase().includes(needle)
      );
    }).sort((a, b) => a.name.localeCompare(b.name));
  });
</script>

<section class="hall">
  <!-- header -->
  <header class="hall-head">
    <div class="hdr-icon"><Flag size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('factions.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('factions.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('factions.new')}`} title={$_('factions.new_title')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2">
              <input required placeholder={$_('factions.name_ph')} bind:value={newName}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <label class="flex items-center gap-2">
                <span class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">{$_('factions.color')}</span>
                <input type="color" bind:value={newColor}
                  class="h-9 w-14 rounded border border-neutral-700 bg-neutral-900" />
                <span class="text-xs tabular-nums" style="color:#8b6355;">{newColor}</span>
              </label>
              <label class="flex items-center gap-2">
                <span class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">{$_('factions.attitude')}</span>
                <select bind:value={newAttitude}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 flex-1">
                  {#each ATTITUDES as a (a)}<option value={a}>{$_(`factions.att_${a}`)}</option>{/each}
                </select>
              </label>
              <textarea rows="6" placeholder={$_('factions.body_ph')}
                bind:value={newBody}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
              <label class="flex items-center gap-2">
                <span class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">{$_('visibility.label')}</span>
                <select bind:value={newVis}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 flex-1">
                  <option value="master">{$_('visibility.master')}</option>
                  <option value="players">{$_('visibility.players')}</option>
                </select>
              </label>
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('factions.raise')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
  </header>

  <div class="rule"></div>

  <div class="toolbar">
    <label class="search">
      <Search size={14} style="color:#8b6355;" />
      <input placeholder={$_('factions.search_ph')} bind:value={q} />
    </label>
    <select bind:value={attFilter} class="att-filter">
      <option value="">{$_('factions.all_attitudes').replace('{{n}}', String(items.length))}</option>
      {#each ATTITUDES as a (a)}<option value={a}>{$_(`factions.att_${a}`)}</option>{/each}
    </select>
  </div>

  {#if error}<p class="err">{error}</p>{/if}

  {#if items.length === 0}
    <p class="empty">{$_('factions.empty')}</p>
  {:else if visible.length === 0}
    <p class="empty">{$_('factions.empty_filtered')}</p>
  {:else}
    <div class="grid">
      {#each visible as f (f.id)}
        {@const tone = attitudeTone(f.attitude)}
        {@const color = f.banner_color ?? '#8b6914'}
        {@const txt = textOn(color)}
        <article class="banner">
          <button type="button" class="banner-open" onclick={() => reading = f}>
            <!-- coat of arms -->
            <div class="shield-wrap">
              <div class="shield" style={`background:linear-gradient(180deg, ${color}, color-mix(in srgb, ${color} 55%, #1a0f08));color:${txt};`}>
                <span class="shield-sigil">{sigil(f.name)}</span>
              </div>
              {#if f.attitude}
                {@const AttIcon = tone.icon}
                <span class="att-badge" style={`background:${tone.bg};color:${tone.fg};border-color:${tone.border};`}>
                  <AttIcon size={10} />
                  {$_(`factions.att_${f.attitude}`)}
                </span>
              {/if}
            </div>
            <!-- banner body -->
            <div class="banner-body">
              <div class="banner-topbar">
                <span class="color-swatch" style={`background:${color};`}></span>
                <span class="color-hex">{color}</span>
              </div>
              <h3 class="banner-name">{f.name}</h3>
              {#if f.description}
                <p class="banner-snippet">{snippet(f.description)}</p>
              {:else}
                <p class="banner-snippet italic" style="color:#8b6355;">{$_('factions.no_description')}</p>
              {/if}
            </div>
          </button>
          <footer class="banner-foot">
            <span class="vis" style={f.visibility === 'master'
              ? 'background:rgba(139,26,26,0.2);color:#8b1a1a;border-color:#8b1a1a;'
              : 'background:rgba(139,105,20,0.2);color:#6d510f;border-color:rgba(139,105,20,0.5);'}>
              {#if f.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
              {$_(`visibility.${f.visibility}`)}
            </span>
            {#if campaign().isMaster}
              <div class="actions">
                <button onclick={(e) => { e.stopPropagation(); cycleVis(f); }} title="Cycle visibility" class="icon-btn"><Eye size={13} /></button>
                <button onclick={(e) => { e.stopPropagation(); edit = { ...f }; }} title="Edit" class="icon-btn"><Pencil size={13} /></button>
                <button onclick={(e) => { e.stopPropagation(); remove(f); }} title="Delete" class="icon-btn danger"><Trash2 size={13} /></button>
              </div>
            {/if}
          </footer>
        </article>
      {/each}
    </div>
  {/if}
</section>

<!-- reader -->
{#if reading}
  {@const tone = attitudeTone(reading.attitude)}
  {@const color = reading.banner_color ?? '#8b6914'}
  {@const txt = textOn(color)}
  <div class="reader-backdrop" role="presentation"
    onclick={() => (reading = null)}
    onkeydown={(e) => e.key === 'Escape' && (reading = null)}>
    <div class="reader" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <button class="reader-close" aria-label="close" onclick={() => (reading = null)}>
        <X size={16} />
      </button>
      <div class="reader-banner" style={`background:linear-gradient(180deg, ${color}, color-mix(in srgb, ${color} 55%, #1a0f08));color:${txt};`}>
        <div class="reader-shield" style={`background:${color};color:${txt};`}>
          <span>{sigil(reading.name)}</span>
        </div>
        <div class="reader-title-wrap">
          <h3 class="reader-title" style={`color:${txt};`}>{reading.name}</h3>
          {#if reading.attitude}
            {@const AttIcon = tone.icon}
            <span class="reader-att" style={`background:${tone.bg};color:${tone.fg};border-color:${tone.border};`}>
              <AttIcon size={12} />
              {$_(`factions.att_${reading.attitude}`)}
            </span>
          {/if}
        </div>
      </div>
      <div class="reader-body-wrap">
        {#if reading.description}
          <div class="reader-body"><Paragraphs text={reading.description} /></div>
        {:else}
          <p class="italic" style="color:#8b6355;">{$_('factions.no_description')}</p>
        {/if}
      </div>
    </div>
  </div>
{/if}

<!-- edit -->
{#if edit}
  <div class="fixed inset-0 z-40 bg-black/70 flex items-center justify-center p-4"
    role="presentation"
    onclick={() => (edit = null)}
    onkeydown={(e) => e.key === 'Escape' && (edit = null)}>
    <div class="w-full max-w-2xl rounded-lg border p-5 max-h-[90vh] overflow-y-auto space-y-3"
      role="dialog" aria-modal="true" tabindex="-1"
      style="border-color:#8b6914; background:#241810;"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between">
        <h3 class="font-display text-lg" style="color:#f4e4c1 !important;">{$_('factions.edit_title')}</h3>
        <button onclick={() => (edit = null)} aria-label="close" style="color:#c9a84c;"><X size={16} /></button>
      </div>
      <input required placeholder={$_('common.name')} bind:value={edit.name}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      <label class="flex items-center gap-2">
        <span class="text-[10px] uppercase tracking-widest font-display" style="color:#c9a84c;">{$_('factions.color')}</span>
        <input type="color" value={edit.banner_color ?? '#8b6914'}
          onchange={(e) => edit && (edit.banner_color = (e.currentTarget as HTMLInputElement).value)}
          class="h-9 w-14 rounded border border-neutral-700 bg-neutral-900" />
        <span class="text-xs tabular-nums" style="color:#8b6355;">{edit.banner_color}</span>
      </label>
      <label class="flex items-center gap-2">
        <span class="text-[10px] uppercase tracking-widest font-display" style="color:#c9a84c;">{$_('factions.attitude')}</span>
        <select bind:value={edit.attitude}
          class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 flex-1">
          {#each ATTITUDES as a (a)}<option value={a}>{$_(`factions.att_${a}`)}</option>{/each}
        </select>
      </label>
      <textarea rows="12" placeholder={$_('common.description')}
        bind:value={edit.description}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      <select bind:value={edit.visibility} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
        <option value="master">{$_('visibility.master')}</option><option value="players">{$_('visibility.players')}</option>
      </select>
      <div class="flex justify-end gap-2">
        <button onclick={() => (edit = null)}
          class="rounded-md px-4 py-2 text-sm"
          style="background:#3a2313;color:#f4e4c1;border:1px solid #6d510f;">{$_('common.cancel')}</button>
        <button onclick={saveEdit} class="rounded-md bg-violet-600 px-6 py-2 text-sm text-white">{$_('common.save')}</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .hall { max-width: 72rem; margin: 0 auto; padding: 1rem 1.25rem; }

  .hall-head { display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 1rem; }
  .hdr-icon, .hdr-right { display: flex; justify-content: center; }
  .hdr-center { text-align: center; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.6rem, 3vw, 2.4rem);
    font-weight: 900; letter-spacing: 0.08em;
    color: #2c1810 !important; line-height: 1;
  }
  .hdr-sub {
    margin-top: 0.25rem;
    font-family: 'Crimson Text', serif; font-style: italic;
    font-size: 0.85rem; color: #6d510f;
  }
  .fleuron { color: #8b6914; margin: 0 0.4rem; font-style: normal; }

  .rule {
    height: 3px; margin: 0.85rem 0 1rem;
    background: linear-gradient(90deg, transparent, #8b6914 10%, #c9a84c 50%, #8b6914 90%, transparent);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦"; position: absolute; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    color: #6d510f; background: #f4e4c1;
    padding: 0 0.5rem; font-size: 0.9rem;
  }

  .toolbar { display: grid; grid-template-columns: 1fr 14rem; gap: 0.75rem; margin-bottom: 1.25rem; }
  @media (max-width: 640px) { .toolbar { grid-template-columns: 1fr; } }
  .search {
    display: flex; align-items: center; gap: 0.5rem;
    border: 1.5px solid rgba(139,105,20,0.5);
    border-radius: 0.3rem; padding: 0 0.65rem;
    background: #f4e4c1;
  }
  .search input {
    flex: 1; background: transparent !important; border: 0 !important;
    padding: 0.4rem 0.25rem !important;
    font-family: 'Crimson Text', serif; color: #2c1810 !important;
    outline: none; box-shadow: none !important;
  }
  .att-filter {
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    border-radius: 0.3rem !important;
    background: #f4e4c1 !important;
    color: #2c1810 !important;
    font-family: 'Cinzel', serif;
    padding: 0.4rem 0.65rem !important;
  }

  .err { color: #c95a5a; font-size: 0.85rem; margin: 0.5rem 0; }
  .empty { text-align: center; padding: 3rem; font-style: italic; color: #8b6355; }

  .grid {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
  }

  .banner {
    display: flex; flex-direction: column;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 4px 12px rgba(0,0,0,0.4);
    overflow: hidden;
    transition: transform 0.1s, box-shadow 0.1s;
  }
  .banner:hover { transform: translateY(-2px); box-shadow: inset 0 1px 0 rgba(255,248,220,0.65), 0 10px 22px rgba(0,0,0,0.55); }
  .banner-open { display: flex; flex-direction: column; background: transparent; cursor: pointer; text-align: left; color: inherit; width: 100%; padding: 0; }

  .shield-wrap {
    position: relative;
    display: grid; place-items: center;
    padding: 1rem 0.85rem 0.5rem;
  }
  .shield {
    width: 4.5rem; height: 5rem;
    display: grid; place-items: center;
    border: 2px solid #4e3909;
    border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%;
    /* shield shape: pointed chevron at bottom via clip-path */
    clip-path: polygon(0 0, 100% 0, 100% 55%, 50% 100%, 0 55%);
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.25), 0 4px 8px rgba(0,0,0,0.5);
    border-radius: 0;
    font-family: 'Cinzel', serif;
  }
  .shield-sigil {
    font-weight: 900;
    font-size: 1.6rem;
    letter-spacing: 0.04em;
    text-shadow: 0 1px 0 rgba(0,0,0,0.35);
  }
  .att-badge {
    position: absolute;
    right: 0.6rem; top: 0.6rem;
    display: inline-flex; align-items: center; gap: 0.25rem;
    padding: 0.1rem 0.45rem;
    font-size: 0.6rem; letter-spacing: 0.12em; text-transform: uppercase;
    border: 1px solid;
    border-radius: 9999px;
    font-family: 'Cinzel', serif;
    font-weight: 700;
  }

  .banner-body { padding: 0.5rem 1rem 0.6rem; }
  .banner-topbar {
    display: flex; align-items: center; gap: 0.4rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.65rem;
    color: #8b6914;
  }
  .color-swatch {
    display: inline-block;
    width: 0.85rem; height: 0.85rem;
    border-radius: 0.15rem;
    border: 1px solid rgba(139,105,20,0.55);
  }
  .color-hex { font-family: 'Special Elite', monospace; font-size: 0.7rem; letter-spacing: 0.05em; }
  .banner-name {
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1.2rem;
    line-height: 1.2;
    color: #2c1810 !important;
    margin-top: 0.25rem;
    letter-spacing: 0.02em;
  }
  .banner-snippet {
    font-family: 'Crimson Text', serif;
    font-size: 0.9rem;
    line-height: 1.4;
    color: #3a2313;
    margin-top: 0.35rem;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    overflow: hidden;
  }

  .banner-foot {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.45rem 1rem 0.55rem;
    border-top: 1px dashed rgba(139,105,20,0.35);
  }
  .vis {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.6rem; letter-spacing: 0.12em; text-transform: uppercase;
    padding: 0.1rem 0.5rem; border-radius: 0.25rem;
    border: 1px solid;
    font-family: 'Cinzel', serif; font-weight: 700;
  }
  .actions { display: inline-flex; gap: 0.15rem; }
  .icon-btn {
    padding: 0.3rem; border-radius: 0.3rem;
    color: #6d510f; background: transparent;
  }
  .icon-btn:hover { background: rgba(139,105,20,0.15); color: #2c1810; }
  .icon-btn.danger { color: #8b1a1a; }
  .icon-btn.danger:hover { background: rgba(139,26,26,0.1); }

  /* reader modal */
  .reader-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.75);
    display: grid; place-items: center;
    z-index: 60; padding: 1rem;
  }
  .reader {
    position: relative;
    width: min(52rem, 100%);
    max-height: 90vh;
    overflow: hidden;
    display: flex; flex-direction: column;
    border: 2px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1;
    box-shadow: 0 18px 40px rgba(0,0,0,0.65);
  }
  .reader-close {
    position: absolute; top: 0.6rem; right: 0.6rem;
    width: 2rem; height: 2rem;
    display: grid; place-items: center;
    border-radius: 9999px;
    background: #3a2313;
    color: #c9a84c;
    border: 1px solid #4e3909;
    z-index: 5;
  }
  .reader-close:hover { background: #4e3909; color: #f7e2a5; }

  .reader-banner {
    display: flex; align-items: center; gap: 1rem;
    padding: 1.25rem 1.75rem;
    border-bottom: 2px solid #4e3909;
    box-shadow: inset 0 -3px 6px rgba(0,0,0,0.35);
  }
  .reader-shield {
    width: 4.5rem; height: 5rem;
    display: grid; place-items: center;
    clip-path: polygon(0 0, 100% 0, 100% 55%, 50% 100%, 0 55%);
    font-family: 'Cinzel', serif;
    font-size: 1.6rem; font-weight: 900;
    text-shadow: 0 1px 0 rgba(0,0,0,0.4);
    border: 2px solid #1a0f08;
    flex: none;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.25);
  }
  .reader-title-wrap { display: flex; flex-direction: column; gap: 0.35rem; min-width: 0; }
  .reader-title {
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: clamp(1.4rem, 2.6vw, 2rem);
    line-height: 1.1;
    letter-spacing: 0.02em;
    margin: 0;
  }
  .reader-att {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.15rem 0.55rem;
    font-size: 0.7rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    border: 1px solid;
    border-radius: 9999px;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    align-self: flex-start;
  }

  .reader-body-wrap {
    padding: 1.75rem 2rem 2.25rem;
    overflow-y: auto;
    background:
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
  }
  .reader-body {
    font-family: 'Crimson Text', serif;
    font-size: 1.05rem; line-height: 1.6;
    color: #2c1810;
  }
</style>
