<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    combatants,
    shoveTarget = $bindable(''),
    shoveKnockProne = $bindable(true),
    shoveResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    shoveTarget?: string;
    shoveKnockProne?: boolean;
    shoveResult?: { success: boolean; attacker_total: number; defender_total: number; knocked_prone?: boolean; pushed_away?: boolean } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_target')}</span>
    <select bind:value={shoveTarget}>
      <option value="">{$_('initiative.ph_select_target')}</option>
      {#each combatants as t (t.id)}
        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
      {/each}
    </select>
  </label>
  <label class="ca-check">
    <input type="checkbox" bind:checked={shoveKnockProne} /> {$_('initiative.msg_knock_prone_or_push')}
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`shove:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`shove:${activeC.id}`)}>
    {$_('initiative.label_shove_submit')}
  </button>
  {#if shoveResult}
    <div class="ca-result {shoveResult.success ? 'hit' : 'miss'}">
      <span>{shoveResult.success ? $_('initiative.msg_grapple_success') : $_('initiative.msg_grapple_failed')} {shoveResult.attacker_total} vs {shoveResult.defender_total}</span>
      {#if shoveResult.knocked_prone}<span>{$_('initiative.msg_target_prone')}</span>{/if}
      {#if shoveResult.pushed_away}<span>{$_('initiative.msg_target_pushed')}</span>{/if}
    </div>
  {/if}
</div>
