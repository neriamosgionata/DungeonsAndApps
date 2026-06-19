<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Swords } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  const DAMAGE_TYPES = [
    'slashing', 'piercing', 'bludgeoning', 'fire', 'cold', 'lightning',
    'thunder', 'acid', 'poison', 'necrotic', 'radiant', 'psychic', 'force',
  ];

  export type MultiattackTarget = {
    target_id: string;
    attack_expr: string;
    damage_expr: string;
    damage_type: string;
    weapon_id?: string;
  };

  export type MultiattackResult = {
    targets_hit: number;
    results: Array<unknown>;
    total_damage: number;
  };

  let {
    activeC,
    combatants,
    multiattackParseTarget = $bindable(''),
    attackTarget = $bindable(''),
    attackExpr = $bindable(''),
    damageExpr = $bindable(''),
    damageType = $bindable('slashing'),
    attackWeaponId = $bindable(''),
    multiattackTargets = $bindable<MultiattackTarget[]>([]),
    multiattackResult = null,
    isInFlight,
    guarded,
    onParse,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    multiattackParseTarget?: string;
    attackTarget?: string;
    attackExpr?: string;
    damageExpr?: string;
    damageType?: string;
    attackWeaponId?: string;
    multiattackTargets?: MultiattackTarget[];
    multiattackResult?: MultiattackResult | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onParse: (c: Combatant) => void | Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();

  function addTarget() {
    if (!attackTarget) return;
    multiattackTargets = [
      ...multiattackTargets,
      {
        target_id: attackTarget,
        attack_expr: attackExpr,
        damage_expr: damageExpr,
        damage_type: damageType,
        weapon_id: attackWeaponId || undefined,
      },
    ];
    attackTarget = '';
    attackExpr = '';
    damageExpr = '';
  }

  function removeTarget(i: number) {
    multiattackTargets = multiattackTargets.filter((_, idx) => idx !== i);
  }
</script>

<div class="ca-form">
  {#if activeC.npc_id}
    <div class="ca-row parse-row">
      <label class="ca-field">
        <span>{$_('initiative.label_parse_npc_multiattack')}</span>
        <select bind:value={multiattackParseTarget}>
          <option value="">Select target for parsed attacks…</option>
          {#each combatants as t (t.id)}
            {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
          {/each}
        </select>
      </label>
      <button
        type="button"
        class="ca-btn"
        disabled={isInFlight(`parse:ma:${activeC.id}`)}
        onclick={() => guarded(`parse:ma:${activeC.id}`, async () => { await onParse(activeC); })}
        title={$_('initiative.title_parse_multiattack')}>
        {$_('initiative.label_parse')}
      </button>
    </div>
    <hr class="ma-hr" />
  {/if}
  <div class="ca-row add-row">
    <label class="ca-field">
      <span>{$_('initiative.label_add_target')}</span>
      <select bind:value={attackTarget}>
        <option value="">Select…</option>
        {#each combatants as t (t.id)}
          {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
        {/each}
      </select>
    </label>
    <label class="ca-field">
      <span>Atk</span>
      <input type="text" bind:value={attackExpr} placeholder="1d20+7" />
    </label>
    <label class="ca-field">
      <span>Dmg</span>
      <input type="text" bind:value={damageExpr} placeholder="1d8+4" />
    </label>
    <label class="ca-field">
      <span>Type</span>
      <select bind:value={damageType}>
        {#each DAMAGE_TYPES as t}
          <option value={t}>{t}</option>
        {/each}
      </select>
    </label>
    <button type="button" class="ca-btn" onclick={addTarget}>+ Add</button>
  </div>
  {#if multiattackTargets.length > 0}
    <div class="targets-list">
      {#each multiattackTargets as mt, i (i)}
        <span class="target-chip">
          {combatants.find((c) => c.id === mt.target_id)?.display_name ?? mt.target_id}:
          {mt.attack_expr} / {mt.damage_expr} {mt.damage_type}
          <button type="button" class="remove-btn" onclick={() => removeTarget(i)}>✕</button>
        </span>
      {/each}
    </div>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`multiattack:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`multiattack:${activeC.id}`)}>
    <Swords size={12} /> Roll Multiattack
  </button>
  {#if multiattackResult}
    <div class="ca-result hit">
      <span>Hit {multiattackResult.targets_hit}/{multiattackResult.results.length} — {multiattackResult.total_damage} total dmg</span>
    </div>
  {/if}
</div>

<style>
  .parse-row { align-items: flex-end; }
  .ma-hr { border-color: #3a2313; margin: 0.25rem 0; }
  .add-row { align-items: flex-end; }
  .targets-list { font-size: 0.7rem; color: #a6855c; margin-bottom: 0.25rem; }
  .target-chip {
    display: inline-flex; align-items: center; gap: 0.2rem;
    margin-right: 0.5rem;
  }
  .remove-btn { font-size: 0.6rem; color: #a93535; }
</style>
