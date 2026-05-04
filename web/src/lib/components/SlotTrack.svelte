<script lang="ts">
  let {
    current,
    max,
    onchange,
    label,
    level,
  }: {
    current: number;
    max: number;
    onchange: (current: number, max: number) => void;
    label?: string;
    level?: number;
  } = $props();

  function toggle(i: number) {
    // clicking the last filled bubble empties it; otherwise fill up to i+1
    if (i + 1 === current) onchange(current - 1, max);
    else onchange(i + 1, max);
  }

  function bumpMax(delta: number) {
    const nm = Math.max(0, Math.min(20, max + delta));
    onchange(Math.min(current, nm), nm);
  }

  // bubble size scales slightly with spell level (1st=small … 9th=larger)
  const sz = $derived(level ? Math.min(14 + (level - 1), 20) : 16);
</script>

<div class="slot-row">
  <!-- level badge: fixed readable size regardless of level -->
  <div class="lvl-badge">
    {#if level}{level}{:else if label}{label}{/if}
  </div>

  <!-- bubbles -->
  <div class="bubbles">
    {#each Array(max) as _, i (i)}
      <button
        type="button"
        onclick={() => toggle(i)}
        aria-label="slot {i + 1} of {max}"
        class="bubble {i < current ? 'full' : 'empty'}"
        style="width:{sz}px; height:{sz}px;"
      ></button>
    {/each}
  </div>

  <!-- count -->
  <span class="count">{current}/{max}</span>

  <!-- max controls -->
  <div class="max-ctrl">
    <button type="button" onclick={() => bumpMax(-1)} aria-label="decrease max" class="adj">−</button>
    <button type="button" onclick={() => bumpMax(1)}  aria-label="increase max" class="adj">+</button>
  </div>
</div>

<style>
  .slot-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .lvl-badge {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 1.5rem;
    height: 1.5rem;
    border-radius: 9999px;
    background: linear-gradient(180deg, #c9a84c 0%, #7a5010 100%);
    border: 1.5px solid #f4e4c1;
    color: #1a0f08;
    text-shadow: 0 1px 0 rgba(255,245,200,0.5);
    font-weight: 800;
    font-size: 0.75rem;
    font-family: var(--font-display, serif);
    box-shadow: 0 1px 3px rgba(0,0,0,0.5), inset 0 1px 0 rgba(255,245,200,0.4);
    flex-shrink: 0;
  }

  .bubbles {
    display: flex;
    flex-wrap: wrap;
    gap: 0.25rem;
  }

  .bubble {
    border-radius: 9999px;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.4), 0 1px 2px rgba(0,0,0,0.4);
    transition: transform 0.06s, box-shadow 0.06s;
    flex-shrink: 0;
    cursor: pointer;
  }
  .bubble:hover { transform: scale(1.18); box-shadow: 0 0 5px rgba(201,168,76,0.6); }

  .full {
    background: radial-gradient(circle at 35% 30%, #f9e47a 0%, #c9a84c 45%, #6d510f 100%);
    box-shadow: 0 0 4px rgba(201,168,76,0.45), inset 0 1px 0 rgba(255,248,220,0.5);
  }
  .empty {
    background: radial-gradient(circle at 35% 30%, #4a3018 0%, #2a1a08 100%);
    border-color: #7a5a20;
    opacity: 0.7;
  }

  .count {
    font-size: 0.72rem;
    color: #c9a48c;
    min-width: 2.5rem;
    text-align: center;
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }

  .max-ctrl {
    display: flex;
    gap: 2px;
    margin-left: auto;
  }
  .adj {
    width: 1.4rem;
    height: 1.4rem;
    border-radius: 0.25rem;
    border: 1.5px solid #f4e4c1;
    background: linear-gradient(180deg, #c9a84c 0%, #6d510f 100%);
    color: #1a0f08;
    font-weight: 800;
    font-size: 0.85rem;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 1px 2px rgba(0,0,0,0.4);
  }
  .adj:hover { background: linear-gradient(180deg, #f4d060 0%, #9a7020 100%); }
</style>
