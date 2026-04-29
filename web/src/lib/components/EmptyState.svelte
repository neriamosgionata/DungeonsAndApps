<script lang="ts">
  import type { Snippet } from 'svelte';
  let {
    title,
    hint,
    actions,
  }: {
    title: string;
    hint?: string;
    actions?: Snippet;
  } = $props();
</script>

<div class="empty-state">
  <svg class="crest" viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
    <defs>
      <radialGradient id="coin" cx="50%" cy="40%" r="55%">
        <stop offset="0%"  stop-color="#f7e2a5"/>
        <stop offset="55%" stop-color="#c9a84c"/>
        <stop offset="100%" stop-color="#6d510f"/>
      </radialGradient>
    </defs>
    <!-- outer ring -->
    <circle cx="32" cy="32" r="28" stroke="#8b6914" stroke-width="1.5" fill="url(#coin)"/>
    <!-- inner ring -->
    <circle cx="32" cy="32" r="22" stroke="#4e3909" stroke-width="1" fill="none"/>
    <!-- boar silhouette (cinghiale sigil) -->
    <path d="M16 40 q2-4 6-4 q1-4 4-4 l3-3 q1-2 3-1 l4-1 q5-1 9 2 q3 2 3 5 l-1 2 l2 2 v3 l-3 2 l-2-1 l-1 2 l-5 0 l-1-2 l-3 2 l-2-1 l-1 1 l-3-1 l-2 2 l-4-2 l-2 0 z"
      fill="#2c1810" opacity="0.85"/>
    <!-- tusks -->
    <path d="M19 40 l-2-1 M18 39 l-1-2" stroke="#f4e4c1" stroke-width="1" stroke-linecap="round"/>
    <!-- stars -->
    <circle cx="50" cy="18" r="1.5" fill="#f7e2a5"/>
    <circle cx="14" cy="18" r="1.2" fill="#f7e2a5"/>
  </svg>

  <h3 class="title">{title}</h3>
  {#if hint}<p class="hint">{hint}</p>{/if}
  {#if actions}<div class="actions">{@render actions()}</div>{/if}
</div>

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 0.75rem;
    padding: 3rem 1.5rem;
  }
  .crest {
    width: 96px; height: 96px;
    filter: drop-shadow(0 4px 6px rgba(0,0,0,0.6));
    animation: glow 4s ease-in-out infinite;
  }
  @keyframes glow {
    0%, 100% { filter: drop-shadow(0 4px 6px rgba(0,0,0,0.6)) drop-shadow(0 0 4px rgba(201,168,76,0.15)); }
    50%      { filter: drop-shadow(0 4px 6px rgba(0,0,0,0.6)) drop-shadow(0 0 12px rgba(201,168,76,0.4)); }
  }
  .title {
    font-family: 'Cinzel', serif;
    font-size: 1.125rem;
    letter-spacing: 0.05em;
    color: #c9a84c;
  }
  .hint {
    font-family: 'Crimson Text', serif;
    font-style: italic;
    color: #d4b896;
    max-width: 28rem;
  }
  .actions { margin-top: 0.5rem; display: flex; gap: 0.5rem; }
</style>
