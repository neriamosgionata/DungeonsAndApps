<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Sparkles } from '@lucide/svelte';
  import type { Character, Combatant } from '$lib/types';

  type Spell = {
    slug: string;
    name: string;
    level: number;
    concentration?: boolean;
    ritual?: boolean;
    casting_time?: string | null;
    range_text?: string | null;
    components?: string | null;
  };

  let {
    activeC,
    combatants,
    partyChars,
    allSpells,
    castSpellFilter = $bindable(''),
    castSpellSlug = $bindable(''),
    castTargets = $bindable<string[]>([]),
    castDamageExpr = $bindable(''),
    castUseSpellAttack = $bindable(false),
    castHalfOnSave = $bindable(false),
    castUpcastLevel = $bindable<number | null>(null),
    castSaveDc = $bindable<number | null>(null),
    castAsRitual = $bindable(false),
    castResult = null,
    isInFlight,
    guarded,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    partyChars: Character[];
    allSpells: Spell[];
    castSpellFilter?: string;
    castSpellSlug?: string;
    castTargets?: string[];
    castDamageExpr?: string;
    castUseSpellAttack?: boolean;
    castHalfOnSave?: boolean;
    castUpcastLevel?: number | null;
    castSaveDc?: number | null;
    castAsRitual?: boolean;
    castResult?: {
      spell_name: string;
      targets: Array<{
        target_id: string;
        target_name: string;
        hit?: boolean | null;
        attack_total?: number | null;
        critical: boolean;
        damage_applied: number;
        save_passed?: boolean | null;
        concentration_broken: boolean;
      }>;
    } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();

  const filteredSpells = $derived(
    castSpellFilter
      ? allSpells.filter((s) =>
          s.name.toLowerCase().includes(castSpellFilter.toLowerCase()) ||
          s.slug.toLowerCase().includes(castSpellFilter.toLowerCase())
        )
      : allSpells.slice(0, 50)
  );

  const selectedSpell = $derived(allSpells.find((s) => s.slug === castSpellSlug));

  function cantripMultiplier(level: number): number {
    return level >= 17 ? 4 : level >= 11 ? 3 : level >= 5 ? 2 : 1;
  }

  const cantripLevel = $derived(
    activeC.character_id
      ? (partyChars.find((p) => p.id === activeC.character_id)?.level_total ?? 1)
      : 1
  );

  const cantripMult = $derived(cantripMultiplier(cantripLevel));

  const scaledDamageExpr = $derived(
    cantripMult > 1 && castDamageExpr
      ? castDamageExpr.replace(/^(\d+)/, (_, n: string) => String(Number(n) * cantripMult))
      : null
  );
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_spell')}</span>
    <input type="text" bind:value={castSpellFilter} placeholder={$_('initiative.ph_search_spells')} class="mb-1" />
    <select bind:value={castSpellSlug} size={4}>
      <option value="">— Select a spell —</option>
      {#each filteredSpells as s (s.slug)}
        <option value={s.slug}>{s.name} (Lv{s.level}) {s.concentration ? '•' : ''}</option>
      {/each}
    </select>
  </label>
  {#if selectedSpell}
    <div class="spell-meta">
      {selectedSpell.casting_time ?? ''} • {selectedSpell.range_text ?? ''} • {selectedSpell.components ?? ''}
      {#if selectedSpell.concentration}• Concentration{/if}
    </div>
  {/if}
  <label class="ca-field">
    <span>{$_('initiative.label_targets')}</span>
    <select multiple bind:value={castTargets} size={3}>
      {#each combatants as t (t.id)}
        <option value={t.id}>{t.display_name}</option>
      {/each}
    </select>
  </label>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_damage')}</span>
      <input type="text" bind:value={castDamageExpr} placeholder="8d6" />
    </label>
    <label class="ca-check" title={$_('initiative.title_spell_attack')}>
      <input type="checkbox" bind:checked={castUseSpellAttack} /> {$_('initiative.label_spell_attack')}
    </label>
    {#if !castUseSpellAttack}
      <label class="ca-check">
        <input type="checkbox" bind:checked={castHalfOnSave} /> {$_('initiative.label_half_on_save')}
      </label>
    {/if}
  </div>
  {#if selectedSpell?.level === 0 && castDamageExpr && scaledDamageExpr}
    <div class="cantrip-hint">
      Cantrip scaled ×{cantripMult} at level {cantripLevel}: server will roll {scaledDamageExpr}
    </div>
  {/if}
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_upcast_level')}</span>
      <input type="number" min={1} max={9} bind:value={castUpcastLevel} placeholder={$_('initiative.ph_upcast_level')} />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.label_save_dc')}</span>
      <input type="number" bind:value={castSaveDc} placeholder={$_('initiative.ph_save_dc')} />
    </label>
  </div>
  {#if selectedSpell?.ritual}
    <label class="ca-check">
      <input type="checkbox" bind:checked={castAsRitual} /> Cast as Ritual (no slot)
    </label>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`cast:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`cast:${activeC.id}`)}>
    <Sparkles size={12} /> Cast Spell
  </button>
  {#if castResult}
    <div class="ca-result">
      <span class="ca-crit">{castResult.spell_name}</span>
      {#each castResult.targets as t (t.target_id)}
        <span>
          {t.target_name}:
          {#if t.hit === false}Miss ({t.attack_total})
          {:else if t.hit === true}{#if t.critical}CRIT! {/if}Hit ({t.attack_total}) — {t.damage_applied} dmg
          {:else}{t.damage_applied} dmg {#if t.save_passed === true}(saved){:else if t.save_passed === false}(failed){/if}
          {/if}
          {#if t.concentration_broken} [conc broken]{/if}
        </span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .spell-meta {
    font-size: 0.7rem;
    color: #a6855c;
    margin-bottom: 0.5rem;
  }
  .cantrip-hint {
    font-size: 0.7rem;
    color: #a6855c;
  }
</style>
