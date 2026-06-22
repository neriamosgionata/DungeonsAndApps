<script lang="ts">
  import { _ } from 'svelte-i18n';
  import Modal from './Modal.svelte';
  import type { Combatant, Encounter } from '$lib/types';

  export type CombatEvent = {
    id: string;
    encounter_id: string;
    round: number;
    actor_combatant: string | null;
    target_combatant: string | null;
    action: string;
    delta_hp: number | null;
    note: string | null;
    created_at: string;
  };

  let {
    encounter,
    combatants,
    events,
    loading,
    onClose,
  }: {
    encounter: Encounter;
    combatants: Combatant[];
    events: CombatEvent[];
    loading: boolean;
    onClose: () => void;
  } = $props();

  function combatantName(id: string | null): string {
    if (!id) return '—';
    return combatants.find((c) => c.id === id)?.display_name ?? $_('common.unknown');
  }
</script>

<Modal onClose={onClose} title="{$_('initiative.label_combat_log')} — {encounter.name}" dark>
  {#if loading}
    <p class="text-sm italic" style="color:#8b6355;">{$_('initiative.label_loading')}</p>
  {:else if events.length === 0}
    <p class="text-sm italic" style="color:#8b6355;">{$_('initiative.label_no_events')}</p>
  {:else}
    <div class="flex flex-col gap-1">
      {#each events as ev (ev.id)}
        <div
          class="text-xs p-2 rounded"
          style="background:rgba(44,24,16,0.5); border:1px solid rgba(201,168,76,0.15);"
        >
          <div class="flex items-center gap-2 flex-wrap">
            <span class="font-display font-bold" style="color:#c9a84c;">{$_('initiative.label_round_prefix')}{ev.round}</span>
            <span style="color:#f7e2a5;">{combatantName(ev.actor_combatant)}</span>
            <span style="color:#a6855c;">→</span>
            <span style="color:#f4e4c1;">{ev.action}</span>
            {#if ev.target_combatant}
              <span style="color:#8b6355;">→</span>
              <span style="color:#f7e2a5;">{combatantName(ev.target_combatant)}</span>
            {/if}
            {#if ev.delta_hp !== null && ev.delta_hp !== 0}
              <span
                class="font-bold"
                style="color:{ev.delta_hp < 0 ? '#b84040' : '#40b840'};"
              >
                {ev.delta_hp > 0 ? '+' : ''}{ev.delta_hp} {$_('initiative.label_hp_short')}
              </span>
            {/if}
          </div>
          {#if ev.note}
            <div class="mt-1 text-[11px]" style="color:#8b6355;">{ev.note}</div>
          {/if}
          <div class="mt-1 text-[10px]" style="color:#555;">
            {new Date(ev.created_at).toLocaleString()}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</Modal>
