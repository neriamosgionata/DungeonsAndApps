<script lang="ts" generics="T extends Record<string, unknown>">
  import { onDestroy, onMount, type Snippet } from 'svelte';
  import { _ } from 'svelte-i18n';
  import CollapsibleAdd from './CollapsibleAdd.svelte';
  import ImageUpload from './ImageUpload.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { Eye, EyeOff, Trash2, ChevronRight, Pencil, X } from '@lucide/svelte';
  const campaign = useCampaign();

  type Fields = { key: keyof T; label: string; type?: 'text' | 'number' | 'textarea' | 'select' | 'image'; options?: string[]; kind?: string }[];

  type Resource = {
    list: (cid: string) => Promise<T[]>;
    create: (cid: string, body: Record<string, unknown>) => Promise<T>;
    update: (id: string, patch: Record<string, unknown>) => Promise<T>;
    delete: (id: string) => Promise<void>;
  };

  let {
    cid,
    title,
    resource,
    fields,
    renderItem,
    renderHeader,
    wsPrefix,
  }: {
    cid: string;
    title: string;
    resource: Resource;
    fields: Fields;
    /** Full body content — shown when row expanded. */
    renderItem: Snippet<[T]>;
    /** Compact header (always visible). Falls back to renderItem when omitted. */
    renderHeader?: Snippet<[T]>;
    /** WS event name prefix (e.g. "faction_", "lore_", "quest_") to auto-reload on. */
    wsPrefix?: string;
  } = $props();

  let openIds = $state<Set<string>>(new Set());
  function toggle(id: string) {
    const s = new Set(openIds);
    if (s.has(id)) s.delete(id); else s.add(id);
    openIds = s;
  }

  let items = $state<T[]>([]);
  let error = $state('');
  let form = $state<Record<string, unknown>>({ visibility: 'master' });

  async function load() {
    try { items = await resource.list(cid); } catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    if (!wsPrefix) return;
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t.startsWith(wsPrefix)) load();
    });
  });
  onDestroy(() => offWs?.());

  async function create(close: () => void) {
    try {
      await resource.create(cid, form);
      form = { visibility: 'master' };
      close();
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function remove(id: string) { await resource.delete(id); await load(); }

  async function toggleVis(item: T) {
    const next = item.visibility === 'players' ? 'master' : 'players';
    await resource.update(item.id as string, { visibility: next });
    await load();
  }

  let edit = $state<Record<string, unknown> | null>(null);
  function startEdit(item: T) {
    edit = { ...item };
  }
  async function saveEdit() {
    if (!edit) return;
    try {
      const patch: Record<string, unknown> = {};
      for (const f of fields) patch[f.key as string] = edit[f.key as string] ?? null;
      await resource.update(edit.id as string, patch);
      edit = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-4xl px-6 py-6">
  <div class="flex items-center justify-between gap-4">
    <h2 class="text-xl font-semibold">{title}</h2>
    {#if campaign().isMaster}
      <CollapsibleAdd label={`+ ${$_('common.add')}`} title={title} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="grid gap-2 sm:grid-cols-2">
            {#each fields as f (f.key)}
              {#if f.type === 'textarea'}
                <textarea placeholder={f.label} bind:value={form[f.key as string]}
                  class="sm:col-span-2 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" rows="3"></textarea>
              {:else if f.type === 'select'}
                <select bind:value={form[f.key as string]}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
                  {#each f.options ?? [] as o (o)}<option value={o}>{o}</option>{/each}
                </select>
              {:else if f.type === 'image'}
                <div class="sm:col-span-2">
                  <ImageUpload
                    value={(form[f.key as string] as string | null) ?? null}
                    kind={f.kind ?? 'misc'} label={f.label}
                    onchange={(url) => form[f.key as string] = url} />
                </div>
              {:else}
                <input type={f.type ?? 'text'} placeholder={f.label} bind:value={form[f.key as string]}
                  class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              {/if}
            {/each}
            <div class="sm:col-span-2 flex justify-end">
              <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
            </div>
          </form>
        {/snippet}
      </CollapsibleAdd>
    {/if}
  </div>
  {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}

  <ul class="mt-6 space-y-2">
    {#each items as it (it.id)}
      {@const isOpen = openIds.has(it.id as string)}
      <li class="rounded-lg border border-neutral-800 bg-neutral-900 overflow-hidden">
        <div class="flex items-center gap-2 px-3 py-2">
          <button type="button" onclick={() => toggle(it.id as string)}
            class="flex-1 flex items-center gap-3 text-left">
            <ChevronRight size={16} class="transition-transform {isOpen ? 'rotate-90' : ''}"
              style="color:#8b6914;" />
            <div class="flex-1 min-w-0">
              {#if renderHeader}
                {@render renderHeader(it)}
              {:else}
                {@render renderItem(it)}
              {/if}
            </div>
          </button>
          {#if campaign().isMaster}
            <div class="flex gap-2 items-center text-sm shrink-0">
              <button onclick={() => toggleVis(it)}
                class="inline-flex items-center gap-1.5 rounded bg-neutral-800 px-2.5 py-1.5 {it.visibility === 'master' ? 'text-neutral-400' : 'text-emerald-400'}">
                {#if it.visibility === 'master'}<EyeOff size={16} />{:else}<Eye size={16} />{/if}
                {it.visibility}
              </button>
              <button title="Edit" class="inline-flex items-center gap-1.5 px-2 py-1.5" style="color:#8b6914;"
                onclick={() => startEdit(it)}>
                <Pencil size={16} />
              </button>
              <button title="Delete" class="inline-flex items-center gap-1.5 text-red-400 px-2 py-1.5" onclick={() => remove(it.id as string)}>
                <Trash2 size={16} />
              </button>
            </div>
          {/if}
        </div>
        {#if isOpen && renderHeader}
          <div class="border-t px-4 py-3" style="border-color:rgba(139,105,20,0.25);">
            {@render renderItem(it)}
          </div>
        {/if}
      </li>
    {/each}
  </ul>
</section>

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
        <h3 class="font-display text-lg">Edit {title}</h3>
        <button onclick={() => edit = null} aria-label="close"><X size={18} /></button>
      </div>
      <div class="grid gap-2 sm:grid-cols-2">
        {#each fields as f (f.key)}
          {#if f.type === 'textarea'}
            <textarea placeholder={f.label} bind:value={edit[f.key as string]}
              class="sm:col-span-2 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" rows="6"></textarea>
          {:else if f.type === 'select'}
            <select bind:value={edit[f.key as string]}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
              {#each f.options ?? [] as o (o)}<option value={o}>{o}</option>{/each}
            </select>
          {:else if f.type === 'image'}
            <div class="sm:col-span-2">
              <ImageUpload
                value={(edit[f.key as string] as string | null) ?? null}
                kind={f.kind ?? 'misc'} label={f.label}
                onchange={(url) => edit && (edit[f.key as string] = url)} />
            </div>
          {:else}
            <input type={f.type ?? 'text'} placeholder={f.label} bind:value={edit[f.key as string]}
              class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
          {/if}
        {/each}
      </div>
      <div class="flex justify-end gap-2">
        <button onclick={() => (edit = null)}
          class="rounded-md border border-neutral-700 px-4 py-2 text-sm">Cancel</button>
        <button onclick={saveEdit}
          class="rounded-md bg-violet-600 px-6 py-2 text-sm text-white">Save</button>
      </div>
    </div>
  </div>
{/if}
