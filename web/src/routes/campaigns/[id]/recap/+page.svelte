<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Sessions } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { ScrollText, Eye, EyeOff, Pencil, Trash2, X, Calendar, Hash, BookOpen } from '@lucide/svelte';

  type Session = {
    id: string;
    title: string;
    session_number?: number | null;
    played_at?: string | null;
    recap?: string | null;
    visibility: string;
  };

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);
  let list = $state<Session[]>([]);
  let error = $state('');

  // new session form
  let title = $state('');
  let recap = $state('');
  let num = $state<number | undefined>(undefined);
  let playedAt = $state('');
  let visibility = $state('players');

  // edit + reader modals
  let edit = $state<Session | null>(null);
  let reading = $state<Session | null>(null);

  async function load() {
    try { list = (await Sessions.list(cid)) as unknown as Session[]; }
    catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      if ((ev.type as string).startsWith('session_')) load();
    });
  });
  onDestroy(() => offWs?.());

  async function create(close: () => void) {
    try {
      await Sessions.create(cid, {
        title, recap, session_number: num, visibility,
        played_at: playedAt || undefined,
      });
      title = ''; recap = ''; num = undefined; playedAt = ''; visibility = 'players';
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function saveEdit() {
    if (!edit) return;
    try {
      await Sessions.update(edit.id, {
        title: edit.title,
        session_number: edit.session_number ?? null,
        played_at: edit.played_at || null,
        recap: edit.recap ?? null,
        visibility: edit.visibility,
      });
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }
  async function remove(s: Session) {
    if (!confirm($_('news.delete_confirm').replace('{{name}}', s.title))) return;
    try { await Sessions.delete(s.id); await load(); }
    catch (e) { error = (e as Error).message; }
  }
  async function toggleVis(s: Session) {
    const next = s.visibility === 'master' ? 'players' : 'master';
    try { await Sessions.update(s.id, { visibility: next }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  function fmtDate(d?: string | null): { month: string; day: string; year: string; full: string } {
    if (!d) return { month: '', day: '', year: '', full: '' };
    try {
      const dt = new Date(d);
      return {
        month: dt.toLocaleDateString(undefined, { month: 'short' }).toUpperCase(),
        day: String(dt.getDate()),
        year: String(dt.getFullYear()),
        full: dt.toLocaleDateString(undefined, { year: 'numeric', month: 'long', day: 'numeric' }),
      };
    } catch { return { month: '', day: '', year: '', full: '' }; }
  }

  function snippet(body: string | null | undefined, n = 180): string {
    if (!body) return '';
    const cleaned = body.replace(/^\s*#{1,2}\s*.+\n?/g, '').trim();
    if (cleaned.length <= n) return cleaned;
    return cleaned.slice(0, n).trim() + '…';
  }

  // sort desc by session_number then played_at
  const sorted = $derived.by(() => {
    return [...list].sort((a, b) => {
      const an = a.session_number ?? 0;
      const bn = b.session_number ?? 0;
      if (an !== bn) return bn - an;
      const ad = a.played_at ? Date.parse(a.played_at) : 0;
      const bd = b.played_at ? Date.parse(b.played_at) : 0;
      return bd - ad;
    });
  });
</script>

<section class="chronicle">
  <!-- header -->
  <header class="chr-head">
    <div class="hdr-icon"><ScrollText size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('recap.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('recap.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div class="hdr-right">
      {#if campaign().isMaster}
        <CollapsibleAdd label={$_('recap.new')} title={$_('recap.new')} alignEnd={true}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="space-y-2">
              <div class="flex gap-2 flex-wrap">
                <input required placeholder={$_('recap.title_ph')} bind:value={title}
                  class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
                <input type="number" placeholder="#" bind:value={num}
                  class="w-20 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
                <input type="date" bind:value={playedAt}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              </div>
              <label class="flex items-center gap-2">
                <span class="text-[10px] uppercase tracking-widest font-display" style="color:#8b6914;">Visibility</span>
                <select bind:value={visibility}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 flex-1">
                  <option value="master">{$_('visibility.master')}</option>
                  <option value="players">{$_('visibility.players')}</option>
                </select>
              </label>
              <textarea rows="8" placeholder={$_('recap.body_ph')} bind:value={recap}
                class="block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
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

  {#if list.length === 0}
    <p class="empty">{$_('recap.empty')}</p>
  {:else}
    <ol class="timeline">
      {#each sorted as s, i (s.id)}
        {@const d = fmtDate(s.played_at)}
        {@const hasBody = !!s.recap}
        <li class="entry">
          <!-- left rail: date ribbon + session # sigil -->
          <div class="rail">
            {#if d.day}
              <div class="date-ribbon">
                <span class="month">{d.month}</span>
                <span class="day">{d.day}</span>
                <span class="year">{d.year}</span>
              </div>
            {/if}
            <div class="sigil" title={s.session_number ? $_('recap.session_number').replace('{{n}}', String(s.session_number)) : $_('nav.recap')}>
              {#if s.session_number != null}
                <span class="sigil-hash"><Hash size={11} /></span>
                <span class="sigil-num">{s.session_number}</span>
              {:else}
                <ScrollText size={16} />
              {/if}
            </div>
            {#if i < sorted.length - 1}
              <div class="rail-line"></div>
            {/if}
          </div>

          <!-- right: chapter card -->
          <article class="chapter">
            <button type="button" class="chapter-open" onclick={() => reading = s} disabled={!hasBody}>
              <div class="chap-topbar">
                {#if d.full}
                  <span class="date-full"><Calendar size={12} /> {d.full}</span>
                {/if}
              </div>
              <h3 class="chap-title">{s.title}</h3>
              {#if hasBody}
                <p class="chap-lede">{snippet(s.recap)}</p>
                <span class="read-more"><BookOpen size={11} /> {$_('recap.read_chapter')}</span>
              {:else}
                <p class="chap-lede italic" style="color:#8b6355;">No recap written yet.</p>
              {/if}
            </button>
            <footer class="chap-foot">
              <span class="vis" style={s.visibility === 'master'
                ? 'background:rgba(139,26,26,0.2);color:#8b1a1a;border-color:#8b1a1a;'
                : 'background:rgba(139,105,20,0.2);color:#6d510f;border-color:rgba(139,105,20,0.5);'}>
                {#if s.visibility === 'master'}<EyeOff size={10} />{:else}<Eye size={10} />{/if}
                {$_(`visibility.${s.visibility}`)}
              </span>
              {#if campaign().isMaster}
                <div class="chap-actions">
                  <button onclick={(e) => { e.stopPropagation(); toggleVis(s); }} title="Cycle visibility" class="icon-btn"><Eye size={13} /></button>
                  <button onclick={(e) => { e.stopPropagation(); edit = { ...s }; }} title="Edit" class="icon-btn"><Pencil size={13} /></button>
                  <button onclick={(e) => { e.stopPropagation(); remove(s); }} title="Delete" class="icon-btn danger"><Trash2 size={13} /></button>
                </div>
              {/if}
            </footer>
          </article>
        </li>
      {/each}
    </ol>
  {/if}
</section>

<!-- reader -->
{#if reading}
  {@const d = fmtDate(reading.played_at)}
  <div class="reader-backdrop" role="presentation"
    onclick={() => (reading = null)}
    onkeydown={(e) => e.key === 'Escape' && (reading = null)}>
    <div class="reader" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <button class="reader-close" aria-label="close" onclick={() => (reading = null)}>
        <X size={16} />
      </button>
      <div class="reader-mast">
        <ScrollText size={16} style="color:#8b6914;" />
        {#if reading.session_number != null}<span>{$_('recap.session_number').replace('{{n}}', String(reading.session_number))}</span>{/if}
        {#if d.full}<span class="dot">·</span><span>{d.full}</span>{/if}
      </div>
      <h3 class="reader-title">{reading.title}</h3>
      <div class="rule"></div>
      {#if reading.recap}
        <div class="reader-body"><Paragraphs text={reading.recap} /></div>
      {:else}
        <p class="italic" style="color:#8b6355;">{$_('recap.no_body')}</p>
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
        <h3 class="font-display text-lg" style="color:#f4e4c1 !important;">{$_('recap.edit_title')}</h3>
        <button onclick={() => edit = null} aria-label="close" style="color:#c9a84c;"><X size={18} /></button>
      </div>
      <div class="flex gap-2 flex-wrap">
        <input required placeholder={$_('common.title')} bind:value={edit.title}
          class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input type="number" placeholder="#" bind:value={edit.session_number}
          class="w-20 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input type="date" value={edit.played_at ? edit.played_at.slice(0,10) : ''}
          onchange={(e) => edit && (edit.played_at = (e.currentTarget as HTMLInputElement).value || null)}
          class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <select bind:value={edit.visibility} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value="master">master</option><option value="players">players</option>
        </select>
      </div>
      <textarea rows="14" placeholder={$_('recap.body_ph')}
        bind:value={edit.recap}
        class="block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
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
  .chronicle { max-width: 64rem; margin: 0 auto; padding: 1rem 1.25rem; }

  .chr-head {
    display: grid; grid-template-columns: auto 1fr auto; align-items: center;
    gap: 1rem;
  }
  .hdr-icon, .hdr-right { display: flex; justify-content: center; }
  .hdr-center { text-align: center; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.6rem, 3vw, 2.3rem);
    font-weight: 900;
    letter-spacing: 0.08em;
    color: #2c1810 !important;
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
    margin: 0.85rem 0 1.5rem;
    background: linear-gradient(90deg, transparent, #8b6914 10%, #c9a84c 50%, #8b6914 90%, transparent);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦";
    position: absolute; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    color: #6d510f; background: #f4e4c1;
    padding: 0 0.5rem; font-size: 0.9rem;
  }

  .err { color: #c95a5a; font-size: 0.85rem; margin: 0.5rem 0; }
  .empty { text-align: center; padding: 3rem; font-style: italic; color: #8b6355; }

  .timeline {
    list-style: none;
    padding: 0;
    margin: 0;
    counter-reset: chapter;
  }
  .entry {
    display: grid;
    grid-template-columns: 6rem 1fr;
    gap: 1rem;
    position: relative;
  }
  .entry + .entry { margin-top: 1.25rem; }

  .rail {
    display: flex; flex-direction: column; align-items: center;
    gap: 0.5rem;
    position: relative;
  }
  .rail-line {
    flex: 1;
    width: 3px;
    margin-top: 0.5rem;
    background:
      linear-gradient(180deg, #8b6914, #4e3909, #8b6914);
    border-radius: 2px;
    box-shadow: inset 0 0 0 1px rgba(255,248,220,0.15);
    min-height: 2rem;
  }

  .date-ribbon {
    width: 100%;
    display: flex; flex-direction: column; align-items: center;
    padding: 0.35rem 0.25rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.3rem;
    background: linear-gradient(180deg, #f7e2a5 0%, #c9a84c 70%, #6d510f 100%);
    color: #1a0f08;
    font-family: 'Cinzel', serif;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 2px 4px rgba(0,0,0,0.4);
  }
  .date-ribbon .month { font-size: 0.65rem; letter-spacing: 0.15em; font-weight: 700; }
  .date-ribbon .day { font-size: 1.4rem; font-weight: 900; line-height: 1; }
  .date-ribbon .year { font-size: 0.6rem; letter-spacing: 0.1em; opacity: 0.75; }

  .sigil {
    width: 2.5rem; height: 2.5rem;
    display: grid; place-items: center;
    border-radius: 9999px;
    border: 2px solid #4e3909;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 55%, #6d510f 100%);
    color: #1a0f08;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 6px rgba(0,0,0,0.5);
    position: relative;
  }
  .sigil-hash { position: absolute; top: 4px; color: #6d510f; }
  .sigil-num {
    font-family: 'Cinzel', serif; font-weight: 900;
    font-size: 1rem; line-height: 1;
    margin-top: 0.25rem;
  }

  .chapter {
    position: relative;
    border: 1.5px solid #8b6914;
    border-radius: 0.4rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 4px 12px rgba(0,0,0,0.4);
    overflow: hidden;
  }
  .chapter::before {
    /* pointer triangle toward the rail */
    content: '';
    position: absolute;
    top: 1.25rem; left: -8px;
    width: 14px; height: 14px;
    background: #f4e4c1;
    border-left: 1.5px solid #8b6914;
    border-bottom: 1.5px solid #8b6914;
    transform: rotate(45deg);
  }
  .chapter-open {
    display: block; width: 100%; text-align: left;
    background: transparent; color: inherit; cursor: pointer;
    padding: 0.85rem 1.1rem 0.5rem;
  }
  .chapter-open[disabled] { cursor: default; }
  .chap-topbar {
    font-family: 'IM Fell English SC', serif;
    font-size: 0.7rem; letter-spacing: 0.15em; text-transform: uppercase;
    color: #8b6914;
    display: flex; align-items: center; gap: 0.4rem;
  }
  .date-full { display: inline-flex; align-items: center; gap: 0.3rem; }
  .chap-title {
    font-family: 'Cinzel', serif;
    font-weight: 800;
    font-size: 1.2rem;
    line-height: 1.2;
    color: #2c1810 !important;
    margin-top: 0.3rem;
    letter-spacing: 0.02em;
  }
  .chap-lede {
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
    line-height: 1.45;
    color: #3a2313;
    margin-top: 0.4rem;
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }
  .read-more {
    display: inline-flex; align-items: center; gap: 0.25rem;
    margin-top: 0.35rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.72rem;
    letter-spacing: 0.1em;
    color: #8b6914;
  }
  .chap-foot {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.45rem 1.1rem 0.6rem;
    border-top: 1px dashed rgba(139,105,20,0.35);
  }
  .vis {
    display: inline-flex; align-items: center; gap: 0.3rem;
    font-size: 0.6rem; letter-spacing: 0.12em; text-transform: uppercase;
    padding: 0.1rem 0.5rem; border-radius: 0.25rem;
    border: 1px solid;
    font-family: 'Cinzel', serif; font-weight: 700;
  }
  .chap-actions { display: inline-flex; gap: 0.15rem; }
  .icon-btn {
    padding: 0.3rem; border-radius: 0.3rem;
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
    width: min(56rem, 100%);
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
  .reader-mast .dot { opacity: 0.5; }
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

  @media (max-width: 640px) {
    .entry { grid-template-columns: 4.5rem 1fr; gap: 0.5rem; }
    .date-ribbon .day { font-size: 1.1rem; }
    .chapter::before { display: none; }
  }
</style>
