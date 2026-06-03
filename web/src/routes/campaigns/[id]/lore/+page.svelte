<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Lore } from '$lib/api/resources';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Markdown from '$lib/components/Markdown.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { BookOpen, Eye, EyeOff, Trash2, Pencil, X, Search, Feather } from '@lucide/svelte';

  type LoreEntry = {
    id: string;
    title: string;
    category?: string | null;
    body?: string | null;
    visibility: string;
  };

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let items = $state<LoreEntry[]>([]);
  let error = $state('');
  let loading = $state(true);
  let q = $state('');
  let catFilter = $state<string>('');

  // create form
  let newTitle = $state('');
  let newCategory = $state('');
  let newBody = $state('');
  let newVis = $state('master');

  // edit modal
  let edit = $state<LoreEntry | null>(null);

  // reader modal
  let reading = $state<LoreEntry | null>(null);

  async function load() {
    try { items = (await Lore.list(cid)) as unknown as LoreEntry[]; }
    catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      if ((ev.type as string).startsWith('lore_')) load();
    });
  });
  onDestroy(() => offWs?.());

  async function create(close: () => void) {
    try {
      await Lore.create(cid, {
        title: newTitle,
        category: newCategory || null,
        body: newBody || null,
        visibility: newVis,
      });
      newTitle = ''; newCategory = ''; newBody = ''; newVis = 'master';
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function saveEdit() {
    if (!edit) return;
    try {
      await Lore.update(edit.id, {
        title: edit.title,
        category: edit.category ?? null,
        body: edit.body ?? null,
        visibility: edit.visibility,
      });
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function remove(l: LoreEntry) {
    if (!confirm($_('lore.delete_confirm').replace('{{name}}', l.title))) return;
    try { await Lore.delete(l.id); await load(); } catch (e) { error = (e as Error).message; }
  }
  async function cycleVis(l: LoreEntry) {
    const next = l.visibility === 'master' ? 'players' : 'master';
    try { await Lore.update(l.id, { visibility: next }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  // --- derived ---
  const categories = $derived<string[]>([
    ...new Set(items.map((l) => (l.category ?? '').trim()).filter(Boolean))
  ].sort((a, b) => a.localeCompare(b)));

  const visible = $derived.by(() => {
    const needle = q.trim().toLowerCase();
    return items.filter((l) => {
      if (catFilter && (l.category ?? '').trim() !== catFilter) return false;
      if (!needle) return true;
      return (
        l.title.toLowerCase().includes(needle) ||
        (l.category ?? '').toLowerCase().includes(needle) ||
        (l.body ?? '').toLowerCase().includes(needle)
      );
    });
  });

  const groups = $derived.by<Array<{ key: string; items: LoreEntry[] }>>(() => {
    const buckets: Record<string, LoreEntry[]> = {};
    for (const l of visible) {
      const k = (l.category ?? '').trim() || $_('lore.uncategorized');
      (buckets[k] ??= []).push(l);
    }
    const rows = Object.keys(buckets).sort((a, b) => {
      if (a === $_('lore.uncategorized')) return 1;
      if (b === $_('lore.uncategorized')) return -1;
      return a.localeCompare(b);
    }).map((k) => ({ key: k, items: buckets[k].slice().sort((a, b) => a.title.localeCompare(b.title)) }));
    return rows;
  });

  function snippet(body: string | null | undefined, n = 140): string {
    if (!body) return '';
    const cleaned = body
      .replace(/^\s*#{1,3}\s*.+\n?/gm, '')
      .replace(/\*\*(.+?)\*\*/g, '$1')
      .replace(/\*(.+?)\*/g, '$1')
      .replace(/`([^`]+)`/g, '$1')
      .replace(/\[([^\]]+)\]\([^)]+\)/g, '$1')
      .replace(/\[\[([^\]]+)\]\]/g, '$1')
      .replace(/^\s*[-*]\s+/gm, '• ')
      .trim();
    if (cleaned.length <= n) return cleaned;
    return cleaned.slice(0, n).trim() + '…';
  }

  function openWikiLink(title: string) {
    const found = items.find((l) => l.title.toLowerCase() === title.toLowerCase());
    if (found) {
      reading = found;
    } else {
      // Create filtered search for partial matches
      q = title;
    }
  }
</script>

<section class="codex">
  <!-- header -->
  <header class="codex-head">
    <div class="hdr-icon"><BookOpen size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('lore.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('lore.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('lore.new')}`} title={$_('lore.new_title')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2">
              <input required placeholder={$_('lore.title_ph')} bind:value={newTitle}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <input placeholder={$_('lore.category_ph')} bind:value={newCategory}
                list="lore-categories"
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <textarea rows="8" placeholder={$_('lore.body_ph')}
                bind:value={newBody}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
              <label class="flex gap-2 items-center">
                <span class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">{$_('visibility.label')}</span>
                <select bind:value={newVis}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 flex-1">
                  <option value="master">{$_('visibility.master')}</option>
                  <option value="players">{$_('visibility.players')}</option>
                </select>
              </label>
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('lore.new')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
  </header>

  <div class="rule"></div>

  <!-- search + category filter -->
  <div class="toolbar">
    <label class="search">
      <Search size={14} style="color:#8b6355;" />
      <input placeholder={$_('lore.search_ph')} bind:value={q} />
    </label>
    <select bind:value={catFilter} class="cat-filter">
      <option value="">{$_('lore.all_categories').replace('{{n}}', String(items.length))}</option>
      {#each categories as cat (cat)}
        <option value={cat}>{cat}</option>
      {/each}
    </select>
  </div>

  <datalist id="lore-categories">
    {#each categories as cat (cat)}
      <option value={cat}></option>
    {/each}
  </datalist>

  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if items.length === 0}
    <p class="empty">{$_('lore.empty')}</p>
  {:else if visible.length === 0}
    <p class="empty">{$_('lore.empty_filtered')}</p>
  {:else}
    {#each groups as g (g.key)}
      <div class="category">
        <h3 class="category-name">
          <span class="cat-flourish">§</span> {g.key}
          <span class="cat-count">({g.items.length})</span>
        </h3>
        <div class="grid">
          {#each g.items as l (l.id)}
            <article class="tome">
              <button type="button" class="tome-open" onclick={() => reading = l}>
                <div class="tome-spine"></div>
                <div class="tome-body">
                  <div class="tome-topbar">
                    <Feather size={14} style="color:#8b6914;" />
                    {#if l.category}<span class="tome-cat">{l.category}</span>{/if}
                  </div>
                  <h4 class="tome-title">{l.title}</h4>
                  {#if l.body}
                    <p class="tome-snippet">{snippet(l.body)}</p>
                  {:else}
                    <p class="tome-snippet italic" style="color:#8b6355;">{$_('lore.no_text')}</p>
                  {/if}
                </div>
              </button>
              <footer class="tome-foot">
                <span class="vis" style={l.visibility === 'master'
                  ? 'background:rgba(139,26,26,0.2);color:#8b1a1a;border-color:#8b1a1a;'
                  : 'background:rgba(139,105,20,0.2);color:#6d510f;border-color:rgba(139,105,20,0.5);'}>
                  {#if l.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
                  {$_(`visibility.${l.visibility}`)}
                </span>
                {#if campaign().isMaster}
                  <div class="tome-actions">
                    <button onclick={(e) => { e.stopPropagation(); cycleVis(l); }} title="Cycle visibility" class="icon-btn"><Eye size={13} /></button>
                    <button onclick={(e) => { e.stopPropagation(); edit = { ...l }; }} title="Edit" class="icon-btn"><Pencil size={13} /></button>
                    <button onclick={(e) => { e.stopPropagation(); remove(l); }} title="Delete" class="icon-btn danger"><Trash2 size={13} /></button>
                  </div>
                {/if}
              </footer>
            </article>
          {/each}
        </div>
      </div>
    {/each}
  {/if}
</section>

<!-- reader -->
{#if reading}
  <div class="reader-backdrop" role="presentation"
    onclick={() => (reading = null)}
    onkeydown={(e) => e.key === 'Escape' && (reading = null)}>
    <div class="reader" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <button class="reader-close" aria-label="close" onclick={() => (reading = null)}>
        <X size={16} />
      </button>
      <div class="reader-mast">
        <BookOpen size={16} style="color:#8b6914;" />
        {#if reading.category}<span>{reading.category}</span>{/if}
      </div>
      <h3 class="reader-title">{reading.title}</h3>
      <div class="rule"></div>
      {#if reading.body}
        <div class="reader-body"><Markdown text={reading.body} onWikiLink={openWikiLink} /></div>
      {:else}
        <p class="italic" style="color:#8b6355;">{$_('lore.no_text')}</p>
      {/if}
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
        <h3 class="font-display text-lg" style="color:#f4e4c1 !important;">{$_('lore.edit_title')}</h3>
        <button onclick={() => (edit = null)} aria-label="close" style="color:#c9a84c;"><X size={16} /></button>
      </div>
      <input required placeholder={$_('common.title')} bind:value={edit.title}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      <input placeholder={$_('common.category')} bind:value={edit.category} list="lore-categories"
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      <textarea rows="14" placeholder={$_('common.body')} bind:value={edit.body}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      <select bind:value={edit.visibility}
        class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
        <option value="master">{$_('visibility.master')}</option>
        <option value="players">{$_('visibility.players')}</option>
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
  .codex { max-width: 72rem; margin: 0 auto; padding: 1rem 1.25rem; }
  @media (max-width: 639px) { .codex { padding: 0.5rem 0.6rem; } }

  .codex-head {
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

  .toolbar {
    display: grid;
    grid-template-columns: 1fr 14rem;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
  }
  @media (max-width: 640px) { .toolbar { grid-template-columns: 1fr; } }
  .search {
    display: flex; align-items: center; gap: 0.5rem;
    border: 1.5px solid rgba(139,105,20,0.5);
    border-radius: 0.3rem;
    padding: 0 0.65rem;
    background: #f4e4c1;
  }
  .search input {
    flex: 1;
    background: transparent !important;
    border: 0 !important;
    padding: 0.4rem 0.25rem !important;
    font-family: 'Crimson Text', serif;
    color: #2c1810 !important;
    outline: none;
    box-shadow: none !important;
  }
  .cat-filter {
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    border-radius: 0.3rem !important;
    background: #f4e4c1 !important;
    color: #2c1810 !important;
    font-family: 'Cinzel', serif;
    padding: 0.4rem 0.65rem !important;
  }

  .empty {
    text-align: center;
    padding: 3rem 1rem;
    font-style: italic;
    color: #8b6355;
  }

  .category { margin-bottom: 2rem; }
  .category-name {
    font-family: 'IM Fell English SC', serif;
    color: #6d510f;
    font-size: 1.1rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border-bottom: 1px dashed rgba(139,105,20,0.4);
    padding-bottom: 0.35rem;
    margin-bottom: 1rem;
    display: flex; align-items: center; gap: 0.4rem;
  }
  .cat-flourish { color: #8b6914; font-size: 1.3rem; }
  .cat-count { font-size: 0.75rem; opacity: 0.7; font-family: 'Crimson Text', serif; }

  .grid {
    display: grid;
    gap: 1rem;
    grid-template-columns: repeat(auto-fill, minmax(17rem, 1fr));
  }

  .tome {
    position: relative;
    display: flex; flex-direction: column;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 4px 10px rgba(0,0,0,0.4);
    overflow: hidden;
    transition: transform 0.1s, box-shadow 0.1s;
  }
  .tome:hover { transform: translateY(-2px); box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 10px 22px rgba(0,0,0,0.55); }
  .tome-open { display: flex; background: transparent; cursor: pointer; text-align: left; color: inherit; width: 100%; }
  .tome-spine {
    width: 8px; flex: none;
    background: repeating-linear-gradient(
      180deg,
      #6d510f 0 4px,
      #4e3909 4px 8px,
      #8b6914 8px 12px,
      #4e3909 12px 16px
    );
    border-right: 1px solid #4e3909;
  }
  .tome-body { flex: 1; padding: 0.85rem 1rem; min-width: 0; }
  .tome-topbar {
    display: flex; align-items: center; gap: 0.4rem;
    font-family: 'Cinzel', serif;
    font-size: 0.65rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #8b6914;
  }
  .tome-cat { font-weight: 700; }
  .tome-title {
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1.1rem;
    line-height: 1.2;
    color: #2c1810 !important;
    margin-top: 0.35rem;
    letter-spacing: 0.02em;
  }
  .tome-snippet {
    font-family: 'Crimson Text', serif;
    font-size: 0.9rem;
    line-height: 1.4;
    color: #3a2313;
    margin-top: 0.4rem;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 4;
    line-clamp: 4;
    overflow: hidden;
  }

  .tome-foot {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.4rem 0.85rem 0.55rem 1.1rem;
    border-top: 1px dashed rgba(139,105,20,0.35);
  }
  .vis {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.6rem; letter-spacing: 0.12em; text-transform: uppercase;
    padding: 0.1rem 0.5rem; border-radius: 0.25rem;
    border: 1px solid;
    font-family: 'Cinzel', serif; font-weight: 700;
  }
  .tome-actions { display: inline-flex; gap: 0.15rem; }
  .icon-btn {
    padding: 0.3rem;
    border-radius: 0.3rem;
    color: #6d510f;
    background: transparent;
  }
  .icon-btn:hover { background: rgba(139,105,20,0.15); color: #2c1810; }
  .icon-btn.danger { color: #8b1a1a; }
  .icon-btn.danger:hover { background: rgba(139,26,26,0.1); }

  /* reader */
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
    overflow-y: auto;
    border: 2px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    padding: 2rem 2.25rem 2.5rem;
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 18px 40px rgba(0,0,0,0.65);
  }
  .reader-close {
    position: absolute; top: 0.6rem; right: 0.6rem;
    width: 2rem; height: 2rem;
    display: grid; place-items: center;
    border-radius: 9999px;
    background: #3a2313;
    color: #c9a84c;
    border: 1px solid #4e3909;
  }
  .reader-close:hover { background: #4e3909; color: #f7e2a5; }
  .reader-mast {
    display: flex; align-items: center; gap: 0.45rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.8rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: #6d510f;
  }
  .reader-title {
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: clamp(1.5rem, 3vw, 2.2rem);
    line-height: 1.1;
    color: #2c1810 !important;
    margin: 0.5rem 0 0.25rem;
    letter-spacing: 0.02em;
  }
  .reader-body {
    font-family: 'Crimson Text', serif;
    font-size: 1.05rem;
    line-height: 1.6;
    color: #2c1810;
  }
</style>
