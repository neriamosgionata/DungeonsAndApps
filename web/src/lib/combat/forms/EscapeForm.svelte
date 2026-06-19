<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    combatants,
    escapeGrapplerId = $bindable(''),
    escapeResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    escapeGrapplerId?: string;
    escapeResult?: { escaped: boolean; escapee_total: number; grappler_total: number } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_select_grappler')}</span>
    <select bind:value={escapeGrapplerId}>
      <option value="">Select grappler…</option>
      {#each combatants as t (t.id)}
        {#if t.id !== activeC.id && t.conditions?.some(c => c.split(':')[0].toLowerCase() === 'grappling')}<option value={t.id}>{t.display_name}</option>{/if}
      {/each}
    </select>
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`escape:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`escape:${activeC.id}`)}>
    {$_('initiative.label_escape_grapple')}
  </button>
  {#if escapeResult}
    <div class="ca-result {escapeResult.escaped ? 'hit' : 'miss'}">
      <span>{escapeResult.escaped ? 'Escaped!' : 'Failed!'} {escapeResult.escapee_total} vs {escapeResult.grappler_total}</span>
    </div>
  {/if}
</div>
