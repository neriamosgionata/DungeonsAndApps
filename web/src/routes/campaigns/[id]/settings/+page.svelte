<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { page } from '$app/state';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import type { Campaign } from '$lib/types';

  const cid = $derived(page.params.id!);
  let campaign = $state<Campaign | null>(null);
  let name = $state('');
  let description = $state('');
  let iconUrl = $state<string | null>(null);
  let error = $state('');
  let loading = $state(true);
  let busy = $state(false);

  async function load() {
    try {
      campaign = await Campaigns.get(cid);
      const members = await Campaigns.members(cid);
      const me = members.find((m) => m.user_id === auth.user?.id);
      const isMaster = campaign.master_id === auth.user?.id || auth.isAdmin || me?.role === 'master';
      if (!isMaster) {
        goto(`/campaigns/${cid}/character`, { replaceState: true });
        return;
      }
      name = campaign.name;
      description = campaign.description ?? '';
      iconUrl = campaign.icon_url ?? null;
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    load();
  });

  async function save() {
    error = ''; busy = true;
    try {
      campaign = await Campaigns.update(cid, {
        name: name.trim(),
        description: description.trim() || null,
        icon_url: iconUrl,
      });
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  async function deleteCampaign() {
    if (!confirm($_('settings.delete_confirm'))) return;
    try {
      await Campaigns.delete(cid);
      goto('/campaigns');
    } catch (e) { error = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-3xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold">{$_('settings.title')}</h2>
  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  {#if campaign}
    <div class="mt-6 space-y-4">
      <div>
        <label for="settings-name" class="block text-sm text-neutral-400 mb-1">{$_('common.name')}</label>
        <input id="settings-name" bind:value={name} class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
      </div>
      <div>
        <label for="settings-desc" class="block text-sm text-neutral-400 mb-1">{$_('common.description')}</label>
        <textarea id="settings-desc" bind:value={description} rows="3" class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      </div>
      <div>
        <span class="block text-sm text-neutral-400 mb-1">{$_('settings.icon')}</span>
        <ImageUpload bind:value={iconUrl} kind="campaign" size={96} />
      </div>
      <div class="text-sm text-neutral-500">
        {$_('settings.created_at')}: {new Date(campaign.created_at).toLocaleDateString()}
      </div>
      <div class="flex items-center gap-3 pt-2">
        <button onclick={save} disabled={busy} class="rounded-md bg-violet-600 px-6 py-2 text-white disabled:opacity-50">
          {busy ? '…' : $_('settings.save')}
        </button>
        <button onclick={deleteCampaign} class="rounded-md bg-red-700 px-6 py-2 text-white">
          {$_('settings.delete')}
        </button>
      </div>
    </div>
  {/if}
</section>
