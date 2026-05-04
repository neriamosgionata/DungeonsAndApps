<script lang="ts">
  import { page } from '$app/state';
  import { onDestroy, onMount } from 'svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { _ } from 'svelte-i18n';
  import { Parties, Loot, Quests, Campaigns, Characters, NPCs } from '$lib/api/resources';
  import type { PartyData, LootItem, Quest, Character, NPC } from '$lib/types';
  import { notifications } from '$lib/notifications.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import Stepper from '$lib/components/Stepper.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import CoinPurse from '$lib/components/CoinPurse.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { Coins, Backpack, ScrollText, Trash2, StickyNote, ChevronRight, Award, X } from '@lucide/svelte';

  const campaign = useCampaign();

  const cid = $derived(page.params.id!);
  let party = $state<PartyData | null>(null);
  let loot = $state<LootItem[]>([]);
  let quests = $state<Quest[]>([]);
  let characters = $state<Character[]>([]);
  let npcs = $state<NPC[]>([]);
  let error = $state('');
  let loading = $state(true);

  let lootQ = $state('');
  let questQ = $state('');
  const filteredLoot = $derived(loot.filter((l) => !lootQ.trim() || (l.name as string).toLowerCase().includes(lootQ.trim().toLowerCase())));
  const filteredQuests = $derived(quests.filter((q) => !questQ.trim() || (q.title as string).toLowerCase().includes(questQ.trim().toLowerCase()) || ((q.description as string | null) ?? '').toLowerCase().includes(questQ.trim().toLowerCase())));

  let itemName = $state('');
  let itemQty = $state(1);
  let questTitle = $state('');
  let questDesc = $state('');

  let selectedIds = $state<Set<string>>(new Set());
  let xpAmount = $state(0);
  let xpReason = $state('');
  let xpBusy = $state(false);

  let questNpcs = $state<Record<string, Array<{npc_id:string;name:string;role?:string|null}>>>({});
  let linkNpcId = $state<Record<string, string>>({});
  let linkNpcRole = $state<Record<string, string>>({});

  async function load() {
    try {
      [party, loot, quests, characters, npcs] = await Promise.all([
        Parties.get(cid), Loot.list(cid), Quests.list(cid), Characters.list(cid), NPCs.list(cid)
      ]);
    } catch (e) { error = (e as Error).message; }
    finally { loading = false; }
  }
  onMount(load);

  let offWs: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on(async (ev) => {
      const t = ev.type as string;
      if (t.startsWith('quest_') || t.startsWith('loot_') || t === 'party_updated') {
        await load();
        if (t.startsWith('quest_')) {
          for (const id of openQuestIds) {
            try {
              const q = await Quests.get(id) as Quest & { npcs: Array<{npc_id:string;name:string;role?:string|null}> };
              questNpcs[id] = q.npcs ?? [];
            } catch { /* ignore */ }
          }
        }
      }
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

  async function linkNpcToQuest(qid: string) {
    const npcId = linkNpcId[qid];
    if (!npcId) return;
    await Quests.linkNpc(qid, npcId, linkNpcRole[qid]?.trim() || undefined);
    linkNpcId[qid] = '';
    linkNpcRole[qid] = '';
    quests = await Quests.list(cid);
    const q = await Quests.get(qid) as Quest & { npcs: Array<{npc_id:string;name:string;role?:string|null}> };
    questNpcs[qid] = q.npcs ?? [];
  }

  async function unlinkNpcFromQuest(qid: string, npcId: string) {
    await Quests.unlinkNpc(qid, npcId);
    quests = await Quests.list(cid);
    const q = await Quests.get(qid) as Quest & { npcs: Array<{npc_id:string;name:string;role?:string|null}> };
    questNpcs[qid] = q.npcs ?? [];
  }

  let openQuestIds = $state<Set<string>>(new Set());
  async function toggleQuest(id: string) {
    const s = new Set(openQuestIds);
    if (s.has(id)) {
      s.delete(id);
    } else {
      s.add(id);
      if (!questNpcs[id]) {
        try {
          const q = await Quests.get(id) as Quest & { npcs: Array<{npc_id:string;name:string;role?:string|null}> };
          questNpcs[id] = q.npcs ?? [];
        } catch { /* ignore */ }
      }
    }
    openQuestIds = s;
  }

  function toggleCharacter(id: string) {
    const s = new Set(selectedIds);
    if (s.has(id)) s.delete(id); else s.add(id);
    selectedIds = s;
  }

  function selectAll() {
    if (selectedIds.size === characters.length) {
      selectedIds = new Set();
    } else {
      selectedIds = new Set(characters.map((c) => c.id));
    }
  }

  async function awardXp(close: () => void) {
    if (selectedIds.size === 0 || xpAmount <= 0) return;
    error = ''; xpBusy = true;
    try {
      const result = await Campaigns.awardXp(cid, {
        character_ids: Array.from(selectedIds),
        xp_each: xpAmount,
        reason: xpReason.trim() || undefined,
      });
      const leveled = result.characters_awarded.filter((c) => c.leveled_up);
      let body = '';
      if (leveled.length > 0) {
        body = leveled.map((c) =>
          $_('group.xp_leveled_up').replace('{{name}}', c.character_name).replace('{{level}}', String(c.new_level))
        ).join(' ');
      } else {
        body = $_('group.xp_success').replace('{{xp}}', String(xpAmount)).replace('{{count}}', String(selectedIds.size));
      }
      notifications.pushToast({
        id: `xp-${Date.now()}`,
        user_id: auth.user?.id ?? '',
        campaign_id: cid,
        kind: 'xp.awarded',
        title: $_('group.award_xp'),
        body,
        ref_kind: null,
        ref_id: null,
        read_at: new Date().toISOString(),
        created_at: new Date().toISOString(),
      });
      selectedIds = new Set();
      xpAmount = 0;
      xpReason = '';
      close();
    } catch (e) { error = (e as Error).message; } finally { xpBusy = false; }
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
    {#if loading}<p class="mt-2 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}
  </div>

  {#if campaign().isMaster}
    <div>
      <div class="flex items-center justify-between gap-4">
        <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><Award size={20} /> {$_('group.award_xp')}</h2>
        <CollapsibleAdd label={$_('group.award_xp')} title={$_('group.award_xp')} alignEnd={false}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); awardXp(close); }} class="space-y-3">
              <div class="flex items-center justify-between">
                <span class="text-sm text-neutral-400">{characters.length} {$_('character.title').toLowerCase()}</span>
                <button type="button" onclick={selectAll} class="text-sm text-amber-400 hover:text-amber-300">
                  {selectedIds.size === characters.length ? $_('common.none') : $_('group.xp_select_all')}
                </button>
              </div>
              <ul class="max-h-48 overflow-y-auto space-y-1 rounded-md border border-neutral-800 bg-neutral-900 p-2">
                {#each characters as c (c.id)}
                  <li class="flex items-center gap-2 px-2 py-1">
                    <input type="checkbox" checked={selectedIds.has(c.id)} onchange={() => toggleCharacter(c.id)}
                      class="accent-amber-600" />
                    <span class="text-sm">{c.name}</span>
                    <span class="ml-auto text-xs text-neutral-500">Lv {c.level_total}</span>
                  </li>
                {/each}
                {#if characters.length === 0}<li class="text-sm text-neutral-500 italic px-2">—</li>{/if}
              </ul>
              <div class="flex gap-2">
                <input type="number" min="1" required placeholder={$_('group.xp_amount')} bind:value={xpAmount}
                  class="w-32 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
                <input type="text" placeholder={$_('group.xp_reason')} bind:value={xpReason}
                  class="flex-1 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              </div>
              <div class="flex justify-end">
                <button disabled={xpBusy || selectedIds.size === 0 || xpAmount <= 0}
                  class="rounded-md bg-violet-600 px-6 py-2 text-white disabled:opacity-50">
                  {xpBusy ? '…' : $_('group.xp_submit')}
                </button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      </div>
    </div>
  {/if}

  <div>
    <div class="flex items-center justify-between gap-4">
      <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><Backpack size={20} /> {$_('group.loot')}</h2>
      <div class="flex items-center gap-2">
        <input bind:value={lootQ} placeholder={$_('group.loot_search_ph')} class="rounded-md border border-neutral-700 bg-neutral-900 px-3 py-1 text-sm" />
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
    </div>
    <ul class="mt-4 space-y-1">
      {#each filteredLoot as l (l.id)}
        <li class="flex items-center gap-3 rounded border border-neutral-800 bg-neutral-900 px-3 py-2 text-sm">
          <div class="w-32">
            <Stepper compact value={(l.quantity as number) ?? 0} min={0}
              onchange={(v) => Loot.update(l.id as string, { quantity: v }).then(() => Loot.list(cid).then(x => loot = x))} />
          </div>
          <span class="flex-1">× {l.name}</span>
          <button aria-label="remove" class="text-red-400" onclick={() => { if (confirm($_('group.loot_delete_confirm'))) Loot.delete(l.id as string).then(() => Loot.list(cid).then(x => loot = x)); }}>
            <Trash2 size={14} />
          </button>
        </li>
      {/each}
    </ul>
  </div>

  <div>
    <div class="flex items-center justify-between gap-4">
      <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><ScrollText size={20} /> {$_('group.quests')}</h2>
      <div class="flex items-center gap-2">
        <input bind:value={questQ} placeholder={$_('group.quest_search_ph')} class="rounded-md border border-neutral-700 bg-neutral-900 px-3 py-1 text-sm" />
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
    </div>
    <ul class="mt-4 space-y-2">
      {#each filteredQuests as q (q.id)}
        {@const qid = q.id as string}
        {@const isOpen = openQuestIds.has(qid)}
        {@const hasBody = !!(q.description as string | undefined)}
        {@const linkedNpcs = questNpcs[qid] ?? []}
        <li class="rounded-lg border border-neutral-800 bg-neutral-900 overflow-hidden">
          <div class="flex items-center gap-2 px-3 py-2">
            <button type="button" onclick={() => toggleQuest(qid)}
              class="flex-1 flex items-center gap-3 text-left cursor-pointer">
              <ChevronRight size={16} class="transition-transform {isOpen ? 'rotate-90' : ''}"
                style="color:#8b6914;" />
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
          {#if isOpen}
            <div class="border-t px-4 py-3" style="border-color:rgba(139,105,20,0.25);">
              {#if hasBody}
                <Paragraphs text={q.description as string | undefined} emptyLabel="" />
              {/if}
              {#if linkedNpcs.length > 0}
                <div class="flex flex-wrap gap-2 {hasBody ? 'mt-3' : ''}">
                  {#each linkedNpcs as npc (npc.npc_id)}
                    <span class="inline-flex items-center gap-1.5 rounded-full bg-neutral-800 border border-neutral-700 px-2.5 py-0.5 text-xs">
                      <span class="truncate">{npc.name}</span>
                      {#if npc.role}<span class="text-neutral-500">({npc.role})</span>{/if}
                      {#if campaign().isMaster}
                        <button type="button" title="Remove" class="ml-0.5 text-red-400 hover:text-red-300" onclick={() => unlinkNpcFromQuest(qid, npc.npc_id)}>
                          <X size={12} />
                        </button>
                      {/if}
                    </span>
                  {/each}
                </div>
              {/if}
              {#if campaign().isMaster && npcs.length > 0}
                <div class="flex items-center gap-2 {hasBody || linkedNpcs.length > 0 ? 'mt-3' : ''}">
                  <select
                    class="rounded bg-neutral-800 border border-neutral-700 px-2 py-1 text-xs shrink-0"
                    value={linkNpcId[qid] ?? ''}
                    onchange={(e) => linkNpcId[qid] = (e.currentTarget as HTMLSelectElement).value}>
                    <option value="">Add NPC…</option>
                    {#each npcs.filter(n => !linkedNpcs.some(ln => ln.npc_id === n.id)) as npc}
                      <option value={npc.id}>{npc.name}</option>
                    {/each}
                  </select>
                  <input
                    type="text"
                    placeholder="role"
                    value={linkNpcRole[qid] ?? ''}
                    oninput={(e) => linkNpcRole[qid] = (e.currentTarget as HTMLInputElement).value}
                    class="w-32 rounded bg-neutral-800 border border-neutral-700 px-2 py-1 text-xs" />
                  <button
                    type="button"
                    disabled={!linkNpcId[qid]}
                    class="rounded bg-violet-600 px-3 py-1 text-xs text-white disabled:opacity-50"
                    onclick={() => linkNpcToQuest(qid)}>
                    Add
                  </button>
                </div>
              {/if}
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
