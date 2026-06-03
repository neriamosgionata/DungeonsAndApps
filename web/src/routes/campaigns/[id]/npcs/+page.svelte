<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { NPCs, Factions } from '$lib/api/resources';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import NpcStatBlock from '$lib/components/NpcStatBlock.svelte';
  import type { NpcStats } from '$lib/components/NpcStatBlock.svelte';
  import { Eye, EyeOff, Trash2, Search, X, Pencil, Users as UsersIcon, Handshake, Swords, Shield } from '@lucide/svelte';

  type Npc = {
    id: string;
    name: string;
    role?: string | null;
    faction_id?: string | null;
    description?: string | null;
    image_key?: string | null;
    visibility: string;
    stats?: NpcStats | Record<string, unknown>;
  };
  type Faction = { id: string; name: string; banner_color?: string | null };

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let npcs = $state<Npc[]>([]);
  let factions = $state<Faction[]>([]);
  let error = $state('');
  let loading = $state(true);
  let q = $state('');
  let filter = $state<string>(''); // '' = all | faction_id | 'none'

  // create form
  let form = $state<Record<string, unknown>>({
    visibility: 'master',
    stats: { attitude: 'neutrale', status: 'vivo', abilities: { str:10, dex:10, con:10, int:10, wis:10, cha:10 } },
  });

  async function load() {
    try {
      [npcs, factions] = await Promise.all([
        NPCs.list(cid) as Promise<Npc[]>,
        Factions.list(cid) as Promise<Faction[]>,
      ]);
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t.startsWith('npc_') || t.startsWith('faction_')) load();
    });
  });
  onDestroy(() => offWs?.());

  const factionsById = $derived(Object.fromEntries(factions.map((f) => [f.id, f])));

  const PAGE_SIZE = 20;
  let pageIdx = $state(0);
  // reset to first page whenever filters change
  $effect(() => { void q; void filter; pageIdx = 0; });

  // group & filter
  const visible = $derived.by(() => {
    const needle = q.trim().toLowerCase();
    return npcs.filter((n) => {
      if (filter) {
        if (filter === 'none' && n.faction_id) return false;
        if (filter !== 'none' && n.faction_id !== filter) return false;
      }
      if (!needle) return true;
      return (
        n.name.toLowerCase().includes(needle) ||
        (n.role ?? '').toLowerCase().includes(needle) ||
        (n.description ?? '').toLowerCase().includes(needle)
      );
    }).sort((a, b) => a.name.localeCompare(b.name));
  });

  const paginated = $derived(!q.trim() && !filter);
  const pageCount = $derived(Math.max(1, Math.ceil(visible.length / PAGE_SIZE)));
  const pageItems = $derived(paginated ? visible.slice(pageIdx * PAGE_SIZE, (pageIdx + 1) * PAGE_SIZE) : visible);

  const groups = $derived.by<Array<{ key: string; name: string; items: Npc[] }>>(() => {
    const buckets: Record<string, Npc[]> = {};
    for (const n of pageItems) {
      const k = n.faction_id ?? '__none__';
      (buckets[k] ??= []).push(n);
    }
    const rows: Array<{ key: string; name: string; items: Npc[] }> = [];
    for (const f of factions) {
      if (buckets[f.id]?.length) rows.push({ key: f.id, name: f.name, items: buckets[f.id] });
    }
    if (buckets.__none__?.length) rows.push({ key: '__none__', name: $_('npcs.no_faction_label'), items: buckets.__none__ });
    for (const r of rows) r.items.sort((a, b) => a.name.localeCompare(b.name));
    return rows;
  });

  function stat(n: Npc, key: 'attitude' | 'status'): string | undefined {
    const s = n.stats as Record<string, unknown> | undefined;
    const v = s?.[key];
    return typeof v === 'string' ? v : undefined;
  }
  function npcStats(n: Npc): NpcStats | undefined {
    return n.stats as NpcStats | undefined;
  }

  const ATTITUDES = ['alleato', 'neutrale', 'nemico'];
  const STATUSES  = ['vivo', 'ferito', 'morto'];

  function attitudeColor(a?: string): string {
    switch (a) {
      case 'alleato': return 'bg-emerald-700/40 text-emerald-100';
      case 'nemico':  return 'bg-red-700/50 text-red-100';
      default:        return 'bg-neutral-700/40 text-neutral-100';
    }
  }
  function statusColor(s?: string): string {
    switch (s) {
      case 'morto':  return 'bg-neutral-800/80 text-neutral-300';
      case 'ferito': return 'bg-amber-700/50 text-amber-100';
      default:       return 'bg-emerald-800/50 text-emerald-100';
    }
  }

  async function create(close: () => void) {
    try {
      await NPCs.create(cid, form);
      form = { visibility: 'master', stats: { attitude: 'neutrale', status: 'vivo', abilities: { str:10, dex:10, con:10, int:10, wis:10, cha:10 } } };
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function toggleVis(n: Npc) {
    const next = n.visibility === 'players' ? 'master' : 'players';
    await NPCs.update(n.id, { visibility: next });
    await load();
  }

  async function remove(id: string) {
    if (!confirm($_('npcs.delete_confirm'))) return;
    await NPCs.delete(id);
    await load();
  }

  function sigil(name: string): string {
    return name.trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 2).toUpperCase();
  }

  let detail = $state<Npc | null>(null);
  let edit = $state<Npc | null>(null);

  async function saveEdit() {
    if (!edit) return;
    try {
      await NPCs.update(edit.id, {
        name: edit.name,
        role: edit.role ?? null,
        faction_id: edit.faction_id ?? null,
        description: edit.description ?? null,
        image_key: edit.image_key ?? null,
        visibility: edit.visibility,
        stats: edit.stats ?? {},
      });
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }
</script>

<section class="roster">
  <header class="roster-head">
    <div class="hdr-icon"><UsersIcon size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('npcs.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('npcs.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
    {#if campaign().isMaster}
      <CollapsibleAdd label={`+ ${$_('npcs.new')}`} title={$_('npcs.new')} alignEnd={true}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2 sm:grid-cols-2">
            <div class="sm:col-span-2">
              <ImageUpload
                value={(form.image_key as string | null) ?? null}
                kind="npc" label={$_('npcs.portrait')}
                onchange={(url) => form.image_key = url} />
            </div>
            <input required placeholder={$_('npcs.name_ph')} bind:value={form.name}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <input placeholder={$_('npcs.role_ph')} bind:value={form.role}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <select bind:value={form.faction_id}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value={null}>{$_('npcs.no_faction')}</option>
              {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
            </select>
            <select
              value={(form.stats as { attitude?: string })?.attitude ?? 'neutrale'}
              onchange={(e) => form.stats = { ...(form.stats as object), attitude: (e.currentTarget as HTMLSelectElement).value }}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              {#each ATTITUDES as a (a)}<option value={a}>{$_(`npcs.att_${a}`)}</option>{/each}
            </select>
            <select
              value={(form.stats as { status?: string })?.status ?? 'vivo'}
              onchange={(e) => form.stats = { ...(form.stats as object), status: (e.currentTarget as HTMLSelectElement).value }}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              {#each STATUSES as s (s)}<option value={s}>{$_(`npcs.status_${s}`)}</option>{/each}
            </select>
            <select bind:value={form.visibility}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value="master">{$_('visibility.master')}</option>
              <option value="players">{$_('visibility.players')}</option>
            </select>
            <textarea placeholder={$_('lore.body_ph')}
              bind:value={form.description} rows="4"
              class="sm:col-span-2 rounded bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
            <div class="sm:col-span-2 flex justify-end">
              <button class="rounded bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
    </div>
  </header>

  <div class="rule"></div>

  <!-- search + filter -->
  <div class="toolbar">
    <label class="search">
      <Search size={14} style="color:#8b6355;" />
      <input placeholder={$_('npcs.search_ph')} bind:value={q} />
    </label>
    <select bind:value={filter} class="fac-filter">
      <option value="">{$_('npcs.all').replace('{{n}}', String(npcs.length))}</option>
      {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
      <option value="none">{$_('npcs.no_faction_label')}</option>
    </select>
  </div>

  {#if error}<p class="err">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  <p class="count">
    {$_('npcs.count').replace('{{n}}', String(visible.length))}
    {#if paginated && pageCount > 1}
      · {$_('npcs.page_of').replace('{{page}}', String(pageIdx + 1)).replace('{{total}}', String(pageCount))}
    {/if}
  </p>

  {#each groups as g (g.key)}
    <div class="faction-group">
      <h3 class="faction-name">
        <span class="flourish">§</span> {g.name}
        <span class="fac-count">({g.items.length})</span>
      </h3>
      <div class="grid">
        {#each g.items as n (n.id)}
          {@const att = stat(n, 'attitude')}
          {@const st = stat(n, 'status')}
          {@const isDead = st === 'morto' || st === 'dead'}
          <article class="npc-card {isDead ? 'dead' : ''}">
            <div class="npc-click" role="button" tabindex="0"
              onclick={() => detail = n}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (detail = n)}>
              <div class="portrait-wrap">
                <div class="portrait">
                  {#if n.image_key}
                    <img src={n.image_key} alt="" />
                  {:else}
                    <span class="sigil">{sigil(n.name)}</span>
                  {/if}
                </div>
                {#if att}
                  {@const Att = att === 'alleato' || att === 'allied' || att === 'friendly' ? Handshake
                    : att === 'nemico' || att === 'hostile' || att === 'enemy' ? Swords : Shield}
                  <span class="att-pip" style={att === 'alleato' || att === 'allied' || att === 'friendly'
                    ? 'background:rgba(79,109,54,0.35);color:#8aa86f;border-color:#6b8a4f;'
                    : att === 'nemico' || att === 'hostile' || att === 'enemy'
                      ? 'background:rgba(139,26,26,0.35);color:#f4b0b0;border-color:#8b1a1a;'
                      : 'background:rgba(139,105,20,0.25);color:#c9a84c;border-color:#8b6914;'}>
                    <Att size={11} />
                  </span>
                {/if}
              </div>

              <div class="npc-body">
                <div class="npc-name">{n.name}</div>
                {#if n.role}<div class="npc-role">{n.role}</div>{/if}

                <div class="tags">
                  {#if st}<span class="tag {statusColor(st)}">{$_(`npcs.status_${st}`)}</span>{/if}
                  {#if n.faction_id && factionsById[n.faction_id]}
                    <span class="tag faction-tag">{factionsById[n.faction_id].name}</span>
                  {/if}
                  {#if npcStats(n)?.ac}<span class="tag bg-amber-900/40 text-amber-100">AC {npcStats(n)!.ac}</span>{/if}
                  {#if npcStats(n)?.hp?.max}<span class="tag bg-red-900/40 text-red-100">HP {npcStats(n)!.hp!.max}</span>{/if}
                  {#if npcStats(n)?.cr}<span class="tag bg-neutral-800 text-neutral-300">CR {npcStats(n)!.cr}</span>{/if}
                </div>

                {#if n.description}
                  <p class="lede">{n.description.replace(/^\s*#{1,2}\s*.+\n?/g, '').trim()}</p>
                {/if}
              </div>
            </div>

            <footer class="npc-foot">
              <span class="vis" style={n.visibility === 'master'
                ? 'background:rgba(139,26,26,0.2);color:#8b1a1a;border-color:#8b1a1a;'
                : 'background:rgba(139,105,20,0.2);color:#6d510f;border-color:rgba(139,105,20,0.5);'}>
                {#if n.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
                {$_(`visibility.${n.visibility}`)}
              </span>
              {#if campaign().isMaster}
                <div class="actions">
                  <button onclick={(e) => { e.stopPropagation(); toggleVis(n); }} title="Toggle visibility" class="icon-btn"><Eye size={13} /></button>
                  <button onclick={(e) => { e.stopPropagation(); edit = { ...n, stats: { ...(n.stats as object ?? {}) } }; }} title="Edit" class="icon-btn"><Pencil size={13} /></button>
                  <button onclick={(e) => { e.stopPropagation(); remove(n.id); }} title="Delete" class="icon-btn danger"><Trash2 size={13} /></button>
                </div>
              {/if}
            </footer>
          </article>
        {/each}
      </div>
    </div>
  {/each}

  {#if visible.length === 0}
    <p class="empty">{$_('npcs.empty')}</p>
  {/if}

  {#if paginated && pageCount > 1}
    <nav class="pager" aria-label="Pagination">
      <button type="button" disabled={pageIdx === 0}
        onclick={() => pageIdx = Math.max(0, pageIdx - 1)}
        class="pg-btn">‹ prev</button>
      {#each Array(pageCount) as _, i (i)}
        <button type="button"
          onclick={() => pageIdx = i}
          class="pg-btn {pageIdx === i ? 'active' : ''}">{i + 1}</button>
      {/each}
      <button type="button" disabled={pageIdx >= pageCount - 1}
        onclick={() => pageIdx = Math.min(pageCount - 1, pageIdx + 1)}
        class="pg-btn">next ›</button>
    </nav>
  {/if}
</section>

{#if edit}
  <div class="npc-modal-backdrop" role="presentation"
    onclick={() => (edit = null)}
    onkeydown={(e) => e.key === 'Escape' && (edit = null)}>
    <div class="npc-modal" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}>
      <button class="npc-modal-close" aria-label="close" onclick={() => (edit = null)}>
        <X size={16} />
      </button>
      <h3 class="font-display font-bold text-xl" style="color:#2c1810;">{$_('npcs.edit_title')}</h3>
      <div class="mt-4 grid gap-2 sm:grid-cols-2">
        <div class="sm:col-span-2">
          <ImageUpload
            value={(edit.image_key as string | null) ?? null}
            kind="npc" label={$_('npcs.portrait')}
            onchange={(url) => edit && (edit.image_key = url)} />
        </div>
        <input required placeholder={$_('npcs.name_ph')} bind:value={edit.name}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input placeholder={$_('npcs.role_ph')} bind:value={edit.role}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <select bind:value={edit.faction_id}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value={null}>{$_('npcs.no_faction')}</option>
          {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
        </select>
        <select
          value={(edit.stats as { attitude?: string })?.attitude ?? 'neutrale'}
          onchange={(e) => edit && (edit.stats = { ...(edit.stats as object ?? {}), attitude: (e.currentTarget as HTMLSelectElement).value })}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          {#each ATTITUDES as a (a)}<option value={a}>{$_(`npcs.att_${a}`)}</option>{/each}
        </select>
        <select
          value={(edit.stats as { status?: string })?.status ?? 'vivo'}
          onchange={(e) => edit && (edit.stats = { ...(edit.stats as object ?? {}), status: (e.currentTarget as HTMLSelectElement).value })}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          {#each STATUSES as s (s)}<option value={s}>{$_(`npcs.status_${s}`)}</option>{/each}
        </select>
        <select bind:value={edit.visibility}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value="master">{$_('visibility.master')}</option>
          <option value="players">{$_('visibility.players')}</option>
        </select>
        <textarea placeholder={$_('lore.body_ph')}
          bind:value={edit.description} rows="4"
          class="sm:col-span-2 rounded bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>

        <div class="sm:col-span-2">
          <div class="text-[11px] uppercase tracking-widest text-neutral-500 mb-1">{$_('npcs.stat_block')}</div>
          {#if edit.stats}
            <NpcStatBlock bind:stats={edit.stats as Record<string, unknown>} edit={true} />
          {/if}
        </div>
      </div>
      <footer class="mt-4 flex justify-end gap-2">
        <button onclick={() => (edit = null)}
          class="rounded-md border border-neutral-700 px-4 py-2 text-sm">{$_('common.cancel')}</button>
        <button onclick={saveEdit} class="rounded-md bg-violet-600 px-6 py-2 text-sm text-white">{$_('common.save')}</button>
      </footer>
    </div>
  </div>
{/if}

{#if detail}
  <div class="npc-modal-backdrop" role="presentation"
    onclick={() => (detail = null)}
    onkeydown={(e) => e.key === 'Escape' && (detail = null)}>
    <div class="npc-modal" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}>
      <button class="npc-modal-close" aria-label="close" onclick={() => (detail = null)}>
        <X size={16} />
      </button>
      <header class="npc-modal-head">
        <div class="npc-avatar npc-avatar-lg">
          {#if detail.image_key}
            <img src={detail.image_key} alt="" />
          {:else}
            <span>{sigil(detail.name)}</span>
          {/if}
        </div>
        <div class="min-w-0">
          <h3 class="font-display font-bold text-2xl leading-tight" style="color:#2c1810;">{detail.name}</h3>
          {#if detail.role}<div class="italic" style="color:#5c3d2e;">{detail.role}</div>{/if}
          <div class="mt-2 flex flex-wrap gap-1 text-[10px] font-display tracking-widest uppercase">
            {#if stat(detail, 'attitude')}
              <span class="tag {attitudeColor(stat(detail, 'attitude'))}">{$_(`npcs.att_${stat(detail, 'attitude')}`)}</span>
            {/if}
            {#if stat(detail, 'status')}
              <span class="tag {statusColor(stat(detail, 'status'))}">{$_(`npcs.status_${stat(detail, 'status')}`)}</span>
            {/if}
            {#if detail.faction_id && factionsById[detail.faction_id]}
              <span class="tag bg-neutral-700/40 text-neutral-100">{factionsById[detail.faction_id].name}</span>
            {/if}
            <span class="tag bg-neutral-700/40 text-neutral-100 inline-flex items-center gap-1">
              {#if detail.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
              {$_(`visibility.${detail.visibility}`)}
            </span>
          </div>
        </div>
      </header>

      <div class="mt-4 divider-cog"></div>

      {#if detail.description}
        <div class="mt-4 text-[15px]" style="color:#2c1810;">
          <Paragraphs text={detail.description} />
        </div>
      {:else}
        <p class="mt-4 italic" style="color:#8b6355;">{$_('npcs.no_description')}</p>
      {/if}

      <!-- Stat Block -->
      {#if detail.stats && (detail.stats as NpcStats).ac}
        <div class="mt-4 p-3 rounded" style="background:rgba(139,105,20,0.08); border:1px solid rgba(139,105,20,0.25);">
          <NpcStatBlock stats={detail.stats as Record<string, unknown>} />
        </div>
      {/if}

      {#if campaign().isMaster}
        <footer class="mt-6 flex items-center gap-3 justify-end pt-3" style="border-top:1px dashed rgba(139,105,20,0.35);">
          <button onclick={() => { toggleVis(detail!); }}
            class="inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm"
            style="background:rgba(43,29,16,0.15); color:#5c3d2e;">
            {#if detail.visibility === 'master'}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
            toggle
          </button>
          <button onclick={() => { if (detail) { edit = { ...detail, stats: { ...(detail.stats as object ?? {}) } }; detail = null; } }}
            class="inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm"
            style="background:#c9a84c; color:#1a0f08; border:1.5px solid #4e3909;">
            <Pencil size={14} /> edit
          </button>
          <button onclick={() => { if (detail) { remove(detail.id); detail = null; } }}
            class="inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm text-red-100"
            style="background:#8b1a1a;">
            <Trash2 size={14} /> delete
          </button>
        </footer>
      {/if}
    </div>
  </div>
{/if}

<style>
  .roster { max-width: 72rem; margin: 0 auto; padding: 1rem 1.25rem; }
  @media (max-width: 639px) { .roster { padding: 0.5rem 0.6rem; } }
  .roster-head { display: grid; grid-template-columns: auto 1fr auto; align-items: center; gap: 1rem; }
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

  .toolbar { display: grid; grid-template-columns: 1fr 14rem; gap: 0.75rem; margin-bottom: 0.75rem; }
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
  .fac-filter {
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    border-radius: 0.3rem !important;
    background: #f4e4c1 !important;
    color: #2c1810 !important;
    font-family: 'Cinzel', serif;
    padding: 0.4rem 0.65rem !important;
  }
  .err { color: #c95a5a; font-size: 0.85rem; margin: 0.5rem 0; }
  .count { margin: 0.5rem 0 0.75rem; font-size: 0.85rem; color: #6d510f; font-family: 'Cinzel', serif; }
  .empty { text-align: center; padding: 3rem; font-style: italic; color: #8b6355; }

  .faction-group { margin-top: 1.5rem; }
  .faction-name {
    font-family: 'IM Fell English SC', serif;
    color: #6d510f;
    font-size: 1.05rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    border-bottom: 1px dashed rgba(139,105,20,0.4);
    padding-bottom: 0.35rem;
    margin-bottom: 0.85rem;
    display: flex; align-items: center; gap: 0.4rem;
  }
  .flourish { color: #8b6914; font-size: 1.3rem; }
  .fac-count { font-size: 0.75rem; opacity: 0.7; font-family: 'Crimson Text', serif; }

  .grid {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(auto-fill, minmax(16rem, 1fr));
  }

  .npc-card {
    position: relative;
    display: flex; flex-direction: column;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    overflow: hidden;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 4px 10px rgba(0,0,0,0.4);
    transition: transform 0.1s, box-shadow 0.1s;
  }
  .npc-card:hover { transform: translateY(-2px); box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 10px 20px rgba(0,0,0,0.55); }
  .npc-card.dead { filter: grayscale(0.8) brightness(0.85); }
  .npc-card.dead::after {
    content: "†"; position: absolute; right: 0.55rem; top: 0.45rem;
    color: #8b1a1a; font-size: 1.2rem; font-family: 'Cinzel', serif;
  }
  .npc-click {
    display: flex; flex-direction: column; gap: 0;
    padding: 0;
    background: transparent; border: 0;
    cursor: pointer;
  }

  .portrait-wrap {
    position: relative;
    aspect-ratio: 3 / 2;
    background:
      linear-gradient(180deg, rgba(139,105,20,0.25), rgba(44,24,16,0.55)),
      radial-gradient(circle at 35% 30%, #c9a84c 0%, #8b6914 55%, #3a2313 100%);
    border-bottom: 1.5px solid #8b6914;
    overflow: hidden;
  }
  .portrait {
    position: absolute; inset: 0;
    display: grid; place-items: center;
  }
  .portrait img {
    width: 100%; height: 100%;
    object-fit: cover;
  }
  .portrait .sigil {
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: 2.5rem;
    color: #1a0f08;
    text-shadow: 0 1px 0 rgba(244,228,193,0.35);
    letter-spacing: 0.04em;
  }
  .att-pip {
    position: absolute; right: 0.55rem; bottom: 0.55rem;
    display: grid; place-items: center;
    width: 1.75rem; height: 1.75rem;
    border-radius: 9999px;
    border: 1.5px solid;
    box-shadow: 0 2px 5px rgba(0,0,0,0.55), inset 0 1px 0 rgba(255,248,220,0.15);
  }

  .npc-body { padding: 0.65rem 0.9rem 0.75rem; }
  .npc-name {
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1.05rem;
    line-height: 1.15;
    color: #2c1810 !important;
    letter-spacing: 0.02em;
  }
  .npc-role {
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.85rem;
    color: #5c3d2e;
    margin-top: 0.1rem;
  }

  .tags {
    display: flex; flex-wrap: wrap; gap: 0.3rem;
    margin-top: 0.4rem;
  }
  .tag {
    padding: 0.1rem 0.45rem;
    border-radius: 0.25rem;
    font-size: 0.6rem;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border: 1px solid rgba(139,105,20,0.35);
  }
  .faction-tag { background: rgba(139,105,20,0.18); color: #6d510f; border-color: rgba(139,105,20,0.5); }

  .lede {
    font-family: 'Crimson Text', serif;
    font-size: 0.85rem;
    line-height: 1.4;
    color: #3a2313;
    margin-top: 0.45rem;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }

  .npc-foot {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.4rem 0.9rem 0.55rem;
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
    padding: 0.3rem;
    border-radius: 0.3rem;
    color: #6d510f;
    background: transparent;
  }
  .icon-btn:hover { background: rgba(139,105,20,0.15); color: #2c1810; }
  .icon-btn.danger { color: #8b1a1a; }
  .icon-btn.danger:hover { background: rgba(139,26,26,0.1); }

  .npc-avatar-lg {
    width: 4.5rem; height: 4.5rem;
    border-radius: 9999px;
    overflow: hidden;
    border: 1.5px solid #8b6914;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 45%, #6d510f 100%);
    display: grid; place-items: center;
    font-family: 'Cinzel', serif; font-weight: 900;
    color: #1a0f08;
    font-size: 1.35rem;
    flex: none;
  }
  .npc-avatar-lg img { width: 100%; height: 100%; object-fit: cover; }

  .pager {
    margin-top: 1.25rem;
    display: flex; justify-content: center; align-items: center;
    gap: 0.35rem;
    flex-wrap: wrap;
  }

  .npc-modal-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.75);
    display: grid; place-items: center;
    z-index: 60;
    padding: 1rem;
  }
  .npc-modal {
    position: relative;
    width: min(42rem, 100%);
    max-height: 90vh;
    overflow-y: auto;
    border-radius: 0.5rem;
    border: 1.5px solid #8b6914;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    padding: 1.5rem 1.75rem;
    color: #2c1810;
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.6),
      0 18px 40px rgba(0,0,0,0.65);
  }
  .npc-modal-close {
    position: absolute; top: 0.5rem; right: 0.5rem;
    display: grid; place-items: center;
    width: 1.75rem; height: 1.75rem;
    border-radius: 9999px;
    background: #3a2313;
    color: #c9a84c;
    border: 1px solid #4e3909;
  }
  .npc-modal-close:hover { background: #4e3909; color: #f7e2a5; }
  .npc-modal-head { display: flex; align-items: flex-start; gap: 1rem; }

  .pg-btn {
    padding: 0.3rem 0.7rem;
    border-radius: 0.3rem;
    border: 1px solid rgba(139,105,20,0.4);
    background: rgba(139,105,20,0.08);
    color: #5c3d2e;
    font-size: 0.85rem;
    font-family: 'Cinzel', serif;
    letter-spacing: 0.03em;
  }
  .pg-btn:hover { background: rgba(201,168,76,0.2); }
  .pg-btn.active {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border-color: #4e3909;
    font-weight: 700;
  }
  .pg-btn:disabled { opacity: 0.4; cursor: default; }
</style>
