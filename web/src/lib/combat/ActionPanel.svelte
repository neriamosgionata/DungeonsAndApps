<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Combatants } from '$lib/api/resources';
  import { Crown, Heart, Shield, Dice5, Dices, Hand } from '@lucide/svelte';
  import type { Combatant, ComputedStats } from '$lib/types';

  export type DeathSaveResult = {
    successes_after: number;
    failures_after: number;
    stabilized: boolean;
    died: boolean;
    nat20: boolean;
    nat1: boolean;
    passed: boolean;
  };

  let {
    activeC,
    isMaster,
    canToggle,
    speed,
    activeComputedStats,
    deathSaveResult,
    isInFlight,
    guarded,
    onLoadList,
    onDeathSave,
  }: {
    activeC: Combatant;
    isMaster: boolean;
    canToggle: boolean;
    speed: number;
    activeComputedStats: ComputedStats | null;
    deathSaveResult: DeathSaveResult | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onLoadList: () => void | Promise<void>;
    onDeathSave: (c: Combatant) => void | Promise<void>;
  } = $props();
</script>

<div class="spotlight">
  <div class="spot-crown"><Crown size={18} style="color:#c9a84c;" /></div>
  <div class="spot-body">
    <div class="spot-title">
      {$_('initiative.active_turn').replace('{{name}}', activeC.display_name as string)}
    </div>
    <div class="spot-stats">
      <span><Dice5 size={11} /> {activeC.initiative}</span>
      {#if isMaster || (activeC.hp_max as number) > 0}
        <span class="sep">·</span>
        <span><Heart size={11} /> {activeC.hp_current}/{activeC.hp_max}{(activeC.temp_hp as number) > 0 ? ` (+${activeC.temp_hp})` : ''}</span>
      {/if}
      {#if isMaster || (activeC.ac as number) > 0}
        <span class="sep">·</span>
        <span><Shield size={11} />
          {#if activeComputedStats && activeComputedStats.ac !== activeC.ac}
            {activeC.ac}→{activeComputedStats.ac}
          {:else}
            {activeC.ac}
          {/if}
        </span>
      {/if}
      {#if activeComputedStats}
        {#if activeComputedStats.attack_advantage}<span class="stat-badge adv">Adv</span>{/if}
        {#if activeComputedStats.attack_disadvantage}<span class="stat-badge dis">Dis</span>{/if}
        {#if activeComputedStats.save_advantage}<span class="stat-badge sadv">{$_('initiative.badge_sadv')}</span>{/if}
        {#if activeComputedStats.save_disadvantage}<span class="stat-badge sdis">{$_('initiative.badge_sdis')}</span>{/if}
        {#if activeComputedStats.speed_halved}<span class="stat-badge slow">{$_('initiative.badge_slow')}</span>{/if}
        {#if activeComputedStats.incapacitated}<span class="stat-badge incap">{$_('initiative.badge_incap')}</span>{/if}
        {#if activeComputedStats.resistances.length > 0}
          <span class="stat-badge res" title={activeComputedStats.resistances.join(', ')}>{$_('initiative.badge_res')}</span>
        {/if}
        {#if activeComputedStats.immunities.length > 0}
          <span class="stat-badge imm" title={activeComputedStats.immunities.join(', ')}>{$_('initiative.badge_imm')}</span>
        {/if}
        {#if activeComputedStats.exhaustion > 0}
          <span class="stat-badge exhaust" title={$_('initiative.exhaustion', { values: { n: activeComputedStats.exhaustion } })}>Ex {activeComputedStats.exhaustion}</span>
        {/if}
        {#if activeComputedStats.passive_scores.length > 0}
          {@const pp = activeComputedStats.passive_scores.find(([s]) => s === 'perception')}
          {#if pp}<span class="stat-badge pp" title={$_('initiative.passive_perception', { values: { n: pp[1] } })}>PP {pp[1]}</span>{/if}
        {/if}
      {/if}
    </div>
    <div class="action-chips">
      <button
        type="button"
        class="act-chip {activeC.action_used ? 'used' : ''}"
        onclick={() => canToggle && guarded(`useAction:action:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'action'); await onLoadList(); })}
        disabled={!canToggle || isInFlight(`useAction:action:${activeC.id}`)}>
        ⚔️ {$_('initiative.action_action')}
      </button>
      <button
        type="button"
        class="act-chip {activeC.bonus_action_used ? 'used' : ''}"
        onclick={() => canToggle && guarded(`useAction:ba:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'bonus_action'); await onLoadList(); })}
        disabled={!canToggle || isInFlight(`useAction:ba:${activeC.id}`)}>
        ⚡ {$_('initiative.action_bonus')}
      </button>
      <button
        type="button"
        class="act-chip {activeC.reaction_used ? 'used' : ''}"
        onclick={() => canToggle && guarded(`useAction:reaction:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'reaction'); await onLoadList(); })}
        disabled={!canToggle || isInFlight(`useAction:reaction:${activeC.id}`)}>
        ↩️ {$_('initiative.action_reaction')}
      </button>
      <span class="act-chip move-chip">👣 {activeC.movement_used_ft}/{speed}ft</span>
      {#if activeC.legendary_actions_max > 0}
        <span class="legendary-dots" title={$_('initiative.action_legendary')}>
          {#each Array(activeC.legendary_actions_max) as _, i (i)}
            <button
              type="button"
              class="ldot {i < activeC.legendary_actions_used ? 'spent' : ''}"
              onclick={() => isMaster && guarded(`legendary:${activeC.id}:${i}`, async () => { await Combatants.legendaryAction(activeC.id as string); await onLoadList(); })}
              disabled={!isMaster || isInFlight(`legendary:${activeC.id}:${i}`)}>⚡</button>
          {/each}
        </span>
      {/if}
      {#if activeC.legendary_resistances_max > 0}
        <button
          type="button"
          class="act-chip lr-chip"
          onclick={() => isMaster && guarded(`lr:${activeC.id}`, async () => { await Combatants.useAction(activeC.id as string, 'legendary_resistance'); await onLoadList(); })}
          disabled={!isMaster || isInFlight(`lr:${activeC.id}`)}>
          🛡️ LR: {activeC.legendary_resistances_max - activeC.legendary_resistances_used}/{activeC.legendary_resistances_max}
        </button>
      {/if}
    </div>
    {#if activeC.hp_current <= 0 && activeC.hp_max > 0}
      <div class="death-save-banner">
        <span class="ds-title">💀 {activeC.display_name} {$_('initiative.ds_title_dying').replace('{{name}}', '')}</span>
        <div class="ds-track">
          <span>{$_('initiative.ds_successes')}</span>
          {#each [1, 2, 3] as i}
            <span class="ds-dot {deathSaveResult ? (deathSaveResult.successes_after >= i ? 'ds-filled' : '') : ''}">●</span>
          {/each}
          <span>{$_('initiative.ds_failures')}</span>
          {#each [1, 2, 3] as i}
            <span class="ds-dot ds-fail {deathSaveResult ? (deathSaveResult.failures_after >= i ? 'ds-filled' : '') : ''}">●</span>
          {/each}
        </div>
        <button
          type="button"
          class="ca-submit"
          onclick={() => guarded(`deathsave:${activeC.id}`, async () => { await onDeathSave(activeC); })}
          disabled={isInFlight(`deathsave:${activeC.id}`)}>
          <Dices size={14} /> {$_('initiative.ds_roll')}
        </button>
        {#if deathSaveResult}
          <div class="ca-result {deathSaveResult.stabilized ? 'hit' : deathSaveResult.died ? 'miss' : ''}">
            {#if deathSaveResult.nat20}
              <span>{$_('initiative.ds_nat20')}</span>
            {:else if deathSaveResult.nat1}
              <span>{$_('initiative.ds_nat1').replace('{{f}}', String(deathSaveResult.failures_after))}</span>
            {:else if deathSaveResult.passed}
              <span>{$_('initiative.ds_success')} ({deathSaveResult.successes_after}/3)</span>
            {:else}
              <span>{$_('initiative.ds_failure')} ({deathSaveResult.failures_after}/3)</span>
            {/if}
            {#if deathSaveResult.stabilized}<span>{$_('initiative.ds_stabilized')}</span>{/if}
            {#if deathSaveResult.died}<span>{$_('initiative.ds_died')}</span>{/if}
          </div>
        {/if}
      </div>
    {/if}
  </div>
</div>

<style>
  .spotlight {
    display: flex; align-items: center; gap: 0.75rem;
    margin-top: 0.85rem;
    padding: 0.75rem 1rem;
    border: 2px solid #c9a84c;
    border-radius: 0.45rem;
    background:
      radial-gradient(circle at 20% 30%, rgba(201, 168, 76, 0.35) 0%, transparent 60%),
      linear-gradient(180deg, rgba(244, 228, 193, 0.1), transparent 55%),
      #241810;
    color: #f4e4c1;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  }
  .spot-crown { flex-shrink: 0; }
  .spot-body { flex: 1; display: flex; flex-direction: column; gap: 0.3rem; }
  .spot-title {
    font-family: 'IM Fell English SC', serif;
    font-size: 1.1rem;
    color: #f7e2a5;
  }
  .spot-stats { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; font-size: 0.85rem; }
  .sep { opacity: 0.5; }
  .stat-badge {
    display: inline-block;
    padding: 0.05rem 0.35rem;
    font-size: 0.65rem;
    border-radius: 0.2rem;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.05em;
  }
  .stat-badge.adv { background: rgba(40, 160, 80, 0.3); color: #90ee90; }
  .stat-badge.dis { background: rgba(200, 80, 80, 0.3); color: #ff9090; }
  .stat-badge.sadv { background: rgba(74, 124, 89, 0.3); color: #b0e0b0; }
  .stat-badge.sdis { background: rgba(180, 100, 100, 0.3); color: #f0a0a0; }
  .stat-badge.slow { background: rgba(180, 180, 100, 0.3); color: #f0e090; }
  .stat-badge.incap { background: rgba(80, 80, 80, 0.3); color: #a0a0a0; }
  .stat-badge.res { background: rgba(74, 124, 89, 0.3); color: #b0e0b0; }
  .stat-badge.imm { background: rgba(139, 105, 20, 0.4); color: #f0c060; }
  .stat-badge.exhaust { background: rgba(139, 26, 26, 0.3); color: #ff9090; }
  .stat-badge.pp { background: rgba(74, 124, 89, 0.3); color: #b0e0b0; }
  .action-chips { display: flex; gap: 0.3rem; flex-wrap: wrap; margin-top: 0.2rem; }
  .act-chip {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.2rem 0.5rem;
    border-radius: 9999px;
    background: rgba(74, 124, 89, 0.25);
    color: #b0e0b0;
    border: 1px solid rgba(74, 124, 89, 0.4);
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    font-weight: 700;
    cursor: pointer;
  }
  .act-chip.used { opacity: 0.4; text-decoration: line-through; }
  .act-chip:disabled { opacity: 0.4; cursor: not-allowed; }
  .act-chip.move-chip { cursor: default; }
  .act-chip.lr-chip { background: rgba(139, 105, 20, 0.3); color: #f0c060; border-color: rgba(201, 168, 76, 0.5); }
  .legendary-dots { display: inline-flex; gap: 0.15rem; }
  .ldot {
    width: 1.4rem; height: 1.4rem;
    display: inline-flex; align-items: center; justify-content: center;
    background: linear-gradient(180deg, #c9a84c, #6d510f);
    color: #1a0f08;
    border: 1px solid #4e3909;
    border-radius: 9999px;
    font-size: 0.8rem;
    cursor: pointer;
  }
  .ldot.spent { opacity: 0.3; }
  .ldot:disabled { opacity: 0.3; cursor: not-allowed; }
  .death-save-banner {
    margin-top: 0.5rem;
    padding: 0.5rem 0.7rem;
    background: rgba(139, 26, 26, 0.15);
    border: 1px dashed rgba(139, 26, 26, 0.5);
    border-radius: 0.3rem;
  }
  .ds-title { color: #ff9090; font-weight: 700; font-family: 'IM Fell English SC', serif; }
  .ds-track { display: flex; align-items: center; gap: 0.3rem; margin: 0.3rem 0; color: #f4e4c1; font-size: 0.75rem; }
  .ds-dot { color: rgba(255, 255, 255, 0.2); font-size: 1rem; }
  .ds-dot.ds-filled { color: #ff9090; }
  .ds-dot.ds-fail.ds-filled { color: #ff5050; }
  .ca-submit {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.3rem 0.7rem;
    border-radius: 0.3rem;
    background: linear-gradient(180deg, #c9a84c, #6d510f);
    color: #1a0f08;
    border: 1px solid #4e3909;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    cursor: pointer;
  }
  .ca-submit:disabled { opacity: 0.5; cursor: not-allowed; }
  .ca-result { font-size: 0.75rem; padding: 0.3rem; margin-top: 0.3rem; }
  .ca-result.hit { color: #90ee90; }
  .ca-result.miss { color: #ff9090; }
</style>
