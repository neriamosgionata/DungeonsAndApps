<script lang="ts">
  // Themed currency input: embossed coin medallion + value + ± controls.
  // Full mode: horizontal row w/ -10/-1/input/+1/+10.
  // Compact mode: vertical stack, ±1 only, tiny medallion — fits 5-wide in a character card.

  type Kind = 'cp' | 'sp' | 'ep' | 'gp' | 'pp';

  let {
    kind,
    value,
    onchange,
    compact = false,
  }: {
    kind: Kind;
    value: number;
    onchange: (v: number) => void;
    compact?: boolean;
  } = $props();

  const meta: Record<Kind, { label: string; title: string }> = {
    cp: { label: 'CP', title: 'Copper' },
    sp: { label: 'SP', title: 'Silver' },
    ep: { label: 'EP', title: 'Electrum' },
    gp: { label: 'GP', title: 'Gold' },
    pp: { label: 'PP', title: 'Platinum' },
  };

  function clamp(v: number) { return Math.max(0, Math.min(999_999_999, Math.round(v))); }
  function bump(n: number) { onchange(clamp(value + n)); }
  function onInput(e: Event) {
    onchange(clamp(+(e.currentTarget as HTMLInputElement).value || 0));
  }
</script>

{#if compact}
  <div class="coin coin-{kind} compact" title={meta[kind].title}>
    <div class="medallion"><span class="label">{meta[kind].label}</span></div>
    <input type="number" min="0" {value} onchange={onInput} />
    <div class="compact-pills">
      <button type="button" aria-label="-1" onclick={() => bump(-1)} class="pill">−</button>
      <button type="button" aria-label="+1" onclick={() => bump(1)}  class="pill">+</button>
    </div>
  </div>
{:else}
  <div class="coin coin-{kind}" title={meta[kind].title}>
    <div class="medallion"><span class="label">{meta[kind].label}</span></div>
    <div class="controls">
      <button type="button" aria-label="-10" onclick={() => bump(-10)} class="pill">−10</button>
      <button type="button" aria-label="-1"  onclick={() => bump(-1)}  class="pill">−1</button>
      <input type="number" min="0" {value} onchange={onInput} />
      <button type="button" aria-label="+1"  onclick={() => bump(1)}   class="pill">+1</button>
      <button type="button" aria-label="+10" onclick={() => bump(10)}  class="pill">+10</button>
    </div>
  </div>
{/if}

<style>
  .coin {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem;
    border-radius: 0.75rem;
    border: 1.5px solid #4e3909;
    background:
      linear-gradient(180deg, rgba(244, 228, 193, 0.06), transparent 40%),
      #241810;
    box-shadow: inset 0 1px 0 rgba(244, 228, 193, 0.08), 0 2px 6px rgba(0,0,0,0.5);
  }
  .coin.compact {
    flex-direction: column;
    gap: 0.25rem;
    padding: 0.35rem 0.25rem;
    border-radius: 0.5rem;
  }

  .medallion {
    width: 3rem; height: 3rem; flex: none;
    border-radius: 9999px;
    display: grid; place-items: center;
    font-family: 'Cinzel', serif;
    font-weight: 900;
    font-size: 0.95rem;
    letter-spacing: 0.08em;
    box-shadow:
      inset 0 2px 0 rgba(255, 255, 255, 0.45),
      inset 0 -3px 4px rgba(0, 0, 0, 0.45),
      0 2px 4px rgba(0, 0, 0, 0.6);
    text-shadow: 0 1px 0 rgba(255, 255, 255, 0.35);
  }
  .compact .medallion {
    width: 1.6rem; height: 1.6rem;
    font-size: 0.6rem;
    letter-spacing: 0.04em;
  }
  .label { line-height: 1; }

  /* D&D coin denominations — metallic gradients */
  .coin-cp .medallion { background: radial-gradient(circle at 35% 28%, #e89b66 0%, #b8723a 45%, #6b3a18 100%); color: #2a1208; }
  .coin-sp .medallion { background: radial-gradient(circle at 35% 28%, #f6f6f6 0%, #bfbfbf 45%, #6b6b6b 100%); color: #1a1a1a; }
  .coin-ep .medallion { background: radial-gradient(circle at 35% 28%, #fbeedc 0%, #cbbb8e 45%, #7f6c3c 100%); color: #2a1d05; }
  .coin-gp .medallion { background: radial-gradient(circle at 35% 28%, #f7e2a5 0%, #c9a84c 45%, #6d510f 100%); color: #2a1d05; }
  .coin-pp .medallion { background: radial-gradient(circle at 35% 28%, #ffffff 0%, #dcdde0 45%, #7a7d82 100%); color: #1a1a1a; }

  .controls {
    display: grid;
    grid-template-columns: auto auto minmax(0, 1fr) auto auto;
    gap: 0.25rem;
    align-items: stretch;
    flex: 1;
  }
  .compact input {
    min-width: 0;
    width: 100%;
    padding: 0.1rem 0.25rem !important;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    text-align: center;
  }
  .compact-pills {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.2rem;
    width: 100%;
  }
  .compact .pill {
    padding: 0.05rem 0 !important;
    font-size: 0.7rem;
    border-radius: 0.25rem;
  }

  .pill {
    padding: 0.25rem 0.5rem;
    border-radius: 0.375rem;
    font-size: 0.75rem;
    font-weight: 600;
    border: 1px solid #4e3909;
    color: #1a0f08;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 100%);
    text-shadow: 0 1px 0 rgba(244, 228, 193, 0.4);
    box-shadow: inset 0 1px 0 rgba(255, 248, 220, 0.5);
    transition: transform 0.05s;
  }
  .pill:hover { background-image: linear-gradient(180deg, #e5c065 0%, #a98517 100%); }
  .pill:active { transform: translateY(1px); }

  .controls input {
    min-width: 0;
    padding: 0.25rem 0.5rem !important;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 1rem;
    text-align: center;
    tab-size: 1;
  }
</style>
