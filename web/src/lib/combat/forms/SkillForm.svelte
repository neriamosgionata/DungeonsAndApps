<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Brain } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  const SKILLS = [
    { id: 'acrobatics', label: 'Acrobatics', ability: 'DEX' },
    { id: 'animal_handling', label: 'Animal Handling', ability: 'WIS' },
    { id: 'arcana', label: 'Arcana', ability: 'INT' },
    { id: 'athletics', label: 'Athletics', ability: 'STR' },
    { id: 'deception', label: 'Deception', ability: 'CHA' },
    { id: 'history', label: 'History', ability: 'INT' },
    { id: 'insight', label: 'Insight', ability: 'WIS' },
    { id: 'intimidation', label: 'Intimidation', ability: 'CHA' },
    { id: 'investigation', label: 'Investigation', ability: 'INT' },
    { id: 'medicine', label: 'Medicine', ability: 'WIS' },
    { id: 'nature', label: 'Nature', ability: 'INT' },
    { id: 'perception', label: 'Perception', ability: 'WIS' },
    { id: 'performance', label: 'Performance', ability: 'CHA' },
    { id: 'persuasion', label: 'Persuasion', ability: 'CHA' },
    { id: 'religion', label: 'Religion', ability: 'INT' },
    { id: 'sleight_of_hand', label: 'Sleight of Hand', ability: 'DEX' },
    { id: 'stealth', label: 'Stealth', ability: 'DEX' },
    { id: 'survival', label: 'Survival', ability: 'WIS' },
  ];

  let {
    activeC,
    skillName = $bindable('acrobatics'),
    skillDc = $bindable(10),
    skillAdv = $bindable(false),
    skillDis = $bindable(false),
    skillResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    skillName?: string;
    skillDc?: number;
    skillAdv?: boolean;
    skillDis?: boolean;
    skillResult?: { skill: string; total: number; natural_roll: number; passed: boolean | null } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="ca-form">
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_skill')}</span>
      <select bind:value={skillName}>
        {#each SKILLS as s (s.id)}
          <option value={s.id}>{s.label} ({s.ability})</option>
        {/each}
      </select>
    </label>
    <label class="ca-field"><span>DC</span><input type="number" bind:value={skillDc} min="1" max="40" /></label>
    <label class="ca-check"><input type="checkbox" bind:checked={skillAdv} /> Adv</label>
    <label class="ca-check"><input type="checkbox" bind:checked={skillDis} /> Dis</label>
  </div>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`skill:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`skill:${activeC.id}`)}>
    <Brain size={12} /> Roll Skill Check
  </button>
  {#if skillResult}
    <div class="ca-result {skillResult.passed === true ? 'hit' : skillResult.passed === false ? 'miss' : ''}">
      <span>{$_('initiative.label_skill_result', { values: { skill: skillResult.skill, total: skillResult.total, rolled: skillResult.natural_roll } })}</span>
      {#if skillResult.passed === true}<span>{$_('initiative.label_passed')}</span>{:else if skillResult.passed === false}<span>{$_('initiative.label_failed')}</span>{/if}
    </div>
  {/if}
</div>
