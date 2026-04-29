<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Stepper from '$lib/components/Stepper.svelte';
  import { Crown, UserMinus } from '@lucide/svelte';

  const cid = $derived(page.params.id!);

  type Member = { user_id: string; display_name: string; email: string; role: string; character_limit: number };
  let members = $state<Member[]>([]);
  let email = $state('');
  let role = $state<'player' | 'master'>('player');
  let error = $state('');
  let busy = $state(false);
  let campaignMasterId = $state<string | null>(null);

  async function load() {
    try {
      const camp = await Campaigns.get(cid);
      campaignMasterId = camp.master_id;
      members = await Campaigns.members(cid);
    } catch (e) { error = (e as Error).message; }
  }

  onMount(async () => {
    if (!auth.authenticated) { goto('/login'); return; }
    await load();
    // page is master-only; if not master, leave
    const camp = await Campaigns.get(cid);
    if (camp.master_id !== auth.user?.id) goto(`/campaigns/${cid}`);
  });

  async function add(close: () => void) {
    error = ''; busy = true;
    try {
      await Campaigns.addMember(cid, email, role);
      email = ''; role = 'player';
      close();
      await load();
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  async function remove(userId: string) {
    if (!confirm($_('members.remove_confirm'))) return;
    try { await Campaigns.removeMember(cid, userId); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function setLimit(userId: string, value: number) {
    try {
      await Campaigns.updateMember(cid, userId, { character_limit: value });
      await load();
    } catch (e) { error = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-3xl px-6 py-6">
  <div class="flex items-start justify-between gap-4">
    <div>
      <h2 class="text-xl font-semibold">{$_('members.title')}</h2>
      <p class="mt-1 text-sm text-neutral-400">{$_('members.explain')}</p>
    </div>
    <CollapsibleAdd label={`+ ${$_('members.add')}`} title={$_('members.add')} alignEnd={false}>
      {#snippet children({ close })}
        <form onsubmit={(e) => { e.preventDefault(); add(close); }} class="flex flex-wrap gap-2">
          <input type="email" required placeholder={$_('members.email_placeholder')} bind:value={email}
            class="flex-1 min-w-48 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
          <select bind:value={role} class="rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
            <option value="player">{$_('members.role_player')}</option>
            <option value="master">{$_('members.role_master')}</option>
          </select>
          <button disabled={busy} class="rounded-md bg-violet-600 px-6 py-2 text-white disabled:opacity-50">
            {busy ? '…' : $_('common.create')}
          </button>
        </form>
      {/snippet}
    </CollapsibleAdd>
  </div>
  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}

  <ul class="mt-6 space-y-2">
    {#each members as m (m.user_id)}
      <li class="flex items-center justify-between gap-3 rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-3">
        <div class="min-w-0">
          <div class="font-semibold">
            {m.display_name}
            {#if m.user_id === campaignMasterId}<Crown size={14} class="ml-1 inline text-amber-400" />{/if}
          </div>
          <div class="text-xs text-neutral-400">{m.email} · {m.role}</div>
        </div>
        {#if m.role === 'player'}
          <div class="shrink-0">
            <Stepper compact label={$_('members.char_limit')} value={m.character_limit} min={1} max={20}
              onchange={(v) => setLimit(m.user_id, v)} />
          </div>
        {/if}
        {#if m.user_id !== campaignMasterId}
          <button onclick={() => remove(m.user_id)} class="inline-flex items-center gap-1 text-sm text-red-400 hover:text-red-300">
            <UserMinus size={14} /> {$_('members.remove')}
          </button>
        {/if}
      </li>
    {/each}
    {#if members.length === 0}<li class="text-neutral-500 italic">{$_('members.empty')}</li>{/if}
  </ul>
</section>
