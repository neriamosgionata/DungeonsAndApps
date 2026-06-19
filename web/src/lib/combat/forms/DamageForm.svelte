<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    dmgAmount = $bindable(0),
    damageType = $bindable('slashing'),
    dmgResult = null,
    isInFlight,
    guarded,
    onApplyDamage,
    onApplyHeal,
  }: {
    activeC: Combatant;
    dmgAmount?: number;
    damageType?: string;
    dmgResult?: { damage_applied: number; damage_resisted?: boolean; concentration_broken?: boolean } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onApplyDamage: (c: Combatant) => void | Promise<void>;
    onApplyHeal: (c: Combatant) => void | Promise<void>;
  } = $props();

  const DAMAGE_TYPES = [
    'slashing', 'piercing', 'bludgeoning', 'fire', 'cold', 'lightning',
    'thunder', 'acid', 'poison', 'necrotic', 'radiant', 'psychic', 'force',
  ];
</script>

<div class="ca-form">
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_amount')}</span>
      <input type="number" bind:value={dmgAmount} min="0" />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.label_dmg_type')}</span>
      <select bind:value={damageType}>
        {#each DAMAGE_TYPES as t (t)}
          <option value={t}>{$_('initiative.damage_type_' + t)}</option>
        {/each}
      </select>
    </label>
  </div>
  <div class="ca-row">
    <button
      type="button"
      class="ca-submit dmg-btn"
      onclick={() => guarded(`damage:${activeC.id}`, async () => { await onApplyDamage(activeC); })}
      disabled={isInFlight(`damage:${activeC.id}`)}>
      {$_('initiative.label_apply_damage')}
    </button>
    <button
      type="button"
      class="ca-submit heal-btn"
      onclick={() => guarded(`heal:${activeC.id}`, async () => { await onApplyHeal(activeC); })}
      disabled={isInFlight(`heal:${activeC.id}`)}>
      {$_('initiative.label_apply_healing')}
    </button>
  </div>
  {#if dmgResult}
    <div class="ca-result">
      <span>{$_('initiative.label_applied_dmg', { values: { amount: Math.abs(dmgResult.damage_applied), kind: $_('initiative.label_kind_' + (dmgResult.damage_applied < 0 ? 'healing' : 'damage')) } })}</span>
      {#if dmgResult.damage_resisted}<span>({$_('initiative.label_damage_resisted')})</span>{/if}
      {#if dmgResult.concentration_broken}<span class="ca-conc">{$_('initiative.label_conc_broken')}</span>{/if}
    </div>
  {/if}
</div>

<style>
  .dmg-btn { background: #8b2020; border-color: #b84040; }
  .dmg-btn:hover { background: #b84040; }
  .heal-btn { background: #206b20; border-color: #40b840; }
  .heal-btn:hover { background: #40b840; }
</style>
