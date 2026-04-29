<script lang="ts">
  import CoinInput from './CoinInput.svelte';

  type Kind = 'cp' | 'sp' | 'ep' | 'gp' | 'pp';

  let {
    values,
    onchange,
    compact = false,
  }: {
    values: Partial<Record<Kind, number>>;
    onchange: (kind: Kind, value: number) => void;
    compact?: boolean;
  } = $props();

  const order: Kind[] = ['pp', 'gp', 'ep', 'sp', 'cp'];

  const totalGp = $derived(
    (values.pp ?? 0) * 10 +
    (values.gp ?? 0) +
    (values.ep ?? 0) * 0.5 +
    (values.sp ?? 0) * 0.1 +
    (values.cp ?? 0) * 0.01
  );
</script>

<div class="purse {compact ? 'compact' : ''}">
  <div class="grid gap-1.5 {compact ? 'grid-cols-5' : 'sm:grid-cols-2 lg:grid-cols-3'}">
    {#each order as k (k)}
      <CoinInput kind={k} value={values[k] ?? 0} onchange={(v) => onchange(k, v)} {compact} />
    {/each}
  </div>
  <div class="total">
    <span class="label">≈</span>
    <span class="amount">{totalGp.toLocaleString(undefined, { maximumFractionDigits: 2 })}</span>
    <span class="unit">gp</span>
  </div>
</div>

<style>
  .purse { display: flex; flex-direction: column; gap: 0.5rem; }
  .compact { gap: 0.375rem; }
  .total {
    align-self: flex-end;
    display: inline-flex;
    align-items: baseline;
    gap: 0.375rem;
    padding: 0.2rem 0.7rem;
    border-radius: 9999px;
    border: 1px solid #4e3909;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 100%);
    box-shadow: inset 0 1px 0 rgba(255, 248, 220, 0.55), 0 1px 2px rgba(0,0,0,0.5);
    font-family: 'Cinzel', serif;
    color: #1a0f08;
    text-shadow: 0 1px 0 rgba(247, 226, 165, 0.5);
  }
  .total .label  { color: #1a0f08; font-weight: 700; }
  .total .amount { font-size: 0.9rem; font-weight: 800; letter-spacing: 0.03em; color: #1a0f08; }
  .total .unit   { font-size: 0.65rem; text-transform: uppercase; letter-spacing: 0.15em; color: #3a2313; font-weight: 700; }
</style>
