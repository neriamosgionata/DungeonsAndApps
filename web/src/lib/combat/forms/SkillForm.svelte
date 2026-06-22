<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Brain } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  const SKILLS = [
    { id: 'acrobatics', labelKey: 'skill_acrobatics', ability: 'DEX' },
    { id: 'animal_handling', labelKey: 'skill_animal_handling', ability: 'WIS' },
    { id: 'arcana', labelKey: 'skill_arcana', ability: 'INT' },
    { id: 'athletics', labelKey: 'skill_athletics', ability: 'STR' },
    { id: 'deception', labelKey: 'skill_deception', ability: 'CHA' },
    { id: 'history', labelKey: 'skill_history', ability: 'INT' },
    { id: 'insight', labelKey: 'skill_insight', ability: 'WIS' },
    { id: 'intimidation', labelKey: 'skill_intimidation', ability: 'CHA' },
    { id: 'investigation', labelKey: 'skill_investigation', ability: 'INT' },
    { id: 'medicine', labelKey: 'skill_medicine', ability: 'WIS' },
    { id: 'nature', labelKey: 'skill_nature', ability: 'INT' },
    { id: 'perception', labelKey: 'skill_perception', ability: 'WIS' },
    { id: 'performance', labelKey: 'skill_performance', ability: 'CHA' },
    { id: 'persuasion', labelKey: 'skill_persuasion', ability: 'CHA' },
    { id: 'religion', labelKey: 'skill_religion', ability: 'INT' },
    { id: 'sleight_of_hand', labelKey: 'skill_sleight_of_hand', ability: 'DEX' },
    { id: 'stealth', labelKey: 'skill_stealth', ability: 'DEX' },
    { id: 'survival', labelKey: 'skill_survival', ability: 'WIS' },
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
          <option value={s.id}>{$_(`initiative.${s.labelKey}`)} ({$_(`initiative.ability_${s.ability.toLowerCase()}`)})</option>
        {/each}
      </select>
    </label>
    <label class="ca-field"><span>{$_('initiative.label_dc')}</span><input type="number" bind:value={skillDc} min="1" max="40" /></label>
    <label class="ca-check"><input type="checkbox" bind:checked={skillAdv} /> {$_('initiative.label_adv')}</label>
    <label class="ca-check"><input type="checkbox" bind:checked={skillDis} /> {$_('initiative.label_dis')}</label>
  </div>
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`skill:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`skill:${activeC.id}`)}>
    <Brain size={12} /> {$_('initiative.btn_roll_skill_check')}
  </button>
  {#if skillResult}
    <div class="ca-result {skillResult.passed === true ? 'hit' : skillResult.passed === false ? 'miss' : ''}">
      <span>{$_('initiative.label_skill_result', { values: { skill: skillResult.skill, total: skillResult.total, rolled: skillResult.natural_roll } })}</span>
      {#if skillResult.passed === true}<span>{$_('initiative.label_passed')}</span>{:else if skillResult.passed === false}<span>{$_('initiative.label_failed')}</span>{/if}
    </div>
  {/if}
</div>
