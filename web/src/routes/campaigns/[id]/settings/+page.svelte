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
  let houseRules = $state('');
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
      archived = !!campaign.archived_at;
      houseRules = (campaign.settings?.house_rules as string | undefined) ?? '';
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
        settings: { house_rules: houseRules.trim() || undefined },
      });
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }

  async function exportCampaign() {
    error = '';
    try {
      const data = await Campaigns.exportCampaign(cid);
      const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `${campaign?.name ?? 'campaign'}.json`.replace(/[^a-z0-9.]+/gi, '-').toLowerCase();
      a.click();
      URL.revokeObjectURL(url);
    } catch (e) { error = (e as Error).message; }
  }

  async function importCampaign() {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'application/json';
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file) return;
      try {
        const parsed = JSON.parse(await file.text());
        if (!parsed || typeof parsed !== 'object' || !parsed.campaign) {
          alert($_('settings.import_invalid'));
          return;
        }
        if (!confirm($_('settings.import_confirm'))) return;
        const c = await Campaigns.importCampaign(parsed);
        goto(`/campaigns/${c.id}`);
        alert($_('settings.import_ok'));
      } catch (e) { error = (e as Error).message; }
    };
    input.click();
  }

  async function deleteCampaign() {
    if (!confirm($_('settings.delete_confirm'))) return;
    try {
      await Campaigns.delete(cid);
      goto('/campaigns');
    } catch (e) { error = (e as Error).message; }
  }

  let archived = $state(false);
  let archiving = $state(false);
  async function toggleArchive() {
    archiving = true; error = '';
    try {
      const c = archived ? await Campaigns.restore(cid) : await Campaigns.archive(cid);
      archived = !!c.archived_at;
    } catch (e) { error = (e as Error).message; }
    finally { archiving = false; }
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
      <div>
        <label for="settings-rules" class="block text-sm text-neutral-400 mb-1">{$_('settings.house_rules')}</label>
        <textarea id="settings-rules" bind:value={houseRules} rows="6"
          placeholder={$_('settings.house_rules_ph')}
          class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
      </div>
      <div class="text-sm text-neutral-500">
        {$_('settings.created_at')}: {new Date(campaign.created_at).toLocaleDateString()}
      </div>
      <div class="flex items-center gap-3 pt-2">
        <button onclick={save} disabled={busy} class="rounded-md bg-violet-600 px-6 py-2 text-white disabled:opacity-50">
          {busy ? '…' : $_('settings.save')}
        </button>
        <button onclick={toggleArchive} disabled={archiving} class="rounded-md bg-neutral-700 px-6 py-2 text-white disabled:opacity-50">
          {archiving ? '…' : (archived ? $_('settings.restore') : $_('settings.archive'))}
        </button>
        <button onclick={exportCampaign} class="rounded-md bg-neutral-700 px-6 py-2 text-white">
          {$_('settings.export')}
        </button>
        <button onclick={importCampaign} class="rounded-md bg-neutral-700 px-6 py-2 text-white">
          {$_('settings.import')}
        </button>
        <button onclick={deleteCampaign} class="rounded-md bg-red-700 px-6 py-2 text-white">
          {$_('settings.delete')}
        </button>
      </div>
    </div>
  {/if}
</section>
