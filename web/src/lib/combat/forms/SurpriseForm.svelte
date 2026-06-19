<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Brain, Sparkles } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  let {
    combatants,
    surprisedCombatantIds = $bindable<string[]>([]),
    surpriseAutoResult = null,
    isInFlight,
    guarded,
    onApplyRound,
    onApplyAuto,
  }: {
    combatants: Combatant[];
    surprisedCombatantIds?: string[];
    surpriseAutoResult?: {
      stealth_rolls: Array<{ name: string; stealth_total: number; natural: number }>;
      perceptions: Array<{ name: string; passive_perception: number; surprised: boolean }>;
    } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onApplyRound: () => void | Promise<void>;
    onApplyAuto: () => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_surprised_combatants')}</span>
    <select multiple bind:value={surprisedCombatantIds} size={4}>
      {#each combatants as t (t.id)}
        <option value={t.id}>{t.display_name}</option>
      {/each}
    </select>
  </label>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded('surprise:round', async () => { await onApplyRound(); })}
    disabled={isInFlight('surprise:round')}>
    <Brain size={12} /> Apply Surprise Round
  </button>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded('surprise:auto', async () => { await onApplyAuto(); })}
    disabled={isInFlight('surprise:auto')}>
    <Sparkles size={12} /> Auto Surprise (Stealth vs PP)
  </button>
  {#if surpriseAutoResult}
    <div class="ca-result">
      <span>{$_('initiative.label_stealth_rolls')}</span>
      {#each surpriseAutoResult.stealth_rolls as sr}
        <span>{sr.name}: {sr.stealth_total} (nat {sr.natural})</span>
      {/each}
      <span>vs PP:</span>
      {#each surpriseAutoResult.perceptions as p}
        <span>{p.name}: PP {p.passive_perception} → {p.surprised ? 'SURPRISED' : 'alert'}</span>
      {/each}
    </div>
  {/if}
</div>
