<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Shield } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    saveAbility = $bindable('str'),
    saveDc = $bindable(10),
    saveAdv = $bindable(false),
    saveDis = $bindable(false),
    saveResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    saveAbility?: string;
    saveDc?: number;
    saveAdv?: boolean;
    saveDis?: boolean;
    saveResult?: { passed: boolean; save_total: number; dc: number; natural_roll: number } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_ability')}</span>
      <select bind:value={saveAbility}>
        <option value="str">STR</option>
        <option value="dex">DEX</option>
        <option value="con">CON</option>
        <option value="int">INT</option>
        <option value="wis">WIS</option>
        <option value="cha">CHA</option>
      </select>
    </label>
    <label class="ca-field"><span>DC</span><input type="number" bind:value={saveDc} min="1" max="40" /></label>
    <label class="ca-check"><input type="checkbox" bind:checked={saveAdv} /> Adv</label>
    <label class="ca-check"><input type="checkbox" bind:checked={saveDis} /> Dis</label>
  </div>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`save:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`save:${activeC.id}`)}>
    <Shield size={12} /> Roll Save
  </button>
  {#if saveResult}
    <div class="ca-result {saveResult.passed ? 'hit' : 'miss'}">
      <span>{saveResult.passed ? 'Passed!' : 'Failed!'} {saveResult.save_total} vs DC {saveResult.dc} (rolled {saveResult.natural_roll})</span>
    </div>
  {/if}
</div>
