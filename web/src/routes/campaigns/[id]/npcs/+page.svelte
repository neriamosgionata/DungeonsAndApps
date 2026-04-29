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
  import { Eye, EyeOff, Trash2, Search, X, Pencil } from '@lucide/svelte';

  type Npc = {
    id: string;
    name: string;
    role?: string | null;
    faction_id?: string | null;
    description?: string | null;
    image_key?: string | null;
    visibility: string;
    stats?: { attitude?: string; status?: string } | Record<string, unknown>;
  };
  type Faction = { id: string; name: string; banner_color?: string | null };

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let npcs = $state<Npc[]>([]);
  let factions = $state<Faction[]>([]);
  let error = $state('');
  let q = $state('');
  let filter = $state<string>(''); // '' = all | faction_id | 'none'

  // create form
  let form = $state<Record<string, unknown>>({
    visibility: 'private', stats: { attitude: 'neutrale', status: 'vivo' },
  });

  async function load() {
    try {
      [npcs, factions] = await Promise.all([
        NPCs.list(cid) as Promise<Npc[]>,
        Factions.list(cid) as Promise<Faction[]>,
      ]);
    } catch (e) { error = (e as Error).message; }
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
    if (buckets.__none__?.length) rows.push({ key: '__none__', name: 'Senza fazione', items: buckets.__none__ });
    for (const r of rows) r.items.sort((a, b) => a.name.localeCompare(b.name));
    return rows;
  });

  function stat(n: Npc, key: 'attitude' | 'status'): string | undefined {
    const s = n.stats as Record<string, unknown> | undefined;
    const v = s?.[key];
    return typeof v === 'string' ? v : undefined;
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
      form = { visibility: 'private', stats: { attitude: 'neutrale', status: 'vivo' } };
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function toggleVis(n: Npc) {
    const next = n.visibility === 'players' ? 'private' : 'players';
    await NPCs.update(n.id, { visibility: next });
    await load();
  }

  async function remove(id: string) {
    if (!confirm('Delete NPC?')) return;
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

<section class="mx-auto max-w-6xl px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-2xl font-display font-bold" style="color:#8b1a1a;">× NPC</h2>
    {#if campaign().isMaster}
      <CollapsibleAdd label="Nuovo PNG" title="Nuovo PNG" alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2 sm:grid-cols-2">
            <div class="sm:col-span-2">
              <ImageUpload
                value={(form.image_key as string | null) ?? null}
                kind="npc" label="Portrait"
                onchange={(url) => form.image_key = url} />
            </div>
            <input required placeholder="Name" bind:value={form.name}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <input placeholder="Role" bind:value={form.role}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <select bind:value={form.faction_id}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value={null}>— no faction —</option>
              {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
            </select>
            <select
              value={(form.stats as { attitude?: string })?.attitude ?? 'neutrale'}
              onchange={(e) => form.stats = { ...(form.stats as object), attitude: (e.currentTarget as HTMLSelectElement).value }}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              {#each ATTITUDES as a (a)}<option value={a}>{a}</option>{/each}
            </select>
            <select
              value={(form.stats as { status?: string })?.status ?? 'vivo'}
              onchange={(e) => form.stats = { ...(form.stats as object), status: (e.currentTarget as HTMLSelectElement).value }}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              {#each STATUSES as s (s)}<option value={s}>{s}</option>{/each}
            </select>
            <select bind:value={form.visibility}
              class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
              <option value="private">private</option>
              <option value="players">players</option>
              <option value="public">public</option>
            </select>
            <textarea placeholder="Description (use blank lines between paragraphs, # Title for headings)"
              bind:value={form.description} rows="4"
              class="sm:col-span-2 rounded bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
            <div class="sm:col-span-2 flex justify-end">
              <button class="rounded bg-violet-600 px-6 py-2 text-white">Crea</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>

  <div class="mt-4 divider-cog"></div>

  <!-- search + filter -->
  <div class="mt-4 grid gap-2 sm:grid-cols-[1fr_16rem]">
    <label class="relative block">
      <Search size={14} class="absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none z-10"
        style="color:#8b6355;" />
      <input placeholder="Cerca personaggio…" bind:value={q}
        class="w-full rounded bg-neutral-900 border border-neutral-700 py-2"
        style="padding-left: 2.25rem !important; padding-right: 0.75rem !important;" />
    </label>
    <select bind:value={filter}
      class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
      <option value="">Tutti</option>
      {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
      <option value="none">Senza fazione</option>
    </select>
  </div>

  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}

  <p class="mt-4 text-sm" style="color:#8b6355;">
    {visible.length} personaggi
    {#if paginated && pageCount > 1}
      · pagina {pageIdx + 1} di {pageCount}
    {/if}
  </p>

  {#each groups as g (g.key)}
    <div class="mt-6">
      <div class="text-xs tracking-widest font-display font-bold uppercase" style="color:#8b1a1a;">
        {g.name} <span style="color:#8b6355;">({g.items.length})</span>
      </div>
      <div class="mt-2 grid gap-3 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
        {#each g.items as n (n.id)}
          <article class="npc-card">
            <div class="npc-click" role="button" tabindex="0"
              onclick={() => detail = n}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && (detail = n)}>
              <header class="npc-head">
                <div class="npc-avatar">
                  {#if n.image_key}
                    <img src={n.image_key} alt="" />
                  {:else}
                    <span>{sigil(n.name)}</span>
                  {/if}
                </div>
                <div class="min-w-0">
                  <div class="font-display font-bold leading-tight">{n.name}</div>
                  {#if n.role}<div class="italic text-sm" style="color:#5c3d2e;">{n.role}</div>{/if}
                </div>
              </header>

              <div class="mt-2 flex flex-wrap gap-1 text-[10px] font-display tracking-widest uppercase">
                {#if stat(n, 'attitude')}
                  <span class="tag {attitudeColor(stat(n, 'attitude'))}">{stat(n, 'attitude')}</span>
                {/if}
                {#if stat(n, 'status')}
                  <span class="tag {statusColor(stat(n, 'status'))}">{stat(n, 'status')}</span>
                {/if}
                {#if n.faction_id && factionsById[n.faction_id]}
                  <span class="tag bg-neutral-700/40 text-neutral-100">{factionsById[n.faction_id].name}</span>
                {/if}
              </div>

              {#if n.description}
                <p class="mt-2 text-xs italic line-clamp-2" style="color:#5c3d2e;">{n.description}</p>
              {/if}
            </div>

            {#if campaign().isMaster}
              <footer class="npc-foot">
                <button onclick={(e) => { e.stopPropagation(); toggleVis(n); }}
                  class="inline-flex items-center gap-1 rounded px-2 py-0.5 text-[10px]"
                  style="background:rgba(43,29,16,0.15); color:#5c3d2e;">
                  {#if n.visibility === 'private'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
                  {n.visibility}
                </button>
                <div class="flex items-center gap-2">
                  <button onclick={(e) => { e.stopPropagation(); edit = { ...n, stats: { ...(n.stats as object ?? {}) } }; }}
                    title="Edit" class="text-[10px] inline-flex items-center gap-1" style="color:#8b6914;">
                    <Pencil size={10} /> edit
                  </button>
                  <button onclick={(e) => { e.stopPropagation(); remove(n.id); }}
                    class="text-red-600 text-[10px] inline-flex items-center gap-1">
                    <Trash2 size={10} /> delete
                  </button>
                </div>
              </footer>
            {/if}
          </article>
        {/each}
      </div>
    </div>
  {/each}

  {#if visible.length === 0}
    <p class="mt-8 text-center italic" style="color:#8b6355;">Nessun risultato.</p>
  {/if}

  {#if paginated && pageCount > 1}
    <nav class="mt-6 flex items-center justify-center gap-2" aria-label="Pagination">
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
      <h3 class="font-display font-bold text-xl" style="color:#2c1810;">Edit NPC</h3>
      <div class="mt-4 grid gap-2 sm:grid-cols-2">
        <div class="sm:col-span-2">
          <ImageUpload
            value={(edit.image_key as string | null) ?? null}
            kind="npc" label="Portrait"
            onchange={(url) => edit && (edit.image_key = url)} />
        </div>
        <input required placeholder="Name" bind:value={edit.name}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input placeholder="Role" bind:value={edit.role}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <select bind:value={edit.faction_id}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value={null}>— no faction —</option>
          {#each factions as f (f.id)}<option value={f.id}>{f.name}</option>{/each}
        </select>
        <select
          value={(edit.stats as { attitude?: string })?.attitude ?? 'neutrale'}
          onchange={(e) => edit && (edit.stats = { ...(edit.stats as object ?? {}), attitude: (e.currentTarget as HTMLSelectElement).value })}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          {#each ATTITUDES as a (a)}<option value={a}>{a}</option>{/each}
        </select>
        <select
          value={(edit.stats as { status?: string })?.status ?? 'vivo'}
          onchange={(e) => edit && (edit.stats = { ...(edit.stats as object ?? {}), status: (e.currentTarget as HTMLSelectElement).value })}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          {#each STATUSES as s (s)}<option value={s}>{s}</option>{/each}
        </select>
        <select bind:value={edit.visibility}
          class="rounded bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value="private">private</option>
          <option value="players">players</option>
          <option value="public">public</option>
        </select>
        <textarea placeholder="Description (use blank lines between paragraphs, # Title for headings)"
          bind:value={edit.description} rows="8"
          class="sm:col-span-2 rounded bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      </div>
      <footer class="mt-4 flex justify-end gap-2">
        <button onclick={() => (edit = null)}
          class="rounded-md border border-neutral-700 px-4 py-2 text-sm">Cancel</button>
        <button onclick={saveEdit} class="rounded-md bg-violet-600 px-6 py-2 text-sm text-white">Save</button>
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
              <span class="tag {attitudeColor(stat(detail, 'attitude'))}">{stat(detail, 'attitude')}</span>
            {/if}
            {#if stat(detail, 'status')}
              <span class="tag {statusColor(stat(detail, 'status'))}">{stat(detail, 'status')}</span>
            {/if}
            {#if detail.faction_id && factionsById[detail.faction_id]}
              <span class="tag bg-neutral-700/40 text-neutral-100">{factionsById[detail.faction_id].name}</span>
            {/if}
            <span class="tag bg-neutral-700/40 text-neutral-100 inline-flex items-center gap-1">
              {#if detail.visibility === 'private'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
              {detail.visibility}
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
        <p class="mt-4 italic" style="color:#8b6355;">Nessuna descrizione.</p>
      {/if}

      {#if campaign().isMaster}
        <footer class="mt-6 flex items-center gap-3 justify-end pt-3" style="border-top:1px dashed rgba(139,105,20,0.35);">
          <button onclick={() => { toggleVis(detail!); }}
            class="inline-flex items-center gap-1.5 rounded px-3 py-1.5 text-sm"
            style="background:rgba(43,29,16,0.15); color:#5c3d2e;">
            {#if detail.visibility === 'private'}<EyeOff size={14} />{:else}<Eye size={14} />{/if}
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
  .npc-card {
    position: relative;
    border-radius: 0.5rem;
    border: 1.5px solid #8b6914;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    padding: 0.85rem;
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 6px rgba(0,0,0,0.4);
  }
  .npc-head {
    display: flex; align-items: center; gap: 0.75rem;
  }
  .npc-avatar {
    width: 2.75rem; height: 2.75rem; flex: none;
    border-radius: 9999px;
    overflow: hidden;
    border: 1.5px solid #8b6914;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 45%, #6d510f 100%);
    display: grid; place-items: center;
    font-family: 'Cinzel', serif;
    font-weight: 900;
    color: #1a0f08;
    font-size: 0.85rem;
  }
  .npc-avatar img { width: 100%; height: 100%; object-fit: cover; }

  .tag {
    padding: 0.1rem 0.5rem;
    border-radius: 0.25rem;
    letter-spacing: 0.1em;
    font-weight: 700;
    border: 1px solid rgba(139,105,20,0.35);
  }

  .npc-foot {
    margin-top: 0.75rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-top: 1px dashed rgba(139,105,20,0.35);
    padding-top: 0.5rem;
  }

  .npc-click {
    display: block;
    width: 100%;
    text-align: left;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }
  .npc-click:hover { filter: brightness(1.03); }

  .npc-avatar-lg { width: 4.5rem; height: 4.5rem; font-size: 1.35rem; }

  :global(.line-clamp-2) {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
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
