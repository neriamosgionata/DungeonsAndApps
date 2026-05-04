<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Campaigns, Invitations } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import Stepper from '$lib/components/Stepper.svelte';
  import type { Invitation } from '$lib/types';
  import { Crown, UserMinus } from '@lucide/svelte';

  const cid = $derived(page.params.id!);

  type Member = { user_id: string; display_name: string; email: string; role: string; character_limit: number };
  let members = $state<Member[]>([]);
  let invitations = $state<Invitation[]>([]);
  let memberQ = $state('');
  const filteredMembers = $derived(members.filter((m) => !memberQ.trim() || m.display_name.toLowerCase().includes(memberQ.trim().toLowerCase()) || m.email.toLowerCase().includes(memberQ.trim().toLowerCase())));
  let email = $state('');
  let role = $state<'player' | 'master'>('player');
  let error = $state('');
  let loading = $state(true);
  let busy = $state(false);
  let campaignMasterId = $state<string | null>(null);

  async function load() {
    try {
      const camp = await Campaigns.get(cid);
      campaignMasterId = camp.master_id;
      members = await Campaigns.members(cid);
      invitations = await Invitations.forCampaign(cid);
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  onMount(async () => {
    if (!auth.authenticated) { goto('/login'); return; }
    await load();
    const me = members.find((m) => m.user_id === auth.user?.id);
    if (!me || me.role !== 'master') goto(`/campaigns/${cid}/character`);
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

  async function setRole(userId: string, value: 'player' | 'master') {
    try {
      await Campaigns.updateMember(cid, userId, { role: value });
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  async function revokeInvitation(id: string) {
    if (!confirm($_('members.revoke_confirm'))) return;
    try { await Invitations.revoke(id); await load(); }
    catch (e) { error = (e as Error).message; }
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
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  <div class="mt-4">
    <input bind:value={memberQ} placeholder={$_('members.search_ph')} class="w-full rounded-md border border-neutral-700 bg-neutral-900 px-3 py-2 text-sm" />
  </div>

  <ul class="mt-4 space-y-2">
    {#each filteredMembers as m (m.user_id)}
      <li class="flex items-center justify-between gap-3 rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-3">
        <div class="min-w-0">
          <div class="font-semibold">
            {m.display_name}
            {#if m.user_id === campaignMasterId}<Crown size={14} class="ml-1 inline text-amber-400" />{/if}
          </div>
          <div class="text-xs text-neutral-400">{m.email}</div>
        </div>
        <div class="flex items-center gap-3 shrink-0">
          {#if m.user_id !== campaignMasterId}
            <select value={m.role} onchange={(e) => setRole(m.user_id, (e.currentTarget as HTMLSelectElement).value as 'player' | 'master')}
              class="rounded-md bg-neutral-800 border border-neutral-700 px-2 py-1 text-xs">
              <option value="player">{$_('members.role_player')}</option>
              <option value="master">{$_('members.role_master')}</option>
            </select>
          {:else}
            <span class="text-xs text-neutral-500">{$_('members.role_master')}</span>
          {/if}
          {#if m.role === 'player'}
            <Stepper compact label={$_('members.char_limit')} value={m.character_limit} min={1} max={20}
              onchange={(v) => setLimit(m.user_id, v)} />
          {/if}
          {#if m.user_id !== campaignMasterId}
            <button onclick={() => remove(m.user_id)} class="inline-flex items-center gap-1 text-sm text-red-400 hover:text-red-300">
              <UserMinus size={14} /> {$_('members.remove')}
            </button>
          {/if}
        </div>
      </li>
    {/each}
    {#if members.length === 0}<li class="text-neutral-500 italic">{$_('members.empty')}</li>{/if}
  </ul>

  {#if invitations.length > 0}
    <div class="mt-8">
      <h3 class="text-lg font-semibold">{$_('members.invitations')}</h3>
      <ul class="mt-3 space-y-2">
        {#each invitations as inv (inv.id)}
          <li class="flex items-center justify-between gap-3 rounded-lg border border-neutral-800 bg-neutral-900 px-4 py-3">
            <div class="min-w-0">
              <div class="font-semibold text-sm">{inv.email ?? inv.user_id}</div>
              <div class="text-xs text-neutral-400">
                {$_('members.invite_role')}: {inv.role} · {new Date(inv.created_at).toLocaleDateString()}
              </div>
            </div>
            <button onclick={() => revokeInvitation(inv.id)} class="text-sm text-red-400 hover:text-red-300 shrink-0">
              {$_('members.revoke')}
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}
</section>
