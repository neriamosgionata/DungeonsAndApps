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
        <option value="str">{$_('initiative.ability_str')}</option>
        <option value="dex">{$_('initiative.ability_dex')}</option>
        <option value="con">{$_('initiative.ability_con')}</option>
        <option value="int">{$_('initiative.ability_int')}</option>
        <option value="wis">{$_('initiative.ability_wis')}</option>
        <option value="cha">{$_('initiative.ability_cha')}</option>
      </select>
    </label>
    <label class="ca-field"><span>{$_('initiative.label_dc')}</span><input type="number" bind:value={saveDc} min="1" max="40" /></label>
    <label class="ca-check"><input type="checkbox" bind:checked={saveAdv} /> {$_('initiative.label_adv')}</label>
    <label class="ca-check"><input type="checkbox" bind:checked={saveDis} /> {$_('initiative.label_dis')}</label>
  </div>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`save:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`save:${activeC.id}`)}>
    <Shield size={12} /> {$_('initiative.btn_roll_save')}
  </button>
  {#if saveResult}
    <div class="ca-result {saveResult.passed ? 'hit' : 'miss'}">
      <span>{saveResult.passed ? $_('initiative.msg_passed') : $_('initiative.msg_failed')} {saveResult.save_total} {$_('initiative.msg_vs_dc')} {saveResult.dc} ({$_('initiative.msg_rolled')} {saveResult.natural_roll})</span>
    </div>
  {/if}
</div>
