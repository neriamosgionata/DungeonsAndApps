<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { _ } from 'svelte-i18n';
  import { Parties, Loot, Quests } from '$lib/api/resources';
  import type { PartyData, LootItem, Quest } from '$lib/types';
  import Stepper from '$lib/components/Stepper.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import CoinPurse from '$lib/components/CoinPurse.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { Coins, Backpack, ScrollText, Trash2, StickyNote, ChevronRight } from '@lucide/svelte';

  const campaign = useCampaign();

  const cid = $derived(page.params.id!);
  let party = $state<PartyData | null>(null);
  let loot = $state<LootItem[]>([]);
  let quests = $state<Quest[]>([]);
  let error = $state('');

  let itemName = $state('');
  let itemQty = $state(1);
  let questTitle = $state('');
  let questDesc = $state('');

  async function load() {
    try {
      [party, loot, quests] = await Promise.all([Parties.get(cid), Loot.list(cid), Quests.list(cid)]);
    } catch (e) { error = (e as Error).message; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      if (t.startsWith('quest_') || t.startsWith('loot_') || t === 'party_updated') load();
    });
  });
  onDestroy(() => offWs?.());

  async function setCoin(k: string, v: number) { party = await Parties.update(cid, { [k]: v }); }
  async function setNotes(v: string) { party = await Parties.update(cid, { shared_notes: v }); }

  async function addLoot(close: () => void) {
    await Loot.create(cid, { name: itemName, quantity: itemQty });
    itemName = ''; itemQty = 1;
    close();
    loot = await Loot.list(cid);
  }

  async function addQuest(close: () => void) {
    await Quests.create(cid, { title: questTitle, description: questDesc || undefined, visibility: 'players' });
    questTitle = ''; questDesc = '';
    close();
    quests = await Quests.list(cid);
  }

  async function advanceQuest(q: Quest, status: Quest['status']) {
    await Quests.update(q.id as string, { status });
    quests = await Quests.list(cid);
  }

  async function removeQuest(id: string) {
    if (!confirm($_('group.delete_confirm'))) return;
    await Quests.delete(id);
    quests = await Quests.list(cid);
  }

  let openQuestIds = $state<Set<string>>(new Set());
  function toggleQuest(id: string) {
    const s = new Set(openQuestIds);
    if (s.has(id)) s.delete(id); else s.add(id);
    openQuestIds = s;
  }
</script>

<section class="mx-auto max-w-5xl px-6 py-6 space-y-8">
  <div>
    <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><Coins size={20} /> {$_('group.coin')}</h2>
    {#if party}
      <div class="mt-3">
        <CoinPurse
          values={{
            pp: (party.pp as number) ?? 0,
            gp: (party.gp as number) ?? 0,
            ep: (party.ep as number) ?? 0,
            sp: (party.sp as number) ?? 0,
            cp: (party.cp as number) ?? 0,
          }}
          onchange={(k, v) => setCoin(k, v)} />
      </div>
      <h3 class="mt-6 inline-flex items-center gap-2 text-xl font-semibold"><StickyNote size={20} /> {$_('group.notes')}</h3>
      <textarea value={(party.shared_notes as string | null) ?? ''} onchange={(e) => setNotes((e.currentTarget as HTMLTextAreaElement).value)}
        rows="3" class="mt-2 w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
    {/if}
    {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}
  </div>

  <div>
    <div class="flex items-center justify-between gap-4">
      <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><Backpack size={20} /> {$_('group.loot')}</h2>
      <CollapsibleAdd label={`+ ${$_('group.loot_add')}`} title={$_('group.loot_add')} alignEnd={false}>
        {#snippet children({ close })}
          <form onsubmit={(e) => { e.preventDefault(); addLoot(close); }} class="flex gap-2">
            <input required placeholder={$_('group.item_ph')} bind:value={itemName}
              class="flex-1 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <input type="number" min="1" bind:value={itemQty}
              class="w-20 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
            <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
          </form>
        {/snippet}
      </CollapsibleAdd>
    </div>
    <ul class="mt-4 space-y-1">
      {#each loot as l (l.id)}
        <li class="flex items-center gap-3 rounded border border-neutral-800 bg-neutral-900 px-3 py-2 text-sm">
          <div class="w-32">
            <Stepper compact value={(l.quantity as number) ?? 0} min={0}
              onchange={(v) => Loot.update(l.id as string, { quantity: v }).then(() => Loot.list(cid).then(x => loot = x))} />
          </div>
          <span class="flex-1">× {l.name}</span>
          <button aria-label="remove" class="text-red-400" onclick={() => Loot.delete(l.id as string).then(() => Loot.list(cid).then(x => loot = x))}>
            <Trash2 size={14} />
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div>
    <div class="flex items-center justify-between gap-4">
      <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><ScrollText size={20} /> {$_('group.quests')}</h2>
      {#if campaign().isMaster}
        <CollapsibleAdd label={`+ ${$_('group.quest_add')}`} title={$_('group.quest_add')} alignEnd={false}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); addQuest(close); }} class="space-y-2">
              <input required placeholder={$_('group.quest_title_ph')} bind:value={questTitle}
                class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <textarea rows="4" placeholder={`${$_('group.quest_desc_ph')} — # Title / blank line = new paragraph`} bind:value={questDesc}
                class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2"></textarea>
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('common.create')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    </div>
    <ul class="mt-4 space-y-2">
      {#each quests as q (q.id)}
        {@const qid = q.id as string}
        {@const isOpen = openQuestIds.has(qid)}
        {@const hasBody = !!(q.description as string | undefined)}
        <li class="rounded-lg border border-neutral-800 bg-neutral-900 overflow-hidden">
          <div class="flex items-center gap-2 px-3 py-2">
            <button type="button" onclick={() => hasBody && toggleQuest(qid)}
              class="flex-1 flex items-center gap-3 text-left {hasBody ? 'cursor-pointer' : 'cursor-default'}">
              {#if hasBody}
                <ChevronRight size={16} class="transition-transform {isOpen ? 'rotate-90' : ''}"
                  style="color:#8b6914;" />
              {:else}
                <span class="w-4"></span>
              {/if}
              <div class="flex-1 min-w-0">
                <div class="font-semibold truncate">{q.title}</div>
                <div class="text-xs" style="color:#8b6355;">{q.status}</div>
              </div>
            </button>
            {#if campaign().isMaster}
              <select value={q.status} onchange={(e) => advanceQuest(q, (e.currentTarget as HTMLSelectElement).value as Quest['status'])}
                class="rounded bg-neutral-800 border border-neutral-700 px-2 py-1 text-xs shrink-0">
                <option>active</option><option>completed</option><option>failed</option><option>abandoned</option>
              </select>
              <button type="button" title="Delete" onclick={() => removeQuest(qid)}
                class="rounded p-1 text-red-500 hover:bg-red-500/10 shrink-0">
                <Trash2 size={14} />
              </button>
            {/if}
          </div>
          {#if isOpen && hasBody}
            <div class="border-t px-4 py-3" style="border-color:rgba(139,105,20,0.25);">
              <Paragraphs text={q.description as string | undefined} emptyLabel="" />
            </div>
          {/if}
        </li>
      {/each}
      {#if quests.length === 0}
        <li class="italic px-4" style="color:#8b6355;">—</li>
      {/if}
    </ul>
  </div>
</section>
