<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Shield } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    combatants,
    reactType = $bindable('shield'),
    reactLabel = $bindable(''),
    reactTargetCasterId = $bindable(''),
    reactSlotLevel = $bindable(0),
    reactAbilityCheckTotal = $bindable(0),
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    reactType?: string;
    reactLabel?: string;
    reactTargetCasterId?: string;
    reactSlotLevel?: number;
    reactAbilityCheckTotal?: number;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();

  const REACT_TYPES = [
    { id: 'shield', labelKey: 'react_shield' },
    { id: 'counterspell', labelKey: 'react_counterspell' },
    { id: 'opportunity_attack', labelKey: 'react_opportunity' },
    { id: 'custom', labelKey: 'react_custom' },
  ];
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_reaction')}</span>
    <select bind:value={reactType}>
      {#each REACT_TYPES as r (r.id)}
        <option value={r.id}>{$_(`initiative.${r.labelKey}`)}</option>
      {/each}
    </select>
  </label>
  {#if reactType === 'shield'}
    {#if activeC.last_hit_attack_total}
      <div class="ca-result" style="background:rgba(200,160,60,0.15)">
        <span>{$_('initiative.react_hit_received')} {activeC.last_hit_attack_total} {$_('initiative.msg_versus_ac')} {activeC.ac}</span>
        <span>{$_('initiative.label_pending_damage', { values: { amount: activeC.last_hit_damage ?? 0 } })}</span>
        {#if (activeC.last_hit_attack_total ?? 0) < activeC.ac + 5}
          <span style="color:#2a8a2a">{$_('initiative.label_shield_negate')}</span>
        {:else}
          <span style="color:#8b6914">{$_('initiative.label_shield_still_lands')}</span>
        {/if}
      </div>
    {:else}
      <div class="ca-result" style="color:#8b1a1a;font-size:0.75rem">{$_('initiative.label_no_pending_hit')}</div>
    {/if}
  {/if}
  {#if reactType === 'counterspell'}
    {@const casting = combatants.find(c => c.spell_being_cast)}
    {#if casting}
      <div class="ca-result" style="background:rgba(200,160,60,0.15)">
        <span>{casting.display_name} {$_('initiative.label_is_casting')} {casting.spell_being_cast}</span>
      </div>
    {:else}
      <div class="ca-result" style="color:#8b1a1a;font-size:0.75rem">{$_('initiative.label_no_spell_being_cast')}</div>
    {/if}
  {/if}
  {#if reactType === 'custom'}
    <label class="ca-field">
      <span>{$_('initiative.label_react_label')}</span>
      <input type="text" bind:value={reactLabel} placeholder={$_('initiative.ph_react_label')} />
    </label>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`react:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`react:${activeC.id}`)}>
    <Shield size={12} /> {$_('initiative.btn_use_reaction')}
  </button>
</div>
