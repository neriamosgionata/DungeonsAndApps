<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Sparkles } from '@lucide/svelte';
  import type { Combatant, EncounterOverlay } from '$lib/types';

  const OVERLAY_DAMAGE_TYPES = [
    'fire', 'cold', 'lightning', 'thunder', 'acid', 'poison',
    'necrotic', 'radiant', 'psychic', 'force', 'slashing', 'piercing', 'bludgeoning',
  ];
  const OVERLAY_SAVE_ABILITIES = [
    { id: 'dex', label: 'DEX' },
    { id: 'con', label: 'CON' },
    { id: 'wis', label: 'WIS' },
    { id: 'str', label: 'STR' },
    { id: 'int', label: 'INT' },
    { id: 'cha', label: 'CHA' },
  ];

  let {
    overlays,
    combatants = [],
    overlayDmgId = $bindable(''),
    overlayDmgExpr = $bindable(''),
    overlayDmgType = $bindable('fire'),
    overlaySaveAbility = $bindable('dex'),
    overlaySaveDc = $bindable(15),
    overlayHalfOnSave = $bindable(true),
    overlayDmgResult = null,
    isInFlight,
    guarded,
    onApply,
  }: {
    overlays: EncounterOverlay[];
    combatants?: Combatant[];
    overlayDmgId?: string;
    overlayDmgExpr?: string;
    overlayDmgType?: string;
    overlaySaveAbility?: string;
    overlaySaveDc?: number | '';
    overlayHalfOnSave?: boolean;
    overlayDmgResult?: { targets_affected: Array<{ target_id: string; target_name: string; damage_applied: number; save_passed: boolean | null }> } | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onApply: () => void | Promise<void>;
  } = $props();

  // Mirror the backend's in-area math (backend/src/routes/combat/tactical/hazards.rs)
  // so the master sees a live preview of who will be affected before submitting.
  // Defaults: radius 20% of map, origin (50,50).
  function inArea(o: EncounterOverlay, c: Combatant): boolean {
    if (!c.token_on_map || c.token_x == null || c.token_y == null) return false;
    const ox = o.origin_x ?? 50;
    const oy = o.origin_y ?? 50;
    const r = o.radius_ft ?? 20;
    const dx = c.token_x - ox;
    const dy = c.token_y - oy;
    if (o.shape === 'cube') {
      return Math.abs(dx) <= r && Math.abs(dy) <= r;
    }
    return Math.sqrt(dx * dx + dy * dy) <= r;
  }

  let selectedOverlay = $derived(overlays.find((o) => o.id === overlayDmgId) ?? null);
  let inAreaCount = $derived(
    selectedOverlay ? combatants.filter((c) => inArea(selectedOverlay!, c)).length : 0,
  );
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_overlay')}</span>
    <select bind:value={overlayDmgId}>
      <option value="">{$_('initiative.ph_select_overlay')}</option>
      {#each overlays.filter((o) => o.active) as o (o.id)}
        <option value={o.id}>{o.label || o.zone_type || 'Overlay'} ({o.shape})</option>
      {/each}
    </select>
  </label>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_damage')}</span>
      <input type="text" bind:value={overlayDmgExpr} placeholder="8d6" />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.label_dmg_type')}</span>
      <select bind:value={overlayDmgType}>
        {#each OVERLAY_DAMAGE_TYPES as t}
          <option value={t}>{$_('initiative.damage_type_' + t)}</option>
        {/each}
      </select>
    </label>
  </div>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_save_short')}</span>
      <select bind:value={overlaySaveAbility}>
        {#each OVERLAY_SAVE_ABILITIES as a (a.id)}
          <option value={a.id}>{a.label}</option>
        {/each}
      </select>
    </label>
    <label class="ca-field"><span>{$_('initiative.label_dc')}</span><input type="number" bind:value={overlaySaveDc} placeholder="15" /></label>
    <label class="ca-check"><input type="checkbox" bind:checked={overlayHalfOnSave} /> {$_('initiative.label_half_on_save')}</label>
  </div>
  {#if selectedOverlay}
    <div class="ca-area-preview" class:zero={inAreaCount === 0}>
      {$_('initiative.overlay_area_count', { values: { n: inAreaCount } })}
    </div>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded('overlay:damage', async () => { await onApply(); })}
    disabled={isInFlight('overlay:damage') || !overlayDmgId || inAreaCount === 0}>
    <Sparkles size={12} /> {$_('initiative.btn_apply_overlay_damage')}
  </button>
  {#if overlayDmgResult}
    <div class="ca-result">
      {#each overlayDmgResult.targets_affected as ta (ta.target_id)}
        <span>{ta.target_name}: {ta.damage_applied} {$_('initiative.msg_dmg')} {#if ta.save_passed === true}({$_('initiative.msg_saved')}){:else if ta.save_passed === false}({$_('initiative.msg_failed_saving')}){/if}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .ca-area-preview { font-size: 0.7rem; padding: 0.25rem 0.4rem; margin-top: 0.3rem; color: #c9a84c; background: rgba(44,24,16,0.4); border: 1px solid rgba(201,168,76,0.3); border-radius: 0.25rem; }
  .ca-area-preview.zero { color: #ff9090; border-color: rgba(255,144,144,0.4); }
</style>
