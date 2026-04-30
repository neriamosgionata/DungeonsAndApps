<script lang="ts">
  import type { CombatantEffect } from '$lib/types';
  import { _ } from 'svelte-i18n';
  import { get } from 'svelte/store';

  interface Props {
    effect: CombatantEffect;
    size?: 'sm' | 'md' | 'lg';
    showLabel?: boolean;
    onclick?: () => void;
  }

  let { effect, size = 'sm', showLabel = false, onclick }: Props = $props();

  function kindColor(kind: string): string {
    switch (kind) {
      case 'buff': return '#4a7c59';
      case 'debuff': return '#a93535';
      case 'condition': return '#c9a84c';
      default: return '#6b7b8a';
    }
  }

  function kindBg(kind: string): string {
    switch (kind) {
      case 'buff': return 'rgba(74,124,89,0.15)';
      case 'debuff': return 'rgba(169,53,53,0.15)';
      case 'condition': return 'rgba(201,168,76,0.15)';
      default: return 'rgba(107,123,138,0.15)';
    }
  }

  function durationText(e: CombatantEffect): string {
    if (!e.active) return get(_)('initiative.effect_expired');
    if (e.duration_unit === 'permanent' || e.remaining == null) return '';
    const unitKey = `initiative.effect_${e.duration_unit}`;
    const unit = get(_)(unitKey) || e.duration_unit;
    return `${e.remaining} ${unit}`;
  }

  function movementText(e: CombatantEffect): string | null {
    const m = e.modifiers as Record<string, unknown> | undefined;
    const mov = m?.movement as Record<string, unknown> | undefined;
    if (!mov) return null;
    const type = mov.type as string;
    const dist = mov.distance_ft as number;
    return `${type} ${dist}ft`;
  }

  /** Map common Lucide icon names to emoji so the badge doesn't show raw text. */
  function iconToEmoji(name: string): string {
    const map: Record<string, string> = {
      'sparkles': '✨', 'heart-pulse': '💓', 'tree-pine': '🌲', 'cloud-fog': '🌫️',
      'shield-check': '🛡️', 'sun': '☀️', 'sun-medium': '🌞', 'zap': '⚡',
      'zap-fast': '⚡', 'feather': '🪶', 'footprints': '👣', 'cloud': '☁️',
      'eye-off': '🙈', 'compass': '🧭', 'flame': '🔥', 'eye': '👁️',
      'shield': '🛡️', 'shield-plus': '🛡️', 'gem': '💎', 'waves': '🌊',
      'leaf': '🍃', 'flame-kindling': '🔥', 'copy': '📋', 'move-vertical': '↕️',
      'skull': '💀', 'brain-circuit': '🧠', 'crown': '👑', 'vine': '🌿',
      'ghost': '👻', 'lock': '🔒', 'target': '🎯', 'ban': '🚫',
      'clock': '⏰', 'spider-web': '🕸️', 'arrow-up': '⬆️', 'rabbit': '🐇',
      'wind': '💨', 'circle-dot': '●', 'star': '⭐', 'sword': '⚔️',
    };
    return map[name] || name.slice(0, 2);
  }

  const sz = $derived({
    sm: { w: '1.1rem', fs: '0.6rem', pad: '0.05rem 0.25rem', gap: '0.15rem' },
    md: { w: '1.35rem', fs: '0.7rem', pad: '0.1rem 0.35rem', gap: '0.2rem' },
    lg: { w: '1.6rem', fs: '0.8rem', pad: '0.15rem 0.45rem', gap: '0.25rem' },
  }[size]);
</script>

{#if showLabel}
  <button type="button" class="badge-row" style="background: {kindBg(effect.kind)}; color: {kindColor(effect.kind)}; padding: {sz.pad}; gap: {sz.gap};" onclick={onclick} title="{effect.name}{effect.concentration ? ' (' + $_('initiative.effect_concentration') + ')' : ''}">
    <span class="icon" style="width: {sz.w}; height: {sz.w}; font-size: {sz.fs};">{iconToEmoji(effect.icon)}</span>
    <span class="name">{effect.name}</span>
    {#if movementText(effect)}
      <span class="mov">↗ {movementText(effect)}</span>
    {/if}
    {#if durationText(effect)}
      <span class="dur">{durationText(effect)}</span>
    {/if}
    {#if effect.concentration}
      <span class="conc">◎</span>
    {/if}
  </button>
{:else}
  <button type="button" class="badge-dot" style="background: {kindBg(effect.kind)}; color: {kindColor(effect.kind)}; width: {sz.w}; height: {sz.w}; font-size: {sz.fs};" onclick={onclick} title="{effect.name}{effect.concentration ? ' (' + $_('initiative.effect_concentration') + ')' : ''} — {durationText(effect)}">
    {iconToEmoji(effect.icon)}
  </button>
{/if}

<style>
  .badge-row {
    display: inline-flex;
    align-items: center;
    border-radius: 999px;
    border: 1px solid currentColor;
    font-size: 0.7rem;
    font-weight: 600;
    cursor: pointer;
    white-space: nowrap;
    line-height: 1;
  }
  .badge-row:hover { filter: brightness(1.15); }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }
  .name { font-weight: 600; }
  .dur { opacity: 0.8; font-weight: 400; }
  .mov { opacity: 0.85; font-weight: 500; font-size: 0.75em; }
  .conc { font-size: 0.65em; opacity: 0.9; }

  .badge-dot {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    border: 1.5px solid currentColor;
    cursor: pointer;
    flex-shrink: 0;
    line-height: 1;
  }
  .badge-dot:hover { filter: brightness(1.2); }
</style>
