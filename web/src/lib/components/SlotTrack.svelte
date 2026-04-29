<script lang="ts">
  // Gold rivet slot tracker.
  let {
    current,
    max,
    onchange,
    label,
  }: {
    current: number;
    max: number;
    onchange: (current: number, max: number) => void;
    label?: string;
  } = $props();

  function set(n: number) {
    if (n < current) onchange(n, max);
    else onchange(n + 1, max);
  }

  function bumpMax(delta: number) {
    const nm = Math.max(0, Math.min(20, max + delta));
    onchange(Math.min(current, nm), nm);
  }
</script>

<div class="flex items-center gap-2">
  {#if label}<span class="w-14 text-[11px] uppercase tracking-[0.15em] font-display font-semibold" style="color:#8b6914;">{label}</span>{/if}
  <div class="flex gap-1">
    {#each Array(max) as _, i (i)}
      <button type="button" onclick={() => set(i)} aria-label="slot {i + 1}"
        class="rivet {i < current ? 'rivet-on' : 'rivet-off'}"></button>
    {/each}
  </div>
  <div class="ml-2 text-xs tabular-nums" style="color:#5c3d2e;">{current}/{max}</div>
  <div class="ml-auto flex gap-0.5">
    <button type="button" onclick={() => bumpMax(-1)} class="minmax">−max</button>
    <button type="button" onclick={() => bumpMax(1)}  class="minmax">+max</button>
  </div>
</div>

<style>
  .rivet {
    height: 1rem; width: 1rem; border-radius: 9999px;
    border: 1px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255, 248, 220, 0.45), 0 1px 2px rgba(0,0,0,0.4);
    transition: transform 0.05s;
  }
  .rivet-on {
    background: radial-gradient(circle at 35% 30%, #f4d97a 0%, #c9a84c 40%, #6d510f 100%);
  }
  .rivet-off {
    background: radial-gradient(circle at 35% 30%, #d4b896 0%, #8b6355 70%);
  }
  .rivet:hover { transform: scale(1.15); }

  .minmax {
    padding: 0 0.4rem;
    font-size: 0.7rem;
    border-radius: 0.25rem;
    border: 1px solid #4e3909;
    background-image: linear-gradient(180deg, #d4b896 0%, #8b6355 100%);
    color: #1a0f08;
    font-weight: 600;
  }
  .minmax:hover { background-image: linear-gradient(180deg, #e8d5a3 0%, #a18062 100%); }
</style>
