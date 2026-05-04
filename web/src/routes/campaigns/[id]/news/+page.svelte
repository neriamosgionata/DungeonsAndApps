<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { News } from '$lib/api/resources';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { Eye, EyeOff, Trash2, Pencil, X, Newspaper, ChevronLeft, ChevronRight } from '@lucide/svelte';

  type News = {
    id: string;
    title: string;
    body?: string | null;
    published_at?: string | null;
    visibility: string;
  };

  const cid = $derived(page.params.id!);
  const campaign = useCampaign();

  let items = $state<News[]>([]);
  let error = $state('');
  let loading = $state(true);

  // create form
  let newTitle = $state('');
  let newBody = $state('');
  let newVis = $state('players');

  // edit modal
  let edit = $state<News | null>(null);

  async function load() {
    try { items = (await News.list(cid)) as unknown as News[]; }
    catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      if ((ev.type as string).startsWith('news_')) load();
    });
  });
  onDestroy(() => offWs?.());

  async function create(close: () => void) {
    try {
      await News.create(cid, { title: newTitle, body: newBody, visibility: newVis });
      newTitle = ''; newBody = ''; newVis = 'players';
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function saveEdit() {
    if (!edit) return;
    try {
      await News.update(edit.id, {
        title: edit.title,
        body: edit.body ?? null,
        visibility: edit.visibility,
      });
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function remove(n: News) {
    if (!confirm($_('news.delete_confirm').replace('{{name}}', n.title))) return;
    try { await News.delete(n.id); await load(); } catch (e) { error = (e as Error).message; }
  }

  async function cycleVis(n: News) {
    const next = n.visibility === 'master' ? 'players' : 'master';
    try { await News.update(n.id, { visibility: next }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  function fmtDate(iso?: string | null): { date: string; time: string } {
    if (!iso) return { date: '', time: '' };
    const d = new Date(iso);
    return {
      date: d.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' }),
      time: d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' }),
    };
  }

  // --- reader: click a card to open a larger article view ---
  let reading = $state<News | null>(null);

  // paginate when no reader open
  const PAGE_SIZE = 6;
  let pageIdx = $state(0);
  const pageCount = $derived(Math.max(1, Math.ceil(items.length / PAGE_SIZE)));
  const pageItems = $derived(items.slice(pageIdx * PAGE_SIZE, (pageIdx + 1) * PAGE_SIZE));
</script>

<section class="gazette">
  <!-- masthead -->
  <header class="mast">
    <div class="mast-left">
      <Newspaper size={28} style="color:#8b6914;" />
    </div>
    <div class="mast-center">
      <h2 class="mast-title">{$_('news.title')}</h2>
      <div class="mast-sub">
        <span class="fleuron">❦</span>
        {$_('news.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="mast-right">
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('news.new')}`} title={$_('news.new')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2">
              <input required placeholder={$_('news.title_ph')} bind:value={newTitle}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <textarea rows="6" placeholder={$_('news.body_ph')} bind:value={newBody}
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
                <button class="rounded-md px-6 py-2 text-white" style="background: linear-gradient(180deg, #c9a84c 0%, #8b6914 60%, #6d510f 100%);">{$_('news.publish')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
  </header>

  <div class="rule"></div>

  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if items.length === 0}
    <p class="empty">{$_('news.empty')}</p>
  {:else}
    <div class="grid">
      {#each pageItems as n (n.id)}
        {@const d = fmtDate(n.published_at)}
        <article class="card">
          <button type="button" class="card-open" onclick={() => reading = n}>
            <div class="dateline">
              {#if d.date}<span>{d.date}</span>{/if}
              {#if d.time}<span class="time">— {d.time}</span>{/if}
            </div>
            <h3 class="headline">{n.title}</h3>
            <div class="rule-thin"></div>
            {#if n.body}
              <p class="lede">{n.body.replace(/^\s*#{1,2}\s*/gm, '').trim()}</p>
            {:else}
              <p class="lede italic" style="color:#8b6355;">{$_('news.no_text')}</p>
            {/if}
          </button>
          <footer class="card-foot">
            <span class="vis" style={n.visibility === 'master'
              ? 'background:rgba(139,26,26,0.2);color:#8b1a1a;border-color:#8b1a1a;'
              : 'background:rgba(139,105,20,0.2);color:#6d510f;border-color:rgba(139,105,20,0.5);'}>
              {#if n.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
              {$_(`visibility.${n.visibility}`)}
            </span>
            {#if campaign().isMaster}
              <div class="master-actions">
                <button onclick={(e) => { e.stopPropagation(); cycleVis(n); }} title="Cycle visibility"
                  class="icon-btn"><Eye size={13} /></button>
                <button onclick={(e) => { e.stopPropagation(); edit = { ...n }; }} title="Edit"
                  class="icon-btn"><Pencil size={13} /></button>
                <button onclick={(e) => { e.stopPropagation(); remove(n); }} title="Delete"
                  class="icon-btn danger"><Trash2 size={13} /></button>
              </div>
            {/if}
          </footer>
        </article>
      {/each}
    </div>

    {#if pageCount > 1}
      <nav class="pager" aria-label="Pagination">
        <button type="button" disabled={pageIdx === 0} onclick={() => pageIdx = Math.max(0, pageIdx - 1)}
          class="pg-btn"><ChevronLeft size={14} /> {$_('news.older')}</button>
        <span class="pg-status">{$_('news.page_of').replace('{{page}}', String(pageIdx + 1)).replace('{{total}}', String(pageCount))}</span>
        <button type="button" disabled={pageIdx >= pageCount - 1} onclick={() => pageIdx = Math.min(pageCount - 1, pageIdx + 1)}
          class="pg-btn">{$_('news.newer')} <ChevronRight size={14} /></button>
      </nav>
    {/if}
  {/if}
</section>

<!-- reader modal -->
{#if reading}
  {@const d = fmtDate(reading.published_at)}
  <div class="reader-backdrop" role="presentation"
    onclick={() => (reading = null)}
    onkeydown={(e) => e.key === 'Escape' && (reading = null)}>
    <div class="reader" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <button class="reader-close" aria-label="close" onclick={() => (reading = null)}>
        <X size={16} />
      </button>
      <div class="reader-mast">
        <Newspaper size={18} style="color:#8b6914;" />
        <span>The Herald</span>
        {#if d.date}<span class="dot">·</span><span>{d.date}</span>{/if}
        {#if d.time}<span class="time">{d.time}</span>{/if}
      </div>
      <h3 class="reader-headline">{reading.title}</h3>
      <div class="rule"></div>
      {#if reading.body}
        <div class="reader-body"><Paragraphs text={reading.body} /></div>
      {:else}
        <p class="italic" style="color:#8b6355;">No text.</p>
      {/if}
    </div>
  </div>
{/if}

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
        <h3 class="font-display text-lg" style="color:#f4e4c1 !important;">{$_('news.edit_title')}</h3>
        <button onclick={() => (edit = null)} aria-label="close" style="color:#c9a84c;"><X size={16} /></button>
      </div>
      <input required placeholder={$_('news.title_ph')} bind:value={edit.title}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      <textarea rows="12" placeholder={$_('common.body')} bind:value={edit.body}
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
  .gazette { max-width: 72rem; margin: 0 auto; padding: 1rem 1.25rem; }

  .mast {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 1rem;
  }
  .mast-left, .mast-right { display: flex; justify-content: center; }
  .mast-center { text-align: center; }
  .mast-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.8rem, 3.5vw, 2.75rem);
    font-weight: 900;
    letter-spacing: 0.08em;
    color: #2c1810;
    text-shadow: 0 1px 0 rgba(244,228,193,0.5);
    line-height: 1;
  }
  .mast-sub {
    margin-top: 0.25rem;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.85rem;
    color: #6d510f;
    letter-spacing: 0.04em;
  }
  .fleuron {
    color: #8b6914;
    margin: 0 0.5rem;
    font-style: normal;
  }

  .rule {
    height: 3px;
    margin: 0.85rem 0 1.25rem;
    background:
      linear-gradient(90deg, transparent 0%, #8b6914 8%, #c9a84c 50%, #8b6914 92%, transparent 100%);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before, .rule::after {
    content: "❦";
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    color: #6d510f;
    background: #f4e4c1;
    padding: 0 0.5rem;
    font-size: 0.9rem;
  }
  .rule::before { left: 45%; }
  .rule::after  { left: 55%; display: none; }

  .rule-thin {
    height: 1px;
    margin: 0.35rem 0 0.5rem;
    background: linear-gradient(90deg, transparent, rgba(139,105,20,0.5), transparent);
  }

  .grid {
    display: grid;
    gap: 1.25rem;
    grid-template-columns: repeat(auto-fill, minmax(18rem, 1fr));
  }

  .card {
    position: relative;
    display: flex; flex-direction: column;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.55),
      0 4px 12px rgba(0,0,0,0.4);
    overflow: hidden;
    transition: transform 0.1s, box-shadow 0.1s;
  }
  .card:hover {
    transform: translateY(-2px);
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.65),
      0 8px 20px rgba(0,0,0,0.55);
  }
  .card-open {
    display: block;
    text-align: left;
    width: 100%;
    padding: 0.85rem 1rem 0.5rem;
    background: transparent;
    color: inherit;
    cursor: pointer;
  }

  .dateline {
    font-family: 'IM Fell English SC', serif;
    font-size: 0.7rem;
    letter-spacing: 0.15em;
    text-transform: uppercase;
    color: #8b6914;
  }
  .dateline .time { font-style: italic; margin-left: 0.25rem; }
  .headline {
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1.2rem;
    line-height: 1.2;
    color: #2c1810 !important;
    margin-top: 0.3rem;
    letter-spacing: 0.02em;
  }
  .lede {
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
    line-height: 1.4;
    color: #3a2313;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 5;
    line-clamp: 5;
    overflow: hidden;
    column-count: 1;
  }

  .card-foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 0.45rem 0.9rem 0.6rem;
    border-top: 1px dashed rgba(139,105,20,0.35);
  }
  .vis {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.6rem;
    letter-spacing: 0.12em;
    text-transform: uppercase;
    padding: 0.1rem 0.5rem;
    border-radius: 0.25rem;
    border: 1px solid;
    font-family: 'Cinzel', serif;
    font-weight: 700;
  }
  .master-actions { display: inline-flex; gap: 0.15rem; }
  .icon-btn {
    padding: 0.3rem;
    border-radius: 0.3rem;
    color: #6d510f;
    background: transparent;
  }
  .icon-btn:hover { background: rgba(139,105,20,0.15); color: #2c1810; }
  .icon-btn.danger { color: #8b1a1a; }
  .icon-btn.danger:hover { background: rgba(139,26,26,0.1); }

  .empty {
    text-align: center;
    padding: 3rem 1rem;
    font-style: italic;
    color: #8b6355;
    font-size: 1rem;
  }

  .pager {
    margin-top: 1.5rem;
    display: flex; justify-content: center; align-items: center;
    gap: 1rem;
    font-family: 'Cinzel', serif;
  }
  .pg-btn {
    display: inline-flex; align-items: center; gap: 0.25rem;
    padding: 0.35rem 0.75rem;
    border-radius: 0.3rem;
    border: 1px solid #4e3909;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    font-size: 0.8rem;
    font-weight: 700;
    letter-spacing: 0.04em;
  }
  .pg-btn:hover { background-image: linear-gradient(180deg, #e5c065 0%, #a98517 55%, #7e5e10 100%); }
  .pg-btn:disabled { opacity: 0.4; cursor: default; }
  .pg-status { color: #6d510f; font-size: 0.75rem; letter-spacing: 0.1em; text-transform: uppercase; }

  /* reader modal */
  .reader-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.75);
    display: grid; place-items: center;
    z-index: 60; padding: 1rem;
  }
  .reader {
    position: relative;
    width: min(56rem, 100%);
    max-height: 90vh;
    overflow-y: auto;
    border: 2px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    padding: 2rem 2.25rem 2.5rem;
    color: #2c1810;
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.6),
      0 18px 40px rgba(0,0,0,0.65);
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
  .reader-mast .dot { opacity: 0.5; }
  .reader-mast .time { font-style: italic; opacity: 0.8; }
  .reader-headline {
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: clamp(1.5rem, 3vw, 2.4rem);
    line-height: 1.1;
    color: #2c1810 !important;
    margin: 0.5rem 0 0.25rem;
    letter-spacing: 0.02em;
  }
  .reader-body {
    font-family: 'Crimson Text', serif;
    font-size: 1.05rem;
    line-height: 1.55;
    color: #2c1810;
    column-count: 2;
    column-gap: 2rem;
    column-rule: 1px dashed rgba(139,105,20,0.35);
  }
  @media (max-width: 640px) {
    .reader-body { column-count: 1; }
  }
</style>
