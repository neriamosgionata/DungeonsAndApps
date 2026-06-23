<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Dice } from '$lib/api/resources';
  import { Dices } from '@lucide/svelte';
  import type { DiceRollResult, DiceHistory } from '$lib/types';

  let {
    cid,
    sharedHistory = [],
  }: {
    cid: string;
    sharedHistory?: Array<DiceRollResult | DiceHistory>;
  } = $props();

  let panelOpen = $state(false);
  let diceCount = $state(1);
  let diceExpr = $state('');
  let diceLabel = $state('');
  let history = $state<Array<DiceRollResult | DiceHistory>>([]);
  let historyOpen = $state(false);
  let error = $state('');

  // F7: merge local history (own rolls) with shared history (other players'
  // rolls streamed via WS) and dedupe by id. Newest first, capped at 30.
  const mergedHistory = $derived.by(() => {
    const seen = new Set<string>();
    const out: Array<DiceRollResult | DiceHistory> = [];
    for (const r of [...history, ...sharedHistory]) {
      if (seen.has(r.id)) continue;
      seen.add(r.id);
      out.push(r);
      if (out.length >= 30) break;
    }
    return out;
  });

  const DICE_TYPES = [
    { f: 4, n: 'd4' },
    { f: 6, n: 'd6' },
    { f: 8, n: 'd8' },
    { f: 10, n: 'd10' },
    { f: 12, n: 'd12' },
    { f: 20, n: 'd20' },
    { f: 100, n: 'd%' },
  ];
  const COUNTS = [1, 2, 3, 4, 5, 6, 8, 10];

  async function roll(expression: string, label?: string) {
    error = '';
    try {
      const res = await Dice.roll(cid, expression, label);
      history = [res, ...history].slice(0, 20);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function loadHistory() {
    try {
      history = await Dice.history(cid, 20);
    } catch {
      history = [];
    }
  }
</script>

<div class="fixed bottom-4 right-4 z-50 flex flex-col items-end gap-2">
  {#if panelOpen}
    <div class="dice-panel">
      <div class="dice-panel-head">
        <span class="font-display font-bold text-sm" style="color:#c9a84c;">
          {$_('initiative.title_dice_roller')}
        </span>
        <button type="button" class="text-xs" style="color:#8b6355;" onclick={() => (panelOpen = false)}>
          ✕
        </button>
      </div>
      <div class="dice-count-row">
        {#each COUNTS as n (n)}
          <button
            type="button"
            class="dice-count-btn {diceCount === n ? 'active' : ''}"
            onclick={() => (diceCount = n)}>{n}</button>
        {/each}
      </div>
      <div class="dice-quick">
        {#each DICE_TYPES as d}
          <button
            type="button"
            class="dice-die"
            onclick={() => roll(`${diceCount}d${d.f}`, `${diceCount}${d.n}`)}>
            {#if diceCount > 1}<span style="font-size:0.6rem;opacity:0.7;">{diceCount}×</span>{/if}{d.n}
          </button>
        {/each}
      </div>
      <div class="dice-custom">
        <input type="text" bind:value={diceExpr} placeholder="2d6+3" style="min-width:0;flex:1;" />
        <button
          type="button"
          class="ca-btn"
          onclick={() => {
            if (diceExpr) roll(diceExpr, diceLabel || undefined);
            diceExpr = '';
          }}>
          {$_('initiative.label_dice_roll')}
        </button>
      </div>
      <input
        type="text"
        bind:value={diceLabel}
        placeholder={$_('initiative.ph_dice_label')}
        class="dice-label-input" />
      {#if error}
        <p class="text-[10px] mt-1" style="color:#b84040;">{error}</p>
      {/if}
      <button
        type="button"
        class="text-[10px] mt-1"
        style="color:#a6855c;"
        onclick={() => {
          historyOpen = !historyOpen;
          if (historyOpen) loadHistory();
        }}>
        {historyOpen ? $_('initiative.label_hide_history') : $_('initiative.label_show_history')}
      </button>
      {#if historyOpen}
        <div class="dice-history">
          {#each mergedHistory as h (h.id)}
            <div class="dice-history-row">
              <span class="font-display text-[10px]" style="color:#a6855c;">{h.expression}</span>
              <span class="font-bold text-sm" style="color:#c9a84c;">{h.total}</span>
              {#if h.label}<span class="text-[10px]" style="color:#8b6355;">({h.label})</span>{/if}
            </div>
          {/each}
          {#if mergedHistory.length === 0}
            <span class="text-[10px] italic" style="color:#8b6355;">{$_('initiative.label_no_rolls')}</span>
          {/if}
        </div>
      {/if}
    </div>
  {/if}
  <button
    type="button"
    class="dice-float-btn"
    onclick={() => (panelOpen = !panelOpen)}
    title={$_('initiative.title_dice_roller')}>
    <Dices size={20} />
  </button>
</div>

<style>
  .dice-panel {
    width: 16rem;
    padding: 0.6rem 0.7rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.45);
    font-family: 'Cinzel', serif;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
  }
  .dice-panel-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px dashed rgba(139, 105, 20, 0.4);
    padding-bottom: 0.3rem;
  }
  .dice-count-row { display: flex; gap: 0.2rem; flex-wrap: wrap; }
  .dice-count-btn {
    flex: 1 0 1.5rem;
    padding: 0.2rem 0.4rem;
    font-size: 0.7rem;
    background: rgba(255, 248, 220, 0.4);
    color: #2c1810;
    border: 1px solid #8b6914;
    border-radius: 4px;
    cursor: pointer;
    font-family: 'IM Fell English SC', serif;
  }
  .dice-count-btn.active {
    background: #c9a84c;
    color: #1a0f08;
    font-weight: 700;
  }
  .dice-quick { display: flex; flex-wrap: wrap; gap: 0.2rem; }
  .dice-die {
    display: inline-flex; align-items: center; gap: 0.15rem;
    padding: 0.2rem 0.5rem;
    font-size: 0.75rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    border-radius: 0.3rem;
    cursor: pointer;
    font-family: 'Cinzel', serif;
    font-weight: 700;
  }
  .dice-custom { display: flex; gap: 0.3rem; }
  .dice-custom input {
    background: rgba(255, 248, 220, 0.5);
    border: 1px solid #8b6914;
    color: #2c1810;
    padding: 0.2rem 0.4rem;
    border-radius: 3px;
    font-family: 'Special Elite', monospace;
    font-size: 0.8rem;
  }
  .dice-label-input {
    background: rgba(255, 248, 220, 0.5);
    border: 1px solid rgba(139, 105, 20, 0.5);
    color: #2c1810;
    padding: 0.15rem 0.4rem;
    border-radius: 3px;
    font-family: 'Special Elite', monospace;
    font-size: 0.7rem;
  }
  .dice-history {
    max-height: 8rem;
    overflow-y: auto;
    border-top: 1px dashed rgba(139, 105, 20, 0.3);
    padding-top: 0.3rem;
    display: flex; flex-direction: column; gap: 0.15rem;
  }
  .dice-history-row { display: flex; align-items: center; gap: 0.4rem; }
  .dice-float-btn {
    width: 3rem;
    height: 3rem;
    border-radius: 9999px;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1.5px solid #4e3909;
    box-shadow: 0 4px 8px rgba(0, 0, 0, 0.4);
    display: inline-flex; align-items: center; justify-content: center;
    cursor: pointer;
  }
</style>
