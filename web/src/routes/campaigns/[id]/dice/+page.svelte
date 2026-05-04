<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy } from 'svelte';
  import { Dice } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { useCampaign } from '$lib/campaignCtx.svelte';
  import { _ } from 'svelte-i18n';
  import type { DiceRollResult, DiceHistory } from '$lib/types';
  import { Dices, X, Lock, History, Eraser, Plus, Minus } from '@lucide/svelte';

  const campaign = useCampaign();
  const cid = $derived(page.params.id!);

  let history = $state<DiceHistory[]>([]);
  let label = $state('');
  let isPrivate = $state(false);
  let modifier = $state(0);
  let last = $state<DiceRollResult | null>(null);
  let error = $state('');
  let loading = $state(true);
  // tray: list of {sides, count}
  let tray = $state<{ sides: number; count: number }[]>([]);
  // custom expression fallback
  let customExpr = $state('');

  const DICE = [4, 6, 8, 10, 12, 20, 100];
  let quickCount = $state(1);

  function addDie(sides: number, n = quickCount) {
    const exists = tray.some((t) => t.sides === sides);
    if (exists) tray = tray.map((t) => t.sides === sides ? { ...t, count: t.count + n } : t);
    else tray = [...tray, { sides, count: n }];
  }
  function removeDie(sides: number) {
    tray = tray.map((t) => t.sides === sides ? { ...t, count: t.count - 1 } : t)
               .filter((t) => t.count > 0);
  }
  function clearTray() { tray = []; modifier = 0; customExpr = ''; }

  const expression = $derived.by(() => {
    if (customExpr.trim()) return customExpr.trim();
    const parts = tray.map((t) => `${t.count}d${t.sides}`);
    if (modifier > 0) parts.push(`+${modifier}`);
    else if (modifier < 0) parts.push(`${modifier}`);
    return parts.join('+') || '';
  });

  async function load() { try { history = await Dice.history(cid, 30); } finally { loading = false; } }
  onMount(load);

  let off: (() => void) | undefined;
  onMount(() => {
    off = campaignSocket.on((ev) => {
      if (ev.type === 'dice_cleared') { load(); return; }
      if (ev.type !== 'dice_roll') return;
      if (campaign().isMaster || ev.user_id === auth.user?.id) load();
    });
  });
  onDestroy(() => off?.());

  async function clearHistory() {
    if (!confirm($_('dice.clear_confirm'))) return;
    try { await Dice.clear(cid); last = null; await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function roll() {
    const expr = expression;
    if (!expr) return;
    error = '';
    try {
      last = await Dice.roll(cid, expr, label || undefined, isPrivate);
      await load();
    } catch (e) { error = (e as Error).message; }
  }

</script>

<section class="tower">
  <!-- header -->
  <header class="tower-head">
    <div class="hdr-icon"><Dices size={28} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('dice.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('dice.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
    <div style="width:2.5rem;"></div>
  </header>

  <div class="rule"></div>

  <!-- dice picker -->
  <div class="die-picker-wrap">
    <div class="count-selector">
      {#each [1,2,3,4,5,6,8,10] as n (n)}
        <button type="button" class="count-btn {quickCount === n ? 'active' : ''}"
          onclick={() => quickCount = n}>{n}</button>
      {/each}
    </div>
    <div class="die-picker">
      {#each DICE as d (d)}
        <button type="button" class="die-btn" onclick={() => addDie(d)}>
          {#if quickCount > 1}<span class="die-qty">{quickCount}×</span>{/if}
          <span class="die-face">d{d}</span>
        </button>
      {/each}
    </div>
  </div>

  <!-- tray -->
  <div class="tray-row">
    {#if tray.length === 0}
      <span class="tray-empty">{$_('dice.tray_empty')}</span>
    {:else}
      {#each tray as t (t.sides)}
        <div class="tray-chip">
          <button type="button" class="tray-minus" onclick={() => removeDie(t.sides)}><Minus size={10} /></button>
          <span class="tray-count">{t.count}</span>
          <span class="tray-die">d{t.sides}</span>
          <button type="button" class="tray-plus" onclick={() => addDie(t.sides)}><Plus size={10} /></button>
        </div>
      {/each}
    {/if}
  </div>

  <!-- modifier + options -->
  <div class="options-row">
    <label class="mod-wrap">
      <span class="opt-label">±</span>
      <button type="button" class="mod-btn" onclick={() => modifier--}><Minus size={12} /></button>
      <span class="mod-val">{modifier >= 0 ? '+' : ''}{modifier}</span>
      <button type="button" class="mod-btn" onclick={() => modifier++}><Plus size={12} /></button>
    </label>
    <input class="label-input" placeholder={$_('dice.label_ph')} bind:value={label} />
    <label class="priv-wrap">
      <input type="checkbox" bind:checked={isPrivate} />
      <Lock size={12} /> {$_('dice.private')}
    </label>
  </div>

  <!-- custom expression override -->
  <details class="custom-expr" open={!!customExpr.trim()}>
    <summary>{$_('dice.add_expr')}</summary>
    <input bind:value={customExpr} placeholder="e.g. 2d20kh1+5" class="expr-input" />
    {#if customExpr.trim() && (tray.length || modifier)}
      <p class="expr-hint">{$_('dice.custom_overrides_hint')}</p>
    {/if}
  </details>

  <!-- expression preview + roll -->
  <div class="roll-row">
    <span class="expr-preview">{expression || '…'}</span>
    <button type="button" class="roll-btn" disabled={!expression} onclick={roll}>
      <Dices size={16} /> {$_('dice.roll')}
    </button>
    {#if tray.length || modifier || customExpr}
      <button type="button" class="clear-btn" onclick={clearTray}>
        <X size={13} /> {$_('dice.clear_tray')}
      </button>
    {/if}
  </div>

  {#if error}<p class="err">{error}</p>{/if}
  {#if loading}<p class="mt-3 text-sm italic" style="color:#8b6355;">{$_('common.loading')}</p>{/if}

  <!-- result -->
  {#if last}
    <div class="result">
      <div class="result-total">{last.total}</div>
      <div class="result-expr">{last.expression}{last.label ? ` · ${last.label}` : ''}</div>
      {#if last.terms?.length}
        <div class="result-terms">
          {#each last.terms as term (term.expr)}
            {#if term.kind === 'dice' && term.rolls?.length}
              {@const sides = term.expr.match(/d(\d+)/)?.[1] ?? '?'}
              {@const keptFlags = (() => {
                // Multiset-consume kept values: the i-th occurrence of value v in
                // `rolls` is "kept" iff `kept` also contains at least i+1 v's.
                const remaining = new Map<number, number>();
                for (const k of term.kept ?? []) remaining.set(k, (remaining.get(k) ?? 0) + 1);
                return (term.rolls ?? []).map((r) => {
                  const left = remaining.get(r) ?? 0;
                  if (left > 0) { remaining.set(r, left - 1); return true; }
                  return false;
                });
              })()}
              {#each term.rolls as roll, ri (ri)}
                <span class="term {keptFlags[ri] === false ? 'term-dropped' : ''}">
                  <span class="term-expr">D{sides}</span>
                  <span class="term-val">{roll}</span>
                </span>
              {/each}
            {:else if term.kind === 'modifier'}
              <span class="term term-mod">
                <span class="term-expr">mod</span>
                <span class="term-val">{term.value >= 0 ? '+' : ''}{term.value}</span>
              </span>
            {/if}
          {/each}
        </div>
      {/if}
    </div>
  {/if}

  <!-- history -->
  <div class="hist-head">
    <h3 class="hist-title"><History size={16} /> {$_('dice.history')}</h3>
    {#if campaign().isMaster && history.length > 0}
      <button onclick={clearHistory} class="hist-clear-btn">
        <Eraser size={13} /> {$_('dice.clear')}
      </button>
    {/if}
  </div>
  <ul class="hist-list">
    {#each history as h (h.id)}
      <li class="hist-item">
        <span class="hist-expr">
          {h.expression}{h.label ? ` · ${h.label}` : ''}
          {#if h.private}<Lock size={11} style="color:#8b6355;" />{/if}
        </span>
        <span class="hist-total">{h.total}</span>
      </li>
    {:else}
      <li class="hist-empty">{$_('dice.history_empty')}</li>
    {/each}
  </ul>
</section>

<style>
  .tower { max-width: 48rem; margin: 0 auto; padding: 1rem 1.25rem; }

  /* header */
  .tower-head {
    display: grid; grid-template-columns: auto 1fr auto;
    align-items: center; gap: 1rem;
  }
  .hdr-center { text-align: center; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.6rem, 3vw, 2.4rem);
    font-weight: 900; letter-spacing: 0.08em;
    color: #2c1810; line-height: 1;
  }
  .hdr-sub {
    margin-top: 0.25rem;
    font-family: 'Crimson Text', serif; font-style: italic;
    font-size: 0.85rem; color: #6d510f;
  }
  .fleuron { color: #8b6914; margin: 0 0.4rem; font-style: normal; }

  .rule {
    height: 3px; margin: 0.85rem 0 1.25rem;
    background: linear-gradient(90deg, transparent 0%, #8b6914 8%, #c9a84c 50%, #8b6914 92%, transparent 100%);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦"; position: absolute; top: 50%; left: 50%;
    transform: translate(-50%,-50%); color: #6d510f;
    background: #f4e4c1; padding: 0 0.5rem; font-size: 0.9rem;
  }

  /* dice picker */
  .die-picker-wrap { margin-bottom: 0.85rem; }
  .count-selector {
    display: flex; gap: 0.35rem; flex-wrap: wrap;
    margin-bottom: 0.5rem;
  }
  .count-btn {
    min-width: 2rem; height: 1.75rem;
    border-radius: 0.25rem;
    border: 1.5px solid rgba(139,105,20,0.4);
    background: rgba(244,228,193,0.5);
    color: #6d510f;
    font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.75rem;
    transition: background 0.08s;
  }
  .count-btn:hover { background: rgba(201,168,76,0.3); }
  .count-btn.active {
    background: linear-gradient(180deg, #c9a84c, #6d510f);
    border-color: #4e3909; color: #1a0f08;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.4);
  }
  .die-picker {
    display: flex; gap: 0.6rem; flex-wrap: wrap;
  }
  .die-btn {
    display: flex; flex-direction: column; align-items: center; justify-content: center;
    width: 4rem; height: 4rem;
    border: 2px solid #8b6914;
    border-radius: 0.4rem;
    background: linear-gradient(180deg, #f4e4c1 0%, #e8d2a0 100%);
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 3px 6px rgba(0,0,0,0.35);
    transition: transform 0.08s, box-shadow 0.08s;
    cursor: pointer;
  }
  .die-btn:hover {
    transform: translateY(-2px);
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.7), 0 6px 12px rgba(0,0,0,0.45);
    background: linear-gradient(180deg, #fffae8 0%, #f4e4c1 100%);
  }
  .die-btn:active { transform: translateY(1px); }
  .die-qty {
    font-family: 'Cinzel', serif; font-weight: 700;
    font-size: 0.65rem; color: #8b6914;
    letter-spacing: 0.05em; line-height: 1;
  }
  .die-face {
    font-family: 'Cinzel', serif; font-weight: 900;
    font-size: 1rem; color: #2c1810;
    letter-spacing: 0.05em;
  }

  /* tray */
  .tray-row {
    min-height: 2.75rem;
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    padding: 0.55rem 0.8rem;
    border: 1.5px solid rgba(139,105,20,0.5);
    border-radius: 0.35rem;
    background: rgba(244,228,193,0.75);
    margin-bottom: 0.75rem;
  }
  .tray-empty {
    font-family: 'Crimson Text', serif; font-style: italic;
    font-size: 0.85rem; color: #8b6355;
  }
  .tray-chip {
    display: inline-flex; align-items: center; gap: 0.25rem;
    padding: 0.25rem 0.5rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.3rem;
    background: #2c1810;
    color: #f4e4c1;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    font-weight: 700;
  }
  .tray-die { color: #c9a84c; }
  .tray-count { min-width: 1.1rem; text-align: center; }
  .tray-minus, .tray-plus {
    width: 1.2rem; height: 1.2rem;
    display: grid; place-items: center;
    border-radius: 0.2rem;
    background: rgba(201,168,76,0.15);
    color: #c9a84c;
    border: 1px solid rgba(201,168,76,0.35);
  }
  .tray-minus:hover, .tray-plus:hover { background: rgba(201,168,76,0.3); }

  /* options */
  .options-row {
    display: flex; gap: 0.6rem; flex-wrap: wrap; align-items: center;
    margin-bottom: 0.55rem;
  }
  .mod-wrap {
    display: inline-flex; align-items: center; gap: 0.3rem;
    border: 1.5px solid rgba(139,105,20,0.5);
    border-radius: 0.3rem;
    padding: 0.3rem 0.5rem;
    background: rgba(244,228,193,0.75);
    color: #2c1810;
    font-family: 'Special Elite', monospace;
  }
  .opt-label {
    font-family: 'Cinzel', serif; font-size: 0.7rem;
    letter-spacing: 0.1em; text-transform: uppercase; color: #6d510f;
    margin-right: 0.2rem;
  }
  .mod-btn {
    width: 1.4rem; height: 1.4rem;
    display: grid; place-items: center;
    border-radius: 0.2rem;
    border: 1px solid rgba(139,105,20,0.4);
    background: rgba(139,105,20,0.12);
    color: #6d510f;
  }
  .mod-btn:hover { background: rgba(139,105,20,0.25); }
  .mod-val {
    min-width: 2rem; text-align: center;
    font-weight: 700; font-size: 0.85rem;
  }
  .label-input {
    flex: 1; min-width: 8rem;
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    background: rgba(244,228,193,0.75) !important;
    color: #2c1810 !important;
    border-radius: 0.3rem !important;
    padding: 0.35rem 0.65rem !important;
    font-family: 'Crimson Text', serif;
    font-size: 0.9rem;
  }
  .priv-wrap {
    display: inline-flex; align-items: center; gap: 0.35rem;
    font-family: 'Cinzel', serif; font-size: 0.75rem;
    letter-spacing: 0.08em; text-transform: uppercase; color: #6d510f;
  }

  /* custom expr */
  .custom-expr {
    margin-bottom: 0.6rem;
    font-family: 'Cinzel', serif; font-size: 0.75rem;
    color: #6d510f; letter-spacing: 0.08em; text-transform: uppercase;
  }
  .custom-expr summary { cursor: pointer; }
  .expr-hint {
    margin-top: 0.3rem;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.75rem;
    color: #8b1a1a;
    text-transform: none;
    letter-spacing: 0;
  }
  .expr-input {
    margin-top: 0.35rem; width: 100%;
    border: 1.5px solid rgba(139,105,20,0.5) !important;
    background: rgba(244,228,193,0.75) !important;
    color: #2c1810 !important;
    border-radius: 0.3rem !important;
    padding: 0.35rem 0.65rem !important;
    font-family: 'Special Elite', monospace; font-size: 0.85rem;
  }

  /* roll row */
  .roll-row {
    display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap;
    margin-bottom: 0.85rem;
  }
  .expr-preview {
    flex: 1; min-width: 6rem;
    font-family: 'Special Elite', monospace; font-size: 1rem;
    color: #6d510f; letter-spacing: 0.05em;
  }
  .roll-btn {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.55rem 1.4rem;
    border-radius: 0.35rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    font-family: 'Cinzel', serif; font-weight: 700;
    font-size: 0.85rem; letter-spacing: 0.08em; text-transform: uppercase;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.4);
  }
  .roll-btn:hover:not(:disabled) { background-image: linear-gradient(180deg, #e5c065, #a98517 55%, #7e5e10); }
  .roll-btn:disabled { opacity: 0.45; cursor: not-allowed; }
  .clear-btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.45rem 0.75rem;
    border-radius: 0.3rem;
    background: rgba(139,26,26,0.12);
    color: #8b1a1a; border: 1px solid rgba(139,26,26,0.35);
    font-family: 'Cinzel', serif; font-size: 0.72rem;
    letter-spacing: 0.06em; text-transform: uppercase;
  }
  .clear-btn:hover { background: rgba(139,26,26,0.25); }

  .err { color: #c95a5a; font-size: 0.85rem; margin-bottom: 0.5rem; }

  /* result */
  .result {
    padding: 1rem 1.25rem;
    margin-bottom: 1.25rem;
    border: 2px solid #c9a84c;
    border-radius: 0.45rem;
    background:
      radial-gradient(circle at 20% 30%, rgba(201,168,76,0.25) 0%, transparent 60%),
      linear-gradient(180deg, rgba(244,228,193,0.15), transparent 55%),
      #2c1810;
    box-shadow: 0 0 0 1px rgba(201,168,76,0.2), 0 6px 16px rgba(0,0,0,0.55);
    color: #f7e2a5;
  }
  .result-total {
    font-family: 'IM Fell English SC', serif;
    font-size: clamp(3rem, 8vw, 5rem);
    font-weight: 900; line-height: 1;
    color: #f7e2a5;
    text-shadow: 0 0 20px rgba(201,168,76,0.6);
  }
  .result-expr {
    font-family: 'Special Elite', monospace;
    font-size: 0.8rem; color: rgba(244,228,193,0.65);
    letter-spacing: 0.05em; margin-top: 0.25rem;
  }
  .result-terms {
    display: flex; flex-wrap: wrap; gap: 0.5rem;
    margin-top: 0.65rem;
  }
  .term {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.2rem 0.55rem;
    border-radius: 0.25rem;
    background: rgba(201,168,76,0.12);
    border: 1px solid rgba(201,168,76,0.35);
    font-family: 'Special Elite', monospace; font-size: 0.78rem;
  }
  .term-expr { color: #c9a84c; font-size: 0.7rem; }
  .term-val { color: #f4e4c1; font-weight: 700; font-size: 1rem; }
  .term-dropped { opacity: 0.4; }
  .term-dropped .term-val { text-decoration: line-through; color: rgba(244,228,193,0.5); }
  .term-mod { background: rgba(78,57,9,0.35); border-color: rgba(139,105,20,0.25); }
  .term-mod .term-expr { color: #8b6914; }

  /* history */
  .hist-head {
    display: flex; justify-content: space-between; align-items: center;
    margin-bottom: 0.6rem;
  }
  .hist-title {
    display: inline-flex; align-items: center; gap: 0.4rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 1.1rem; letter-spacing: 0.1em;
    text-transform: uppercase; color: #6d510f;
  }
  .hist-clear-btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.35rem 0.75rem;
    border-radius: 0.3rem;
    background: rgba(139,26,26,0.12);
    color: #8b1a1a; border: 1px solid rgba(139,26,26,0.35);
    font-family: 'Cinzel', serif; font-size: 0.72rem;
    letter-spacing: 0.06em; text-transform: uppercase;
  }
  .hist-clear-btn:hover { background: rgba(139,26,26,0.25); }

  .hist-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.3rem; }
  .hist-item {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.5rem 0.85rem;
    border: 1.5px solid rgba(139,105,20,0.3);
    border-radius: 0.35rem;
    background: rgba(244,228,193,0.75);
    color: #2c1810;
  }
  .hist-expr {
    display: inline-flex; align-items: center; gap: 0.35rem;
    font-family: 'Special Elite', monospace; font-size: 0.82rem;
    color: #6d510f;
  }
  .hist-total {
    font-family: 'IM Fell English SC', serif;
    font-size: 1.15rem; font-weight: 900; color: #2c1810;
  }
  .hist-empty {
    font-style: italic; color: #8b6355;
    font-family: 'Crimson Text', serif; font-size: 0.9rem;
    padding: 0.5rem 0;
  }
</style>
