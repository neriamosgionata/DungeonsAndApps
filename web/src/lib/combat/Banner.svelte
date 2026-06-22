<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Combatant, Encounter } from '$lib/types';
  import { Combatants } from '$lib/api/resources';
  import {
    Swords,
    Crown,
    Hourglass,
    Play,
    SkipBack,
    SkipForward,
    Square,
    Trash2,
  } from '@lucide/svelte';

  export type EncounterDifficulty = {
    difficulty: string;
    adjusted_xp: number;
    total_xp: number;
    thresholds: { deadly: number };
    party_levels?: number[];
    monster_xp?: Array<[string, number, number]>;
  };

  export type FlankingPair = {
    attacker_a_id: string;
    attacker_b_id: string;
    target_id: string;
    attacker_a_name: string;
    attacker_b_name: string;
    target_name: string;
  };

  let {
    encounter,
    combatants,
    isMaster,
    encounterDifficulty,
    flankingPairs,
    pendingCombatants,
    isInFlight,
    onLairAction,
    onLoadDifficulty,
    onLoadFlanking,
    onShowCombatLog,
    onStart,
    onPrev,
    onNext,
    onEnd,
    onRemove,
  }: {
    encounter: Encounter;
    combatants: Combatant[];
    isMaster: boolean;
    encounterDifficulty: EncounterDifficulty | null;
    flankingPairs: FlankingPair[];
    pendingCombatants: Combatant[];
    isInFlight: (key: string) => boolean;
    onLairAction: () => void | Promise<void>;
    onLoadDifficulty: () => void;
    onLoadFlanking: () => void;
    onShowCombatLog: () => void;
    onStart: () => void;
    onPrev: () => void;
    onNext: () => void;
    onEnd: () => void;
    onRemove: () => void;
  } = $props();

  const active = $derived(encounter.status === 'active');
  const total = $derived(combatants.length);
</script>

<div class="banner">
  <div class="banner-title">
    <Swords size={16} />
    <span>{encounter.name}</span>
  </div>
  <div class="banner-meta">
    <span class="meta-chip"><Crown size={12} /> {$_('initiative.round')} <b>{encounter.round}</b></span>
    <span class="meta-chip">
      <Hourglass size={12} />
      {$_('initiative.turn_of', { values: { n: (encounter.turn_index as number) + 1, total } })}
    </span>
    {#if isMaster && active}
      <button
        type="button"
        class="meta-chip lair-chip {encounter.lair_action_used ? 'used' : ''}"
        onclick={onLairAction}
        disabled={isInFlight('lair:action')}>
        🏰 {$_('initiative.action_lair')}
      </button>
    {/if}
    {#if isMaster}
      <button type="button" class="meta-chip diff-chip" onclick={onLoadDifficulty}>
        ⚖️ Difficulty
      </button>
      <button type="button" class="meta-chip flank-chip" onclick={onLoadFlanking}>
        ⚔️ Flank
      </button>
    {/if}
    <button type="button" class="meta-chip log-chip" onclick={onShowCombatLog}>
      📜 Combat Log
    </button>
  </div>
  {#if encounterDifficulty}
    <div class="diff-panel">
      <span class="diff-label {encounterDifficulty.difficulty}">
        {encounterDifficulty.difficulty.toUpperCase()}
      </span>
      <span>{$_('initiative.label_adjusted_xp', { values: { x: encounterDifficulty.adjusted_xp.toLocaleString(), d: encounterDifficulty.thresholds.deadly.toLocaleString() } })}</span>
      <span>{$_('initiative.label_total_xp', { values: { x: encounterDifficulty.total_xp.toLocaleString(), n: encounterDifficulty.party_levels?.length ?? 0 } })}</span>
      {#if (encounterDifficulty.monster_xp?.length ?? 0) > 0}
        <details class="diff-details">
          <summary>Monster XP ({encounterDifficulty.monster_xp?.length ?? 0} entries)</summary>
          {#each (encounterDifficulty.monster_xp ?? []) as [name, xp, count]}
            <span class="diff-entry">{name}: {xp.toLocaleString()} XP {#if count > 1}(×{count}){/if}</span>
          {/each}
        </details>
      {/if}
    </div>
  {/if}
  {#if flankingPairs.length > 0}
    <div class="flank-panel">
      <span class="flank-title">⚔️ Flanking:</span>
      {#each flankingPairs as p (p.attacker_a_id + '-' + p.attacker_b_id + '-' + p.target_id)}
        <span class="flank-pair">{p.attacker_a_name} + {p.attacker_b_name} → {p.target_name}</span>
      {/each}
    </div>
  {/if}
  {#if isMaster}
    <div class="banner-actions">
      {#if encounter.status === 'planned'}
        <button
          onclick={onStart}
          class="btn btn-start"
          disabled={pendingCombatants.length > 0 || isInFlight('encounter:start')}
          title={pendingCombatants.length > 0
            ? pendingCombatants.map((c) => c.display_name).join(', ')
            : undefined}>
          <Play size={14} /> {$_('initiative.start')}
          {#if pendingCombatants.length > 0}
            <span class="start-pending">({pendingCombatants.length})</span>
          {/if}
        </button>
      {:else if active}
        <button onclick={onPrev} class="btn btn-ghost" disabled={isInFlight('encounter:prev')} title={$_('initiative.prev_turn_title')}>
          <SkipBack size={14} /> {$_('initiative.prev')}
        </button>
        <button onclick={onNext} class="btn btn-next" disabled={isInFlight('encounter:next')} title={$_('initiative.next_turn_title')}>
          <SkipForward size={14} /> {$_('initiative.next')}
        </button>
        <button onclick={onEnd} class="btn btn-end" disabled={isInFlight('encounter:end')}>
          <Square size={14} /> {$_('initiative.end')}
        </button>
      {/if}
      <button onclick={onRemove} class="btn btn-danger" disabled={isInFlight('encounter:remove')} title={$_('initiative.delete')}>
        <Trash2 size={14} />
      </button>
    </div>
  {/if}
</div>

<style>
  .banner {
    display: flex; align-items: center; gap: 0.75rem; flex-wrap: wrap;
    padding: 0.75rem 1rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.25);
    color: #2c1810;
    margin-bottom: 0.75rem;
  }
  .banner-title {
    display: inline-flex; align-items: center; gap: 0.5rem;
    font-family: 'IM Fell English SC', serif;
    font-size: 1.1rem;
    font-weight: 700;
    color: #2c1810;
  }
  .banner-title :global(svg) { color: #c9a84c; }
  .banner-meta { display: inline-flex; align-items: center; gap: 0.5rem; flex-wrap: wrap; }
  .meta-chip {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.2rem 0.5rem;
    background: rgba(255, 248, 220, 0.5);
    border: 1px solid rgba(139, 105, 20, 0.4);
    border-radius: 0.3rem;
    font-size: 0.75rem;
    color: #2c1810;
    cursor: pointer;
    font-family: 'Cinzel', serif;
  }
  .meta-chip:disabled { opacity: 0.5; cursor: not-allowed; }
  .lair-chip.used { opacity: 0.4; }
  .banner-actions { margin-left: auto; display: inline-flex; gap: 0.4rem; flex-wrap: wrap; }
  .btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.3rem 0.7rem;
    border-radius: 0.3rem;
    border: 1px solid;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 0.75rem;
    cursor: pointer;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-start { background: linear-gradient(180deg, #c9a84c, #6d510f); color: #1a0f08; border-color: #4e3909; }
  .btn-next { background: linear-gradient(180deg, #c9a84c, #6d510f); color: #1a0f08; border-color: #4e3909; }
  .btn-prev, .btn-ghost { background: rgba(255, 248, 220, 0.5); color: #2c1810; border-color: #8b6914; }
  .btn-end { background: rgba(139, 0, 0, 0.85); color: #f4e4c1; border-color: #4e0a0a; }
  .btn-danger { background: rgba(139, 0, 0, 0.85); color: #f4e4c1; border-color: #4e0a0a; padding: 0.3rem 0.5rem; }
  .start-pending { color: #b84040; font-weight: 800; margin-left: 0.2rem; }
  .diff-panel, .flank-panel {
    width: 100%;
    display: flex; align-items: center; gap: 0.6rem; flex-wrap: wrap;
    padding: 0.4rem 0.6rem;
    background: rgba(255, 248, 220, 0.4);
    border: 1px dashed rgba(139, 105, 20, 0.4);
    border-radius: 0.3rem;
    font-size: 0.78rem;
  }
  .diff-label { font-weight: 700; padding: 0.1rem 0.4rem; border-radius: 0.2rem; }
  .diff-label.easy { background: rgba(40, 160, 80, 0.2); color: #1f5d2a; }
  .diff-label.medium { background: rgba(201, 168, 76, 0.3); color: #6d510f; }
  .diff-label.hard { background: rgba(200, 100, 50, 0.25); color: #8b3a1a; }
  .diff-label.deadly { background: rgba(139, 0, 0, 0.25); color: #6b0a0a; }
  .diff-details { width: 100%; }
  .diff-entry { display: inline-block; margin-right: 0.5rem; }
  .flank-title { font-weight: 700; }
  .flank-pair { color: #6d510f; font-family: 'Special Elite', monospace; font-size: 0.75rem; }
</style>
