<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant } from '$lib/types';

  const TRIGGER_EVENTS = [
    { id: '', label: 'Manual only' },
    { id: 'target_enters_range', label: 'Target enters range (move)' },
    { id: 'target_attacks', label: 'Target attacks' },
    { id: 'target_casts', label: 'Target casts a spell' },
  ];

  const READY_ACTIONS = [
    { id: 'attack', label: 'Attack' },
    { id: 'cast spell', label: 'Cast Spell' },
    { id: 'dash', label: 'Dash' },
    { id: 'disengage', label: 'Disengage' },
    { id: 'dodge', label: 'Dodge' },
    { id: 'help', label: 'Help' },
  ];

  let {
    activeC,
    combatants,
    readyTrigger = $bindable(''),
    readyTriggerEvent = $bindable(''),
    readyWatchTarget = $bindable(''),
    readyAction = $bindable('attack'),
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    readyTrigger?: string;
    readyTriggerEvent?: string;
    readyWatchTarget?: string;
    readyAction?: string;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_enter_trigger')}</span>
    <input type="text" bind:value={readyTrigger} placeholder={$_('initiative.ph_ready_trigger')} />
  </label>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_auto_trigger')}</span>
      <select bind:value={readyTriggerEvent}>
        {#each TRIGGER_EVENTS as e (e.id)}
          <option value={e.id}>{e.label}</option>
        {/each}
      </select>
    </label>
    {#if readyTriggerEvent}
      <label class="ca-field">
        <span>{$_('initiative.label_watch')}</span>
        <select bind:value={readyWatchTarget}>
          <option value="">Anyone</option>
          {#each combatants as t (t.id)}
            {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
          {/each}
        </select>
      </label>
    {/if}
  </div>
  <label class="ca-field">
    <span>{$_('initiative.action_action')}</span>
    <select bind:value={readyAction}>
      {#each READY_ACTIONS as a (a.id)}
        <option value={a.id}>{a.label}</option>
      {/each}
    </select>
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`ready:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`ready:${activeC.id}`)}>
    {$_('initiative.label_ready_action')}
  </button>
</div>
