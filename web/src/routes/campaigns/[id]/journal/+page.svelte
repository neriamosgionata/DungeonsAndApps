<script lang="ts">
  import { onMount } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Journal } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { BookOpen, Pencil, Trash2, Plus, X } from '@lucide/svelte';

  const cid = $derived(page.params.id!);

  let entries = $state<Array<{ id: string; title: string; body: string; updated_at: string }>>([]);
  let loading = $state(true);
  let error = $state('');
  let editing = $state<{ id: string; title: string; body: string } | null>(null);
  let newTitle = $state('');
  let newBody = $state('');
  let saving = $state(false);

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
  });

  async function load() {
    try {
      entries = await Journal.list(cid);
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  async function saveNew() {
    if (!newTitle.trim()) return;
    saving = true; error = '';
    try {
      await Journal.create(cid, newTitle.trim(), newBody);
      newTitle = ''; newBody = '';
      await load();
    } catch (e) { error = (e as Error).message; } finally { saving = false; }
  }

  async function saveEdit() {
    if (!editing) return;
    saving = true; error = '';
    try {
      await Journal.update(editing.id, { title: editing.title.trim(), body: editing.body });
      editing = null;
      await load();
    } catch (e) { error = (e as Error).message; } finally { saving = false; }
  }

  async function remove(id: string) {
    if (!confirm($_('journal.delete_confirm'))) return;
    try { await Journal.delete(id); await load(); }
    catch (e) { error = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-3xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold flex items-center gap-2">
    <BookOpen size={20} style="color:#c9a84c;" />
    {$_('journal.title')}
  </h2>
  <p class="mt-1 text-sm" style="color:#8b6355;">{$_('journal.subtitle')}</p>

  {#if error}<p class="mt-3 text-sm" style="color:#8b1a1a;">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  <div class="mt-5 rounded-lg border p-4" style="border-color:rgba(139,105,20,0.5);background:#2c1810;">
    <h3 class="text-sm font-semibold flex items-center gap-1" style="color:#c9a84c;"><Plus size={13} /> {$_('journal.new')}</h3>
    <input bind:value={newTitle} placeholder={$_('journal.title_ph')}
      class="mt-2 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
    <textarea bind:value={newBody} rows="5" placeholder={$_('journal.body_ph')}
      class="mt-2 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
    <button onclick={saveNew} disabled={saving || !newTitle.trim()}
      class="mt-2 rounded px-4 py-1.5 text-sm" style="background:#8b6914;color:#f4e4c1;">
      {saving ? '…' : $_('common.save')}
    </button>
  </div>

  <div class="mt-5 space-y-3">
    {#each entries as e (e.id)}
      {#if editing?.id === e.id}
        <div class="rounded-lg border p-4" style="border-color:rgba(139,105,20,0.5);background:#2c1810;">
          <input bind:value={editing.title} placeholder={$_('journal.title_ph')}
            class="w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm" />
          <textarea bind:value={editing.body} rows="6"
            class="mt-2 w-full rounded bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm"></textarea>
          <div class="mt-2 flex gap-2">
            <button onclick={saveEdit} disabled={saving} class="rounded px-3 py-1 text-sm" style="background:#8b6914;color:#f4e4c1;">
              {$_('common.save')}
            </button>
            <button onclick={() => editing = null} class="rounded px-3 py-1 text-sm" style="background:#3a2313;color:#f4e4c1;">
              <X size={12} /> {$_('common.cancel')}
            </button>
          </div>
        </div>
      {:else}
        <article class="rounded-lg border p-4" style="border-color:rgba(139,105,20,0.4);background:#3a2313;">
          <div class="flex items-start justify-between gap-2">
            <h3 class="font-semibold" style="color:#f4e4c1;">{e.title}</h3>
            <div class="flex gap-1 shrink-0">
              <button onclick={() => editing = { id: e.id, title: e.title, body: e.body }} class="icon-btn" style="color:#a6855c;" title={$_('journal.edit')}>
                <Pencil size={13} />
              </button>
              <button onclick={() => remove(e.id)} class="icon-btn" style="color:#8b1a1a;" title={$_('journal.delete')}>
                <Trash2 size={13} />
              </button>
            </div>
          </div>
          {#if e.body}
            <p class="mt-2 whitespace-pre-wrap text-sm" style="color:#c2a178;">{e.body}</p>
          {/if}
          <p class="mt-2 text-[10px] uppercase tracking-widest" style="color:#8b6355;">
            {new Date(e.updated_at).toLocaleString()}
          </p>
        </article>
      {/if}
    {:else}
      <p class="italic text-sm" style="color:#8b6355;">{$_('journal.empty')}</p>
    {/each}
  </div>
</section>
