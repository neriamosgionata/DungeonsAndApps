<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { Swords } from '@lucide/svelte';
  import type { Character, Combatant } from '$lib/types';

  const DAMAGE_TYPES = [
    'slashing', 'piercing', 'bludgeoning', 'fire', 'cold', 'lightning',
    'thunder', 'acid', 'poison', 'necrotic', 'radiant', 'psychic', 'force',
  ];
  const EXTRA_DAMAGE_TYPES = [
    'piercing', 'slashing', 'bludgeoning', 'radiant', 'necrotic',
    'fire', 'cold', 'lightning', 'thunder', 'psychic', 'force',
  ];
  const COVER_TYPES = [
    { id: 'none', label_key: 'initiative.cover_none' },
    { id: 'half', label_key: 'initiative.cover_half' },
    { id: 'three_quarters', label_key: 'initiative.cover_three_quarters' },
  ];
  const BARDIC_DICE = [0, 6, 8, 10, 12];

  type Weapon = { id: string; name: string; attack_bonus?: number; damage?: string; damage_type?: string };
  type AttackResult = {
    hit: boolean;
    critical?: boolean;
    attack_total: number;
    target_ac: number;
    damage_applied: number;
    damage_resisted?: boolean;
    damage_vulnerable?: boolean;
    damage_immune?: boolean;
    extra_damage_applied?: number;
    extra_damage_type?: string | null;
    concentration_broken?: boolean;
    instant_death?: boolean;
  };
  type CoverResult = {
    attacker_id: string;
    target_id: string;
    cover_type: string;
    cover_bonus: number;
    blockers: string[];
  };

  let {
    activeC,
    combatants,
    partyChars,
    attackTarget = $bindable(''),
    attackWeaponId = $bindable(''),
    attackExpr = $bindable(''),
    damageExpr = $bindable(''),
    damageType = $bindable('slashing'),
    attackAdv = $bindable(false),
    attackDis = $bindable(false),
    powerAttack = $bindable(false),
    recklessAttack = $bindable(false),
    skipAmmo = $bindable(false),
    blessDice = $bindable(0),
    bardicInspirationDie = $bindable(0),
    coverType = $bindable('none'),
    extraDamageExpr = $bindable(''),
    extraDamageType = $bindable('piercing'),
    attackResult = null,
    coverResult = null,
    isInFlight,
    guarded,
    onCheckCover,
    onSubmit,
  }: {
    activeC: Combatant;
    combatants: Combatant[];
    partyChars: Character[];
    attackTarget?: string;
    attackWeaponId?: string;
    attackExpr?: string;
    damageExpr?: string;
    damageType?: string;
    attackAdv?: boolean;
    attackDis?: boolean;
    powerAttack?: boolean;
    recklessAttack?: boolean;
    skipAmmo?: boolean;
    blessDice?: number;
    bardicInspirationDie?: number;
    coverType?: string;
    extraDamageExpr?: string;
    extraDamageType?: string;
    attackResult?: AttackResult | null;
    coverResult?: CoverResult | null;
    isInFlight: (key: string) => boolean;
    guarded: (key: string, fn: () => Promise<unknown>) => Promise<void>;
    onCheckCover: (attackerId: string, targetId: string) => void | Promise<void>;
    onSubmit: (c: Combatant) => void | Promise<void>;
  } = $props();

  const activeChar = $derived(partyChars.find((p) => p.id === activeC.character_id));
  const weapons = $derived(
    ((activeChar?.sheet?.weapons ?? []) as Weapon[])
  );
  const hasWeapons = $derived(weapons.length > 0);
</script>

<div class="ca-form">
  <label class="ca-field">
    <span>{$_('initiative.label_target')}</span>
    <select bind:value={attackTarget}>
      <option value="">{$_('initiative.ph_select_target')}</option>
      {#each combatants as t (t.id)}
        <option value={t.id}>{t.display_name}</option>
      {/each}
    </select>
  </label>
  {#if activeC.character_id && hasWeapons}
    <label class="ca-field">
      <span>{$_('initiative.label_weapon')}</span>
      <select bind:value={attackWeaponId}>
        <option value="">{$_('initiative.ph_manual')}</option>
        {#each weapons as w (w.id)}
          <option value={w.id}>{w.name} {w.attack_bonus ? `(+${w.attack_bonus})` : ''} {w.damage ? `[${w.damage}]` : ''}</option>
        {/each}
      </select>
    </label>
  {/if}
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_attack')}</span>
      <input type="text" bind:value={attackExpr} placeholder={$_('initiative.ph_atk_expr')} />
    </label>
    <label class="ca-field">
      <span>{$_('initiative.label_damage')}</span>
      <input type="text" bind:value={damageExpr} placeholder={$_('initiative.ph_dmg_expr')} />
    </label>
  </div>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_dmg_type')}</span>
      <select bind:value={damageType}>
        {#each DAMAGE_TYPES as t}
          <option value={t}>{t}</option>
        {/each}
      </select>
    </label>
    <label class="ca-check"><input type="checkbox" bind:checked={attackAdv} /> {$_('initiative.label_adv')}</label>
    <label class="ca-check"><input type="checkbox" bind:checked={attackDis} /> {$_('initiative.label_dis')}</label>
    <label class="ca-check" title={$_('initiative.title_power_atk')}>
      <input type="checkbox" bind:checked={powerAttack} /> {$_('initiative.label_power_atk')}
    </label>
    <label class="ca-check" title={$_('initiative.title_reckless')}>
      <input type="checkbox" bind:checked={recklessAttack} /> {$_('initiative.label_reckless')}
    </label>
    <label class="ca-check" title={$_('initiative.title_skip_ammo')}>
      <input type="checkbox" bind:checked={skipAmmo} /> {$_('initiative.label_skip_ammo')}
    </label>
    <label class="ca-field" title={$_('initiative.title_bless')}>
      <span>{$_('initiative.label_bless')}</span>
      <input type="number" min="0" max="4" bind:value={blessDice} style="width:3rem" />
    </label>
    <label class="ca-field" title={$_('initiative.title_bardic_inspiration')}>
      <span>{$_('initiative.label_bard')}</span>
      <select bind:value={bardicInspirationDie} style="width:4rem">
        {#each BARDIC_DICE as d}
          <option value={d}>{d === 0 ? '—' : `d${d}`}</option>
        {/each}
      </select>
    </label>
    <label class="ca-field">
      <span>{$_('initiative.label_cover')}</span>
      <select bind:value={coverType}>
        {#each COVER_TYPES as c (c.id)}
          <option value={c.id}>{$_(c.label_key)}</option>
        {/each}
      </select>
    </label>
    <button
      type="button"
      class="ca-btn"
      onclick={() => attackTarget && onCheckCover(activeC.id as string, attackTarget)}>
      {$_('initiative.label_check_cover')}
    </button>
  </div>
  <div class="ca-row">
    <label class="ca-field">
      <span>{$_('initiative.label_extra_dmg')}</span>
      <input type="text" bind:value={extraDamageExpr} placeholder={$_('initiative.ph_extra_dmg')} />
    </label>
    {#if extraDamageExpr}
      <label class="ca-field">
        <span>{$_('initiative.label_extra_type')}</span>
        <select bind:value={extraDamageType}>
          {#each EXTRA_DAMAGE_TYPES as t}
            <option value={t}>{t}</option>
          {/each}
        </select>
      </label>
    {/if}
  </div>
  {#if coverResult && coverResult.attacker_id === activeC.id && coverResult.target_id === attackTarget}
    <div class="ca-result">
      <span>{$_('initiative.label_cover_result', { values: { type: coverResult.cover_type, bonus: coverResult.cover_bonus } })}</span>
      {#if coverResult.blockers.length > 0}<span>{$_('initiative.label_blocked_by', { values: { blockers: coverResult.blockers.join(', ') } })}</span>{/if}
    </div>
  {/if}
  <button
    type="button"
    class="ca-submit"
    onclick={() => guarded(`attack:${activeC.id}`, async () => { await onSubmit(activeC); })}
    disabled={isInFlight(`attack:${activeC.id}`)}>
    <Swords size={12} /> {$_('initiative.label_roll_attack_btn')}
  </button>
  {#if attackResult}
    <div class="ca-result {attackResult.hit ? 'hit' : 'miss'}">
      {#if attackResult.critical}<span class="ca-crit">{$_('initiative.label_crit')}</span>{/if}
      {#if attackResult.hit}
        <span>{$_('initiative.label_hit_msg', { values: { total: attackResult.attack_total, ac: attackResult.target_ac } })}</span>
        <span>
          {$_('initiative.label_damage_msg', { values: { amount: attackResult.damage_applied } })}
          {#if attackResult.damage_resisted}({$_('initiative.label_damage_resisted')}){/if}
          {#if attackResult.damage_vulnerable}({$_('initiative.label_damage_vulnerable')}){/if}
          {#if attackResult.damage_immune}({$_('initiative.label_damage_immune')}){/if}
          {#if attackResult.extra_damage_applied} + {attackResult.extra_damage_applied} {attackResult.extra_damage_type ?? ''}{/if}
        </span>
        {#if attackResult.concentration_broken}<span class="ca-conc">{$_('initiative.label_conc_broken')}</span>{/if}
        {#if attackResult.instant_death}<span class="ca-crit">{$_('initiative.label_instadeath')}</span>{/if}
      {:else}
        <span>{$_('initiative.label_miss_msg', { values: { total: attackResult.attack_total, ac: attackResult.target_ac } })}</span>
      {/if}
    </div>
  {/if}
</div>
