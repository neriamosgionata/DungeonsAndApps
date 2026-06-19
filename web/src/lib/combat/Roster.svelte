<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Search, Crown, Shield, Sparkles, Play, X } from '@lucide/svelte';
  import { Encounters } from '$lib/api/resources';
  import EffectBadge from '$lib/components/EffectBadge.svelte';
  import type { Combatant, CombatantEffect, Encounter } from '$lib/types';

  let {
    combatants,
    currentEnc,
    isActiveEncounter,
    isMaster,
    allNpcs,
    effectsFor,
    hpRatio,
    hpColor,
    isInFlight,
    guarded,
    onApplyDamage,
    onSetTemp,
    onGotoTurn,
    onShowEffectPanel,
    onShowStatBlock,
  }: {
    combatants: Combatant[];
    currentEnc: Encounter;
    isActiveEncounter: boolean;
    isMaster: boolean;
    allNpcs: Array<{ id: string; name: string; stats?: Record<string, unknown> }>;
    effectsFor: (c: Combatant) => CombatantEffect[];
    hpRatio: (c: Combatant) => number;
    hpColor: (r: number) => string;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onApplyDamage: (c: Combatant, delta: number) => void;
    onSetTemp: (c: Combatant, val: number) => void;
    onGotoTurn: (i: number) => void;
    onShowEffectPanel: (c: Combatant) => void;
    onShowStatBlock: (c: Combatant) => void;
  } = $props();

  let search = $state('');
  const filtered = $derived(combatants.filter((c) => {
    const q = search.trim().toLowerCase();
    return !q || c.display_name.toLowerCase().includes(q);
  }));

  async function removeCombatant(c: Combatant) {
    if (!confirm($_('initiative.remove_combatant_confirm'))) return;
    await guarded(`combatant:delete:${c.id}`, async () => {
      await Encounters.combatants.delete(c.id as string);
      onGotoTurn(currentEnc.turn_index as number);
    });
  }
</script>

<div class="flex items-center gap-2 mb-3">
  <Search size={14} class="text-neutral-500 shrink-0" />
  <input
    placeholder={$_('initiative.ph_filter_combatants')}
    bind:value={search}
    class="flex-1 max-w-xs rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 text-sm" />
</div>

<section class="roster">
  <div class="roster-head">
    <span class="col-order">{$_('initiative.col_order')}</span>
    <span class="col-name">{$_('initiative.col_name')}</span>
    <span class="col-init">{$_('initiative.col_init')}</span>
    <span class="col-hp">{$_('initiative.col_hp')}</span>
    <span class="col-ac">{$_('initiative.col_ac')}</span>
    <span class="col-actions">{$_('initiative.col_actions')}</span>
  </div>

  {#each filtered as c, i (c.id)}
    {@const globalIndex = combatants.indexOf(c)}
    {@const pending = !c.initiative_rolled}
    {@const activeRow = globalIndex === currentEnc.turn_index && isActiveEncounter && !pending}
    {@const r = hpRatio(c)}
    {@const effs = effectsFor(c)}
    <div class="row {activeRow ? 'row-active' : ''} {pending ? 'row-pending' : ''}">
      <span class="col-order">
        {#if activeRow}<Crown size={14} style="color:#c9a84c;" />{:else}{pending ? '—' : c.turn_order}{/if}
      </span>
      <span class="col-name">
        {#if isMaster && isActiveEncounter && !pending}
          <button onclick={() => onGotoTurn(globalIndex)} class="name-btn">{c.display_name}</button>
        {:else}
          <span class="name-plain">{c.display_name}</span>
        {/if}
        {#if pending}
          <span class="awaiting">{$_('initiative.awaiting_init')}</span>
        {/if}
        {#if isActiveEncounter && !pending}
          <span class="act-indicators">
            <span class="act-ind {c.action_used ? 'used' : ''}" title={$_('initiative.action_action')}>A</span>
            <span class="act-ind {c.bonus_action_used ? 'used' : ''}" title={$_('initiative.action_bonus')}>B</span>
            <span class="act-ind {c.reaction_used ? 'used' : ''}" title={$_('initiative.action_reaction')}>R</span>
            {#if c.legendary_actions_max > 0 && isMaster}
              <span class="act-ind legend" title={$_('initiative.action_legendary')}>{c.legendary_actions_max - c.legendary_actions_used}⚡</span>
            {/if}
          </span>
        {/if}
        {#if effs.length > 0}
          <span class="effect-badges">
            {#each effs.slice(0, 6) as eff (eff.id)}
              <EffectBadge effect={eff} size="sm" onclick={() => onShowEffectPanel(c)} />
            {/each}
            {#if effs.length > 6}<span class="more-effects">+{effs.length - 6}</span>{/if}
          </span>
        {/if}
      </span>
      <span class="col-init">{pending ? '—' : c.initiative}</span>
      <span class="col-hp">
        {#if isMaster && c.ref_type !== 'character'}
          <div class="hp-ctl">
            <button
              type="button"
              disabled={isInFlight(`hp:dmg:${c.id}`)}
              onclick={() => guarded(`hp:dmg:${c.id}`, async () => { onApplyDamage(c, -1); })}
              class="hp-btn hp-dmg"
              title={$_('initiative.col_hp')}>−</button>
            <span class="hp-val" style="color: {hpColor(r)};">
              {c.hp_current}<span class="hp-sep">/{c.hp_max}</span>
              {#if (c.temp_hp as number) > 0}<span class="temp-hp" title={$_('initiative.temp_hp_title')}>+{c.temp_hp}</span>{/if}
            </span>
            <button
              type="button"
              disabled={isInFlight(`hp:heal:${c.id}`)}
              onclick={() => guarded(`hp:heal:${c.id}`, async () => { onApplyDamage(c, 1); })}
              class="hp-btn hp-heal"
              title={$_('initiative.col_hp')}>+</button>
            <label class="temp-wrap" title={$_('initiative.temp_hp_title')}>
              <span class="temp-tag">{$_('initiative.temp_short')}</span>
              <input
                type="number"
                min="0"
                value={(c.temp_hp as number | undefined) ?? 0}
                onchange={(e) => onSetTemp(c, +(e.currentTarget as HTMLInputElement).value)}
                class="temp-input" />
            </label>
          </div>
          <div class="hp-bar"><span style="width: {r * 100}%; background: {hpColor(r)};"></span></div>
        {:else if (c.hp_max as number) > 0}
          <div>
            <span class="hp-val" style="color: {hpColor(r)};">{c.hp_current}<span class="hp-sep">/{c.hp_max}</span>{#if (c.temp_hp as number) > 0}<span class="temp-hp">+{c.temp_hp}</span>{/if}</span>
            <div class="hp-bar"><span style="width: {r * 100}%; background: {hpColor(r)};"></span></div>
          </div>
        {:else}
          <span class="hp-hidden">—</span>
        {/if}
      </span>
      <span class="col-ac">{#if isMaster || (c.ac as number) > 0}<Shield size={11} /> {c.ac}{:else}—{/if}</span>
      <span class="col-actions">
        <button title={$_('initiative.effects')} class="icon-btn" onclick={() => onShowEffectPanel(c)}>
          <Sparkles size={13} />
          {#if effs.length > 0}<span class="action-badge">{effs.length}</span>{/if}
        </button>
        {#if c.npc_id}
          {@const npc = allNpcs.find((n) => n.id === c.npc_id)}
          {#if npc?.stats}
            <button title={$_('initiative.title_stat_block')} class="icon-btn" onclick={() => onShowStatBlock(c)}>
              <Shield size={13} />
            </button>
          {/if}
        {/if}
        {#if isMaster}
          {#if isActiveEncounter}
            <button title={$_('initiative.jump_to_turn')} onclick={() => onGotoTurn(i)} class="icon-btn"><Play size={13} /></button>
          {/if}
          <button
            title={$_('initiative.remove_combatant')}
            class="icon-btn danger"
            disabled={isInFlight(`combatant:delete:${c.id}`)}
            onclick={() => removeCombatant(c)}>
            <X size={14} />
          </button>
        {/if}
      </span>
    </div>
  {/each}
</section>

<style>
  .roster {
    margin-top: 1rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.45rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: 0 4px 10px rgba(0, 0, 0, 0.25);
    overflow: hidden;
  }
  .roster-head, .row {
    display: grid;
    grid-template-columns: 2rem 1fr 3rem 9rem 2.5rem 4rem;
    align-items: center;
    padding: 0.4rem 0.6rem;
    gap: 0.4rem;
  }
  .roster-head {
    background: linear-gradient(180deg, rgba(139, 105, 20, 0.25), transparent);
    color: #6d510f;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.78rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    border-bottom: 1.5px solid #8b6914;
  }
  .row {
    border-top: 1px dashed rgba(139, 105, 20, 0.2);
    color: #2c1810;
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
    content-visibility: auto;
    contain-intrinsic-size: auto 3.2rem;
  }
  .row:first-child { border-top: 0; }
  .row.row-pending { opacity: 0.65; }
  .row.row-active {
    background: linear-gradient(90deg, rgba(201, 168, 76, 0.18), transparent 60%);
    box-shadow: inset 4px 0 0 #c9a84c;
  }
  .col-order { text-align: center; font-family: 'IM Fell English SC', serif; color: #6d510f; font-weight: 800; }
  .col-name { display: flex; align-items: center; gap: 0.4rem; flex-wrap: wrap; }
  .col-init { text-align: center; font-family: 'IM Fell English SC', serif; color: #2c1810; }
  .col-hp { display: flex; flex-direction: column; gap: 0.15rem; }
  .col-ac { text-align: center; font-family: 'Cinzel', serif; }
  .col-actions { display: inline-flex; gap: 0.2rem; justify-content: flex-end; flex-wrap: wrap; }
  .name-btn { background: none; border: 0; color: #2c1810; font: inherit; font-weight: 700; cursor: pointer; text-decoration: underline; }
  .name-plain { color: #2c1810; font-weight: 600; }
  .awaiting { font-size: 0.7rem; color: #b84040; font-style: italic; margin-left: 0.3rem; }
  .act-indicators { display: inline-flex; gap: 0.15rem; }
  .act-ind {
    display: inline-flex; align-items: center; justify-content: center;
    width: 1.1rem; height: 1.1rem;
    border-radius: 9999px;
    background: #6b8a4f; color: #f4e4c1;
    font-family: 'Cinzel', serif; font-weight: 800; font-size: 0.6rem;
  }
  .act-ind.used { background: #888; opacity: 0.4; text-decoration: line-through; }
  .act-ind.legend { background: #c9a84c; color: #1a0f08; }
  .effect-badges { display: inline-flex; gap: 0.15rem; flex-wrap: wrap; }
  .more-effects { font-size: 0.7rem; color: #6d510f; }
  .hp-ctl { display: inline-flex; gap: 0.2rem; align-items: center; }
  .hp-btn { width: 1.4rem; height: 1.4rem; border-radius: 0.2rem; border: 1px solid #8b6914; background: rgba(255, 248, 220, 0.5); color: #2c1810; font-weight: 800; cursor: pointer; }
  .hp-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .hp-btn.hp-dmg:hover { background: rgba(184, 64, 64, 0.3); }
  .hp-btn.hp-heal:hover { background: rgba(64, 184, 64, 0.3); }
  .hp-val { font-family: 'Cinzel', serif; font-weight: 700; font-size: 0.85rem; }
  .hp-sep { color: #6d510f; opacity: 0.6; }
  .temp-hp { color: #6b8a4f; font-weight: 700; font-size: 0.7rem; margin-left: 0.15rem; }
  .temp-wrap { display: inline-flex; align-items: center; gap: 0.2rem; margin-left: 0.3rem; }
  .temp-tag { font-size: 0.65rem; color: #6d510f; }
  .temp-input { width: 2.6rem; padding: 0.1rem 0.2rem; border: 1px solid #8b6914; border-radius: 0.2rem; background: rgba(255, 248, 220, 0.5); color: #2c1810; font-size: 0.75rem; }
  .hp-bar { height: 0.3rem; background: rgba(0, 0, 0, 0.1); border-radius: 0.15rem; overflow: hidden; margin-top: 0.15rem; }
  .hp-bar span { display: block; height: 100%; transition: width 0.2s ease; }
  .hp-hidden { color: #888; }
  .icon-btn { width: 1.5rem; height: 1.5rem; display: inline-flex; align-items: center; justify-content: center; background: transparent; border: 1px solid transparent; border-radius: 0.2rem; color: #2c1810; cursor: pointer; position: relative; }
  .icon-btn:hover { background: rgba(201, 168, 76, 0.2); border-color: #8b6914; }
  .icon-btn:disabled { opacity: 0.4; cursor: not-allowed; }
  .icon-btn.danger:hover { background: rgba(184, 64, 64, 0.2); border-color: #8b1a1a; color: #b84040; }
  .action-badge { position: absolute; top: -0.2rem; right: -0.2rem; background: #c9a84c; color: #1a0f08; border-radius: 9999px; font-size: 0.55rem; font-weight: 800; padding: 0 0.2rem; min-width: 0.8rem; text-align: center; }
  @media (max-width: 639px) {
    .roster-head, .row { grid-template-columns: 2rem 1fr 3rem 8rem 2.5rem 3.2rem; font-size: 0.78rem; }
  }
</style>
