<script lang="ts">
  let {
    value,
    onchange,
    min = -Infinity,
    max = Infinity,
    step = 1,
    label,
    compact = false,
  }: {
    value: number;
    onchange: (v: number) => void;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    compact?: boolean;
  } = $props();

  function clamp(v: number) { return Math.max(min, Math.min(max, v)); }
  function bump(delta: number) { onchange(clamp(value + delta)); }
</script>

<div class="stepper {compact ? 'compact' : ''}">
  {#if label}<span class="lbl">{label}</span>{/if}
  <div class="row">
    <button type="button" aria-label="decrease" onclick={() => bump(-step)} class="gold-btn">−</button>
    <input type="number" {min} {max} {step} {value}
      onchange={(e) => onchange(clamp(+(e.currentTarget as HTMLInputElement).value))} />
    <button type="button" aria-label="increase" onclick={() => bump(step)} class="gold-btn">+</button>
  </div>
</div>

<style>
  .stepper { display: flex; flex-direction: column; }
  .lbl {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.15em;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    color: #8b6914;
    margin-bottom: 0.25rem;
  }
  .row {
    display: flex;
    align-items: stretch;
    gap: 0.25rem;
    height: 2rem;
  }
  .stepper.compact .row { height: 1.5rem; }
  .row input {
    flex: 1;
    min-width: 0;
    padding: 0 0.5rem !important;
    line-height: 1 !important;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    text-align: center;
    tab-size: 1;
  }
  .gold-btn {
    aspect-ratio: 1 / 1;
    flex: none;
    height: 100%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 9999px;
    font-size: 1rem;
    line-height: 1;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    border: 1.5px solid #4e3909;
    color: #1a0f08;
    font-weight: 700;
    font-family: Georgia, serif;
    text-shadow: 0 1px 0 rgba(244, 228, 193, 0.5);
    box-shadow:
      inset 0 1px 0 rgba(255, 248, 220, 0.55),
      inset 0 -2px 3px rgba(0, 0, 0, 0.35),
      0 1px 2px rgba(0, 0, 0, 0.6);
    transition: transform 0.05s;
  }
  .gold-btn:hover {
    background-image: linear-gradient(180deg, #e5c065 0%, #a98517 55%, #7e5e10 100%);
  }
  .gold-btn:active { transform: translateY(1px) scale(0.97); }
</style>
