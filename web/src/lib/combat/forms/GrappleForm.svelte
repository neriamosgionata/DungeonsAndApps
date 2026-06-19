<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    combatants,
    grappleTarget = $bindable(''),
    grappleResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    grappleTarget?: string;
    grappleResult?: { success: boolean; attacker_total: number; defender_total: number; grapple_applied?: boolean } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_target')}</span>
    <select bind:value={grappleTarget}>
      <option value="">Select target…</option>
      {#each combatants as t (t.id)}
        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
      {/each}
    </select>
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`grapple:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`grapple:${activeC.id}`)}>
    {$_('initiative.label_grapple_submit')}
  </button>
  {#if grappleResult}
    <div class="ca-result {grappleResult.success ? 'hit' : 'miss'}">
      <span>{grappleResult.success ? 'Success!' : 'Failed!'} {grappleResult.attacker_total} vs {grappleResult.defender_total}</span>
      {#if grappleResult.grapple_applied}<span>Target grappled!</span>{/if}
    </div>
  {/if}
</div>
