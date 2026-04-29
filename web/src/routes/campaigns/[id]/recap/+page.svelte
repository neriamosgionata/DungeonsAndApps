<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Sessions } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { ScrollText, ChevronRight, Eye, EyeOff, Pencil, Trash2, X } from '@lucide/svelte';

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

  // edit modal state
  let edit = $state<Session | null>(null);

  async function load() {
    try { list = (await Sessions.list(cid)) as unknown as Session[]; }
    catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t.startsWith('session_')) load();
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
    if (!confirm(`Delete session "${s.title}"?`)) return;
    try { await Sessions.delete(s.id); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function toggleVis(s: Session) {
    const next = s.visibility === 'private' ? 'players' : s.visibility === 'players' ? 'public' : 'private';
    try { await Sessions.update(s.id, { visibility: next }); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  let openIds = $state<Set<string>>(new Set());
  function toggle(id: string) {
    const n = new Set(openIds);
    if (n.has(id)) n.delete(id); else n.add(id);
    openIds = n;
  }
  function fmtDate(d?: string | null): string {
    if (!d) return '';
    try { return new Date(d).toLocaleDateString(); } catch { return ''; }
  }
</script>

<section class="mx-auto max-w-4xl px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><ScrollText size={20} /> {$_('recap.title')}</h2>
    {#if campaign().isMaster}
      <CollapsibleAdd label={$_('recap.new')} title={$_('recap.new')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="space-y-2">
            <div class="flex gap-2 flex-wrap">
              <input required placeholder={$_('recap.title_ph')} bind:value={title}
                class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <input type="number" placeholder="#" bind:value={num}
                class="w-20 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <input type="date" bind:value={playedAt}
                class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <select bind:value={visibility} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
                <option value="private">private</option><option value="players">players</option><option value="public">public</option>
              </select>
            </div>
            <textarea rows="6" placeholder={$_('recap.body_ph')} bind:value={recap}
              class="block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
            <div class="flex justify-end">
              <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>
  {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}

  <ul class="mt-6 space-y-2">
    {#each list as s (s.id)}
      {@const isOpen = openIds.has(s.id)}
      {@const hasBody = !!s.recap || !!s.played_at}
      <li class="rounded-lg border border-neutral-800 bg-neutral-900 overflow-hidden">
        <div class="flex items-center gap-2 px-4 py-3">
          <button type="button" onclick={() => hasBody && toggle(s.id)}
            class="flex-1 flex items-center gap-3 text-left {hasBody ? 'cursor-pointer' : 'cursor-default'}">
            {#if hasBody}
              <ChevronRight size={16} class="transition-transform {isOpen ? 'rotate-90' : ''}"
                style="color:#8b6914;" />
            {:else}
              <span class="w-4"></span>
            {/if}
            <div class="flex-1 min-w-0">
              <h3 class="font-semibold truncate">
                {s.session_number ? `#${s.session_number} · ` : ''}{s.title}
              </h3>
              {#if s.played_at}
                <div class="text-xs" style="color:#8b6355;">{fmtDate(s.played_at)}</div>
              {/if}
            </div>
          </button>

          <span class="inline-flex items-center gap-1 text-xs shrink-0" style="color:#8b6355;">
            {#if s.visibility === 'private'}<EyeOff size={12} />{:else}<Eye size={12} />{/if}
            {s.visibility}
          </span>

          {#if campaign().isMaster}
            <div class="flex items-center gap-1 shrink-0">
              <button type="button" onclick={() => toggleVis(s)} title="Toggle visibility"
                class="rounded p-1 hover:bg-neutral-800/40"><Eye size={14} /></button>
              <button type="button" onclick={() => edit = { ...s }} title="Edit"
                class="rounded p-1 hover:bg-neutral-800/40"><Pencil size={14} /></button>
              <button type="button" onclick={() => remove(s)} title="Delete"
                class="rounded p-1 text-red-500 hover:bg-red-500/10"><Trash2 size={14} /></button>
            </div>
          {/if}
        </div>
        {#if isOpen && hasBody}
          <div class="border-t px-4 py-3" style="border-color:rgba(139,105,20,0.25);">
            <Paragraphs text={s.recap} emptyLabel={$_('recap.no_body')} />
          </div>
        {/if}
      </li>
    {/each}
    {#if list.length === 0}
      <li class="italic px-4" style="color:#8b6355;">{$_('recap.empty')}</li>
    {/if}
  </ul>
</section>

{#if edit}
  <div class="fixed inset-0 z-40 bg-black/70 flex items-center justify-center p-4"
    role="presentation"
    onclick={() => (edit = null)}
    onkeydown={(e) => e.key === 'Escape' && (edit = null)}>
    <div class="w-full max-w-2xl rounded-lg border p-5 max-h-[90vh] overflow-y-auto space-y-3"
      role="dialog" tabindex="-1"
      style="border-color:#8b6914; background:#241810;"
      onclick={(e) => e.stopPropagation()} onkeydown={(e) => e.stopPropagation()}>
      <div class="flex items-center justify-between">
        <h3 class="font-display text-lg">Edit session</h3>
        <button onclick={() => edit = null} aria-label="close"><X size={18} /></button>
      </div>
      <div class="flex gap-2 flex-wrap">
        <input required placeholder="Title" bind:value={edit.title}
          class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input type="number" placeholder="#" bind:value={edit.session_number}
          class="w-20 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <input type="date" value={edit.played_at ? edit.played_at.slice(0,10) : ''}
          onchange={(e) => edit && (edit.played_at = (e.currentTarget as HTMLInputElement).value || null)}
          class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
        <select bind:value={edit.visibility} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
          <option value="private">private</option><option value="players">players</option><option value="public">public</option>
        </select>
      </div>
      <textarea rows="10" placeholder="Recap (use blank lines between paragraphs, # Title for sections)"
        bind:value={edit.recap}
        class="block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      <div class="flex justify-end gap-2">
        <button onclick={() => (edit = null)}
          class="rounded-md border border-neutral-700 px-4 py-2 text-sm">Cancel</button>
        <button onclick={saveEdit} class="rounded-md bg-violet-600 px-6 py-2 text-sm text-white">Save</button>
      </div>
    </div>
  </div>
{/if}
