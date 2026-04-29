<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Invitations } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { Check, X, ArrowLeft } from '@lucide/svelte';

  type Inv = Awaited<ReturnType<typeof Invitations.mine>>[number];
  let items = $state<Inv[]>([]);
  let error = $state('');

  async function load() {
    try { items = await Invitations.mine(); } catch (e) { error = (e as Error).message; }
  }
  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
  });

  async function accept(id: string, cid: string) {
    try { await Invitations.accept(id); goto(`/campaigns/${cid}`); }
    catch (e) { error = (e as Error).message; }
  }
  async function decline(id: string) {
    try { await Invitations.decline(id); await load(); }
    catch (e) { error = (e as Error).message; }
  }
</script>

<header class="border-b border-amber-900/40 bg-[#2a1d10] px-6 py-3 flex items-center gap-4">
  <a href="/campaigns" class="text-neutral-400 hover:text-neutral-200"><ArrowLeft size={18} /></a>
  <span class="font-bold" style="color:#c9a84c;">{$_('invitations.title')}</span>
</header>

<section class="page-panel">
  <h2 class="text-xl font-semibold">{$_('invitations.title')}</h2>
  <p class="mt-1 text-sm">{$_('invitations.explain')}</p>
  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}

  <ul class="mt-6 space-y-2">
    {#each items as i (i.id)}
      <li class="flex items-start justify-between gap-3 rounded-lg border p-4"
        style="border-color:#d4b896;">
        <div class="min-w-0">
          <div class="font-semibold">{i.campaign_name}</div>
          <div class="text-sm">
            {$_('invitations.role')}: <span class="font-semibold">{i.role}</span>
            {#if i.inviter_name} · {$_('invitations.from')} {i.inviter_name}{/if}
          </div>
          {#if i.message}<p class="mt-1 text-sm italic">{i.message}</p>{/if}
        </div>
        <div class="flex gap-2 shrink-0">
          <button onclick={() => accept(i.id, i.campaign_id)}
            class="inline-flex items-center gap-1 rounded bg-emerald-600 px-3 py-1.5 text-sm text-white">
            <Check size={14} /> {$_('invitations.accept')}
          </button>
          <button onclick={() => decline(i.id)}
            class="inline-flex items-center gap-1 rounded bg-red-600 px-3 py-1.5 text-sm text-white">
            <X size={14} /> {$_('invitations.decline')}
          </button>
        </div>
      </li>
    {/each}
    {#if items.length === 0}<li class="italic" style="color:#8b6355;">{$_('invitations.empty')}</li>{/if}
  </ul>
</section>
