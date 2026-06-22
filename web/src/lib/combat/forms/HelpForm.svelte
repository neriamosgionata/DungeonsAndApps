<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Hand } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  let {
    activeC,
    combatants,
    helpTarget = $bindable(''),
    onHelp,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    helpTarget?: string;
    onHelp: (c: Combatant, targetId: string) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_target')}</span>
    <select bind:value={helpTarget}>
      <option value="">{$_('initiative.ph_select_ally')}</option>
      {#each combatants as t (t.id)}
        {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
      {/each}
    </select>
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => { if (helpTarget) { onHelp(activeC, helpTarget); helpTarget = ''; } }}>
    <Hand size={12} /> {$_('initiative.btn_help_ally')}
  </button>
</div>
