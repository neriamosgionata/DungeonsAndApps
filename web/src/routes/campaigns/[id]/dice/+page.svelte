<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { Dice } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { _ } from 'svelte-i18n';
  const campaign = useCampaign();
  import { Dices, Trash2, Lock, History, Eraser } from '@lucide/svelte';

  const cid = $derived(page.params.id!);
  let history = $state<Record<string, unknown>[]>([]);
  let expression = $state('1d20');
  let label = $state('');
  let isPrivate = $state(false);
  import type { DiceRollResult } from '$lib/types';
  let last = $state<DiceRollResult | null>(null);
  let error = $state('');

  async function load() { history = await Dice.history(cid, 30); }
  onMount(load);

  let off: (() => void) | undefined;
  onMount(() => {
    off = campaignSocket.on((ev) => {
      if (ev.type === 'dice_cleared') { load(); return; }
      if (ev.type !== 'dice_roll') return;
      // masters see everything; players only their own rolls
      if (campaign().isMaster || ev.user_id === auth.user?.id) load();
    });
  });
  onDestroy(() => off?.());

  async function clearHistory() {
    if (!confirm($_('dice.clear_confirm'))) return;
    try {
      await Dice.clear(cid);
      last = null;
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  const presets = ['1d20','1d20+5','2d20kh1','2d20kl1','1d4','1d6','1d8','1d10','1d12','8d6','4d6dl1'];

  async function roll(expr: string) {
    try {
      last = await Dice.roll(cid, expr, label || undefined, isPrivate);
      await load();
    } catch (e) { error = (e as Error).message; }
  }
</script>

<section class="mx-auto max-w-4xl px-6 py-6">
  <h2 class="inline-flex items-center gap-2 text-xl font-semibold"><Dices size={20} /> Dice</h2>

  <div class="mt-4 flex flex-wrap gap-2">
    {#each presets as p (p)}
      <button onclick={() => roll(p)}
        class="rounded-md bg-neutral-800 px-3 py-1 text-sm hover:bg-neutral-700"
        style="color:#f4e4c1;">{p}</button>
    {/each}
  </div>

  <form onsubmit={(e) => { e.preventDefault(); roll(expression); }} class="mt-4 flex gap-2 flex-wrap">
    <input required bind:value={expression} class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    <input placeholder="Label (optional)" bind:value={label}
      class="flex-1 min-w-40 rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    <label class="flex items-center gap-2 text-sm">
      <input type="checkbox" bind:checked={isPrivate} /> Private
    </label>
    <button class="rounded-md bg-violet-600 px-4 py-2 text-white">Roll</button>
  </form>
  {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}

  {#if last}
    <div class="mt-5 rounded-lg border border-violet-500/50 bg-violet-950/30 p-4">
      <div class="text-3xl font-bold text-violet-300">{last.total}</div>
      <div class="text-sm text-neutral-400">{last.expression}{last.label ? ` · ${last.label}` : ''}</div>
    </div>
  {/if}

  <div class="mt-8 flex items-center justify-between">
    <h3 class="inline-flex items-center gap-2 text-lg font-semibold"><History size={18} /> {$_('dice.history')}</h3>
    {#if campaign().isMaster && history.length > 0}
      <button onclick={clearHistory}
        class="inline-flex items-center gap-1 rounded bg-red-600 px-3 py-1 text-sm text-white">
        <Eraser size={14} /> {$_('dice.clear')}
      </button>
    {/if}
  </div>
  <ul class="mt-3 space-y-1 text-sm">
    {#each history as h (h.id)}
      <li class="flex justify-between items-center rounded border border-neutral-800 bg-neutral-900 px-3 py-2">
        <span class="inline-flex items-center gap-1.5">
          {h.expression}{h.label ? ` · ${h.label}` : ''}
          {#if h.private}<Lock size={12} class="text-neutral-500" />{/if}
        </span>
        <span class="font-bold text-violet-300">{h.total}</span>
      </li>
    {/each}
  </ul>
</section>
