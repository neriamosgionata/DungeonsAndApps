<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Swords } from '@lucide/svelte';
  import type { Combatant } from '$lib/types';

  const DAMAGE_TYPES = [
    'slashing', 'piercing', 'bludgeoning', 'fire', 'cold', 'lightning',
    'thunder', 'acid', 'poison', 'necrotic', 'radiant', 'psychic', 'force',
  ];

  export type MultiattackTarget = {
    target_id: string;
    attack_expr: string;
    damage_expr: string;
    damage_type: string;
    weapon_id?: string;
  };

  export type MultiattackResult = {
    targets_hit: number;
    results: Array<unknown>;
    total_damage: number;
  };

  let {
    activeC,
    combatants,
    partyChars = [],
    // L-F4: multiattackParseTarget stays bindable (parent reads it for
    // the parsed multiattack target) — it doesn't conflict with anything.
    // The OTHER input fields (attackTarget, attackExpr, etc.) are local
    // state to avoid clobbering the parent-shared AttackForm bindings.
    multiattackParseTarget = $bindable(''),
    multiattackTargets = $bindable<MultiattackTarget[]>([]),
    multiattackResult = null,
    isInFlight,
    guarded,
    onParse,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    partyChars?: Array<{ id: string; sheet?: unknown }>;
    multiattackParseTarget?: string;
    multiattackTargets?: MultiattackTarget[];
    multiattackResult?: MultiattackResult | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onParse: (c: Combatant) => void | Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();

  // L-F4: local state for the input fields. Previously these were
  // $bindable props that two-way-bound to the parent's state, which
  // MultiattackForm shares with AttackForm — clearing attackTarget on
  // addTarget would clobber the main attack form's target.
  let attackTarget = $state('');
  let attackExpr = $state('');
  let damageExpr = $state('');
  let damageType = $state('slashing');
  let attackWeaponId = $state('');

  function addTarget() {
    if (!attackTarget) return;
    multiattackTargets = [
      ...multiattackTargets,
      {
        target_id: attackTarget,
        attack_expr: attackExpr,
        damage_expr: damageExpr,
        damage_type: damageType,
        weapon_id: attackWeaponId || undefined,
      },
    ];
    attackTarget = '';
    attackExpr = '';
    damageExpr = '';
  }

  function removeTarget(i: number) {
    multiattackTargets = multiattackTargets.filter((_, idx) => idx !== i);
  }

  // M-F2: per-attack target + weapon reassignment. The parsed-multiattack
  // flow (doParseMultiattack) sets all attacks to the same target. Let
  // the GM redirect individual attacks to different targets (e.g. bite
  // → enemy A, claws → enemy B) and pick a different weapon per attack
  // (main-hand for attack 1, off-hand for TWF attack 2).
  function retarget(i: number, new_target_id: string) {
    multiattackTargets = multiattackTargets.map((mt, idx) =>
      idx === i ? { ...mt, target_id: new_target_id } : mt
    );
  }
  function rearm(i: number, new_weapon_id: string) {
    multiattackTargets = multiattackTargets.map((mt, idx) =>
      idx === i ? { ...mt, weapon_id: new_weapon_id || undefined } : mt
    );
  }
  function getWeapons(c: Combatant): Array<{ id: string; name: string }> {
    // M-F2: weapon list comes from the linked character sheet (or the NPC
    // sheet for ref_type='npc'). The Combatant struct doesn't expose the
    // sheet directly — we look up via partyChars or npc_data.
    const ch = c.ref_type === 'character' && c.character_id
      ? partyChars.find((p) => p.id === c.character_id)
      : null;
    if (ch) {
      const sheet = ch.sheet as Record<string, unknown> | undefined;
      const raw = sheet?.weapons as Array<{ id?: string; name?: string }> | undefined;
      if (Array.isArray(raw)) {
        return raw
          .filter((w) => w.id && w.name)
          .map((w) => ({ id: w.id as string, name: w.name as string }));
      }
    }
    // NPC: read from the combatant's sheet (c.sheet may exist on the
    // payload) or fall back to active_effects / class features. For now
    // NPCs without explicit weapons get an empty list — the GM can still
    // roll multiattack without per-attack weapon.
    return [];
  }
</script>

<div class="ca-form">
  {#if activeC.npc_id}
    <div class="ca-row parse-row">
      <label class="ca-field">
        <span>{$_('initiative.label_parse_npc_multiattack')}</span>
        <select bind:value={multiattackParseTarget}>
          <option value="">{$_('initiative.err_select_parsed_target')}</option>
          {#each combatants as t (t.id)}
            {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
          {/each}
        </select>
      </label>
      <button
        type="button"
        class="ca-btn"
        disabled={isInFlight(`parse:ma:${activeC.id}`)}
        onclick={() => guarded(`parse:ma:${activeC.id}`, async () => { await onParse(activeC); })}
        title={$_('initiative.title_parse_multiattack')}>
        {$_('initiative.label_parse')}
      </button>
    </div>
    <hr class="ma-hr" />
  {/if}
  <div class="ca-row add-row">
    <label class="ca-field">
      <span>{$_('initiative.label_add_target')}</span>
      <select bind:value={attackTarget}>
        <option value="">{$_('initiative.ph_select_target')}</option>
        {#each combatants as t (t.id)}
          {#if t.id !== activeC.id}<option value={t.id}>{t.display_name}</option>{/if}
        {/each}
      </select>
    </label>
    <label class="ca-field">
      <span>{$_('initiative.action_atk')}</span>
      <input type="text" bind:value={attackExpr} placeholder="1d20+7" />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.action_dmg')}</span>
      <input type="text" bind:value={damageExpr} placeholder="1d8+4" />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.action_type')}</span>
      <select bind:value={damageType}>
        {#each DAMAGE_TYPES as t}
          <option value={t}>{t}</option>
        {/each}
      </select>
    </label>
    <button type="button" class="ca-btn" onclick={addTarget}>{$_('initiative.btn_add')}</button>
  </div>
  {#if multiattackTargets.length > 0}
    <div class="targets-list">
      {#each multiattackTargets as mt, i (i)}
        <div class="target-row">
          <span class="attack-num">#{i + 1}</span>
          <select
            class="target-select"
            value={mt.target_id}
            onchange={(e) => retarget(i, (e.currentTarget as HTMLSelectElement).value)}
            title="Reassign this attack's target">
            {#each combatants as t (t.id)}
              {#if t.id !== activeC.id}
                <option value={t.id}>{t.display_name}</option>
              {/if}
            {/each}
          </select>
          <select
            class="weapon-select"
            value={mt.weapon_id ?? ''}
            onchange={(e) => rearm(i, (e.currentTarget as HTMLSelectElement).value)}
            title="Reassign this attack's weapon">
            <option value="">— {$_('initiative.opt_no_weapon')} —</option>
            {#each getWeapons(activeC) as w (w.id)}
              <option value={w.id}>{w.name}</option>
            {/each}
          </select>
          <span class="attack-stats">
            {mt.attack_expr} / {mt.damage_expr} {mt.damage_type}
          </span>
          <button type="button" class="remove-btn" onclick={() => removeTarget(i)}>✕</button>
        </div>
      {/each}
    </div>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`multiattack:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`multiattack:${activeC.id}`)}>
    <Swords size={12} /> {$_('initiative.btn_roll_multiattack')}
  </button>
  {#if multiattackResult}
    <div class="ca-result hit">
      <span>{$_('initiative.msg_hit')} {multiattackResult.targets_hit}/{multiattackResult.results.length} — {multiattackResult.total_damage} {$_('initiative.msg_total_dmg')}</span>
    </div>
  {/if}
</div>

<style>
  .parse-row { align-items: flex-end; }
  .ma-hr { border-color: #3a2313; margin: 0.25rem 0; }
  .add-row { align-items: flex-end; }
  .targets-list { font-size: 0.7rem; color: #a6855c; margin-bottom: 0.25rem; display: flex; flex-direction: column; gap: 0.2rem; }
  .target-row { display: flex; align-items: center; gap: 0.3rem; padding: 0.2rem 0.4rem; background: rgba(0,0,0,0.05); border-radius: 3px; }
  .attack-num { color: #8b6914; font-weight: 700; min-width: 1.5rem; }
  .target-select, .weapon-select { padding: 0.15rem 0.3rem; font-size: 0.7rem; background: rgba(255,248,220,0.5); border: 1px solid #8b6914; color: #2c1810; border-radius: 3px; }
  .attack-stats { color: #a6855c; font-family: monospace; font-size: 0.7rem; }
  .remove-btn { font-size: 0.6rem; color: #a93535; }
</style>
