<script lang="ts">
  import type { Combatant, CombatantEffect, Spell } from '$lib/types';
  import { Effects, Spells } from '$lib/api/resources';
  import { _ } from 'svelte-i18n';
  import EffectBadge from './EffectBadge.svelte';
  import { X, Plus, Sparkles, Trash2 } from '@lucide/svelte';

  interface Props {
    combatant: Combatant;
    effects: CombatantEffect[];
    encounterId: string;
    isMaster: boolean;
    isMe: boolean; // player owns this character-linked combatant
    onchange?: () => void;
    onclose?: () => void;
  }

  let { combatant, effects, encounterId, isMaster, isMe, onchange, onclose }: Props = $props();

  let mode = $state<'list' | 'add' | 'source'>('list');
  let newName = $state('');
  let newKind = $state<'buff' | 'debuff' | 'neutral' | 'condition'>('buff');
  let newIcon = $state('✨');
  let newDurationUnit = $state<'rounds' | 'minutes' | 'hours' | 'permanent'>('rounds');
  let newDurationValue = $state(1);
  let newConcentration = $state(false);
  let newTickTrigger = $state('target_turn_start');
  let newSourceType = $state<'spell' | 'ability' | 'item' | 'weapon' | 'manual' | 'condition'>('manual');
  let newSourceName = $state('');
  let sourceTab = $state<'spell' | 'ability' | 'item' | 'weapon'>('spell');
  let spellQuery = $state('');
  let spellResults = $state<Spell[]>([]);
  let spellLoading = $state(false);
  let selectedSpellSlug = $state('');

  const canManage = $derived(isMaster || isMe);
  const activeEffects = $derived(effects.filter((e) => e.active).sort((a, b) => a.name.localeCompare(b.name)));
  const expiredEffects = $derived(effects.filter((e) => !e.active).sort((a, b) => a.name.localeCompare(b.name)));

  async function addEffect() {
    if (!newName.trim()) return;
    if (!confirm($_('initiative.effect_add_confirm')
        .replace('{{name}}', newName.trim())
        .replace('{{target}}', combatant.display_name as string))) return;
    try {
      await Effects.apply(combatant.id as string, {
        name: newName.trim(),
        kind: newKind,
        icon: newIcon || '✨',
        duration_unit: newDurationUnit,
        duration_value: newDurationUnit === 'permanent' ? null : newDurationValue,
        remaining: newDurationUnit === 'permanent' ? null : newDurationValue,
        tick_trigger: newTickTrigger,
        concentration: newConcentration,
        source_type: newSourceType,
        source_name: newSourceName.trim() || null,
      });
      resetAdd();
      mode = 'list';
      onchange?.();
    } catch (e) { alert((e as Error).message); }
  }

  async function applySpell() {
    if (!selectedSpellSlug) return;
    if (!confirm($_('initiative.effect_apply_spell_confirm')
        .replace('{{target}}', combatant.display_name as string))) return;
    try {
      await Effects.applySpell(combatant.id as string, selectedSpellSlug);
      mode = 'list';
      onchange?.();
    } catch (e) { alert((e as Error).message); }
  }

  async function removeEffect(eid: string) {
    const eff = effects.find((e) => e.id === eid);
    if (!confirm($_('initiative.effect_remove_confirm')
        .replace('{{name}}', eff?.name ?? eid)
        .replace('{{target}}', combatant.display_name as string))) return;
    try {
      await Effects.remove(combatant.id as string, eid);
      onchange?.();
    } catch (e) { alert((e as Error).message); }
  }

  async function searchSpells() {
    if (!spellQuery.trim()) { spellResults = []; return; }
    spellLoading = true;
    try {
      spellResults = await Spells.list({ q: spellQuery.trim() });
      spellResults = spellResults.filter((s) => (s.effects)?.length);
    } catch { spellResults = []; }
    spellLoading = false;
  }

  function resetAdd() {
    newName = '';
    newKind = 'buff';
    newIcon = '✨';
    newDurationUnit = 'rounds';
    newDurationValue = 1;
    newConcentration = false;
    newTickTrigger = 'target_turn_start';
    newSourceType = 'manual';
    newSourceName = '';
  }
</script>

<div class="overlay" onclick={(e) => { if (e.target === e.currentTarget) onclose?.(); }} role="presentation">
  <div class="panel">
    <header class="panel-head">
      <h3>{combatant.display_name} — {$_('initiative.effects')}</h3>
      <button type="button" class="close-btn" onclick={() => onclose?.()}><X size={16} /></button>
    </header>

    {#if mode === 'list'}
      <div class="panel-body">
        {#if activeEffects.length === 0}
          <p class="empty">{$_('initiative.effect_none')}</p>
        {:else}
          <ul class="effect-list">
            {#each activeEffects as eff (eff.id)}
              <li class="effect-row">
                <EffectBadge effect={eff} size="md" showLabel={true} />
                <span class="meta">
                  {#if eff.source_name}{eff.source_name} · {/if}
                  {$_('initiative.effect_remaining')}: {eff.remaining ?? '∞'}
                  {#if eff.modifiers && (eff.modifiers as Record<string, unknown>).movement}
                    {@const mov = (eff.modifiers as Record<string, unknown>).movement as Record<string, unknown>}
                    · ↗ {mov.type} {mov.distance_ft}ft
                  {/if}
                  {#if eff.concentration}<span class="conc-dot" title={$_('initiative.effect_concentration')}>◎</span>{/if}
                </span>
                {#if canManage}
                  <button type="button" class="icon-btn danger" onclick={() => removeEffect(eff.id)} title={$_('initiative.effect_remove')}>
                    <Trash2 size={12} />
                  </button>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        {#if expiredEffects.length > 0}
          <details class="expired-block">
            <summary>{$_('initiative.effect_expired')} ({expiredEffects.length})</summary>
            <ul class="effect-list">
              {#each expiredEffects as eff (eff.id)}
                <li class="effect-row expired">
                  <EffectBadge effect={eff} size="sm" showLabel={true} />
                </li>
              {/each}
            </ul>
          </details>
        {/if}
      </div>
      {#if canManage}
        <footer class="panel-foot">
          <button type="button" class="btn btn-ghost" onclick={() => mode = 'add'}><Plus size={13} /> {$_('initiative.effect_add')}</button>
          <button type="button" class="btn btn-ghost" onclick={() => { mode = 'source'; spellQuery = ''; spellResults = []; selectedSpellSlug = ''; sourceTab = 'spell'; }}><Sparkles size={13} /> {$_('initiative.effect_apply_source')}</button>
        </footer>
      {/if}

    {:else if mode === 'add'}
      <div class="panel-body">
        <form onsubmit={(e) => { e.preventDefault(); addEffect(); }} class="add-form">
          <label class="field">
            <span>{$_('initiative.effect_name')}</span>
            <input bind:value={newName} required />
          </label>
          <label class="field">
            <span>{$_('initiative.effect_kind')}</span>
            <select bind:value={newKind}>
              <option value="buff">{$_('initiative.effect_buff')}</option>
              <option value="debuff">{$_('initiative.effect_debuff')}</option>
              <option value="neutral">{$_('initiative.effect_neutral')}</option>
              <option value="condition">{$_('initiative.effect_condition')}</option>
            </select>
          </label>
          <label class="field">
            <span>Icon</span>
            <input bind:value={newIcon} maxlength={4} />
          </label>
          <label class="field">
            <span>{$_('initiative.effect_duration')}</span>
            <select bind:value={newDurationUnit}>
              <option value="rounds">{$_('initiative.effect_rounds')}</option>
              <option value="minutes">{$_('initiative.effect_minutes')}</option>
              <option value="hours">{$_('initiative.effect_hours')}</option>
              <option value="permanent">{$_('initiative.effect_permanent')}</option>
            </select>
          </label>
          {#if newDurationUnit !== 'permanent'}
            <label class="field">
              <span>Value</span>
              <input type="number" min={1} bind:value={newDurationValue} />
            </label>
            <label class="field">
              <span>Tick trigger</span>
              <select bind:value={newTickTrigger}>
                <option value="target_turn_start">Target turn start</option>
                <option value="target_turn_end">Target turn end</option>
                <option value="caster_turn_start">Caster turn start</option>
                <option value="caster_turn_end">Caster turn end</option>
                <option value="round_end">Round end</option>
              </select>
            </label>
          {/if}
          <label class="field">
            <span>{$_('initiative.effect_source_type')}</span>
            <select bind:value={newSourceType}>
              <option value="spell">{$_('initiative.effect_source_spell')}</option>
              <option value="ability">{$_('initiative.effect_source_ability')}</option>
              <option value="item">{$_('initiative.effect_source_item')}</option>
              <option value="weapon">{$_('initiative.effect_source_weapon')}</option>
              <option value="condition">{$_('initiative.effect_source_condition')}</option>
              <option value="manual">{$_('initiative.effect_source_manual')}</option>
            </select>
          </label>
          <label class="field">
            <span>{$_('initiative.effect_source_name')}</span>
            <input bind:value={newSourceName} placeholder="e.g. Bless, Rage, Potion of Speed…" />
          </label>
          <label class="field check">
            <input type="checkbox" bind:checked={newConcentration} />
            <span>{$_('initiative.effect_concentration')}</span>
          </label>
          <div class="form-actions">
            <button type="button" class="btn btn-ghost" onclick={() => { resetAdd(); mode = 'list'; }}>{$_('common.cancel')}</button>
            <button type="submit" class="btn btn-primary">{$_('common.add')}</button>
          </div>
        </form>
      </div>

    {:else if mode === 'source'}
      <div class="panel-body">
        <nav class="source-tabs">
          {#each ['spell','ability','item','weapon'] as tab}
            <button type="button" class="source-tab {sourceTab === tab ? 'active' : ''}" onclick={() => { sourceTab = tab as typeof sourceTab; spellQuery = ''; spellResults = []; selectedSpellSlug = ''; }}>
              {$_(`initiative.effect_source_${tab}`)}
            </button>
          {/each}
        </nav>
        {#if sourceTab === 'spell'}
          <div class="spell-search">
            <input placeholder={$_('spells.search_ph')} bind:value={spellQuery} onkeydown={(e) => { if (e.key === 'Enter') searchSpells(); }} />
            <button type="button" class="btn btn-primary" onclick={searchSpells} disabled={spellLoading}>
              {spellLoading ? '…' : $_('common.search')}
            </button>
          </div>
          {#if spellResults.length > 0}
            <ul class="spell-list">
              {#each spellResults as sp (sp.slug)}
                {@const effs = sp.effects ?? []}
                <button type="button" class="spell-row {selectedSpellSlug === sp.slug ? 'selected' : ''}" onclick={() => selectedSpellSlug = sp.slug}>
                  <span class="spell-name">{sp.name}</span>
                  <span class="spell-meta">Lv {sp.level} · {sp.school}</span>
                  <span class="spell-effects">
                    {#each effs as ef, i (i)}
                      {ef.name}{#if i < effs.length - 1}, {/if}
                    {/each}
                  </span>
                </button>
              {/each}
            </ul>
            {#if selectedSpellSlug}
              <button type="button" class="btn btn-primary apply-btn" onclick={applySpell}>
                <Sparkles size={13} /> {$_('initiative.effect_apply_spell')}
              </button>
            {/if}
          {:else if !spellLoading && spellQuery}
            <p class="empty">{$_('spells.none')}</p>
          {/if}
        {:else}
          <p class="empty">{$_(`initiative.effect_source_${sourceTab}`)} catalog coming soon.</p>
        {/if}
        <button type="button" class="btn btn-ghost back-btn" onclick={() => mode = 'list'}>{$_('common.cancel')}</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.45);
    display: flex; align-items: center; justify-content: center;
    z-index: 50; padding: 1rem;
  }
  .panel {
    background: #1c1612;
    border: 1px solid rgba(139,105,20,0.35);
    border-radius: 0.5rem;
    width: min(28rem, 100%);
    max-height: 80vh;
    display: flex; flex-direction: column;
    overflow: hidden;
  }
  .panel-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid rgba(139,105,20,0.25);
    font-family: 'IM Fell English SC', 'Cinzel', serif;
  }
  .panel-head h3 { margin: 0; font-size: 1.05rem; color: #e8dcc8; }
  .close-btn {
    background: transparent; border: none; color: #a09080; cursor: pointer;
    display: flex; align-items: center; justify-content: center;
    padding: 0.25rem; border-radius: 0.25rem;
  }
  .close-btn:hover { background: rgba(139,105,20,0.2); color: #e8dcc8; }

  .panel-body { padding: 0.75rem 1rem; overflow-y: auto; flex: 1; }
  .panel-foot {
    display: flex; gap: 0.5rem; padding: 0.75rem 1rem;
    border-top: 1px solid rgba(139,105,20,0.25);
  }

  .empty { text-align: center; font-style: italic; color: #8b6355; margin: 1rem 0; }

  .effect-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.35rem; }
  .effect-row {
    display: flex; align-items: center; gap: 0.5rem; flex-wrap: wrap;
    padding: 0.35rem 0.25rem; border-radius: 0.25rem;
  }
  .effect-row:hover { background: rgba(139,105,20,0.08); }
  .effect-row .meta { font-size: 0.7rem; color: #a09080; margin-left: auto; }
  .effect-row .conc-dot { color: #c9a84c; }
  .effect-row.expired { opacity: 0.55; }

  .expired-block { margin-top: 0.5rem; }
  .expired-block summary { cursor: pointer; color: #8b6355; font-size: 0.8rem; }

  .add-form { display: flex; flex-direction: column; gap: 0.6rem; }
  .field { display: flex; flex-direction: column; gap: 0.2rem; font-size: 0.8rem; color: #c0b0a0; }
  .field input, .field select {
    background: #15100c; border: 1px solid rgba(139,105,20,0.3);
    color: #e8dcc8; border-radius: 0.3rem; padding: 0.35rem 0.5rem; font-size: 0.85rem;
  }
  .field.check { flex-direction: row; align-items: center; gap: 0.4rem; }

  .form-actions { display: flex; gap: 0.5rem; justify-content: flex-end; margin-top: 0.25rem; }

  .spell-search { display: flex; gap: 0.4rem; margin-bottom: 0.5rem; }
  .spell-search input { flex: 1; background: #15100c; border: 1px solid rgba(139,105,20,0.3); color: #e8dcc8; border-radius: 0.3rem; padding: 0.35rem 0.5rem; font-size: 0.85rem; }

  .spell-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.2rem; max-height: 14rem; overflow-y: auto; }
  .spell-row {
    padding: 0.4rem 0.5rem; border-radius: 0.3rem; cursor: pointer;
    border: 1.5px solid transparent;
    display: flex; flex-direction: column; gap: 0.1rem;
  }
  .spell-row:hover { background: rgba(139,105,20,0.12); }
  .spell-row.selected { border-color: rgba(201,168,76,0.6); background: rgba(139,105,20,0.15); }
  .spell-name { font-weight: 600; color: #e8dcc8; font-size: 0.85rem; }
  .spell-meta { font-size: 0.7rem; color: #a09080; }
  .spell-effects { font-size: 0.7rem; color: #8b6914; }

  .apply-btn { width: 100%; margin-top: 0.5rem; }
  .back-btn { margin-top: 0.5rem; }

  .btn {
    display: inline-flex; align-items: center; gap: 0.3rem;
    padding: 0.4rem 0.7rem; border-radius: 0.3rem; font-size: 0.8rem;
    border: 1px solid transparent; cursor: pointer; font-weight: 600;
  }
  .btn-ghost { background: transparent; border-color: rgba(139,105,20,0.35); color: #c9a84c; }
  .btn-ghost:hover { background: rgba(139,105,20,0.15); }
  .btn-primary { background: #6b4f1e; color: #f4e4c1; border-color: #8b6914; }
  .btn-primary:hover { background: #8b6914; }
  .btn-primary:disabled { opacity: 0.5; cursor: not-allowed; }

  .icon-btn {
    display: inline-flex; align-items: center; justify-content: center;
    padding: 0.25rem; border-radius: 0.25rem; border: none; background: transparent;
    color: #a09080; cursor: pointer;
  }
  .icon-btn:hover { background: rgba(139,105,20,0.15); }
  .icon-btn.danger:hover { background: rgba(169,53,53,0.2); color: #d66; }

  .source-tabs {
    display: flex; gap: 0.2rem; margin-bottom: 0.6rem;
    border-bottom: 1px solid rgba(139,105,20,0.25);
    padding-bottom: 0.3rem;
  }
  .source-tab {
    background: transparent; border: none; color: #a09080;
    font-size: 0.75rem; font-weight: 600; cursor: pointer;
    padding: 0.25rem 0.5rem; border-radius: 0.25rem;
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    text-transform: uppercase; letter-spacing: 0.06em;
  }
  .source-tab:hover { color: #c9a84c; }
  .source-tab.active { color: #c9a84c; background: rgba(139,105,20,0.15); }
</style>
