<script lang="ts">
  import type { Snippet } from 'svelte';
  let {
    title,
    subtitle,
    icon,
    actions,
  }: {
    title: string;
    subtitle?: string;
    icon?: Snippet;
    actions?: Snippet;
  } = $props();
</script>

<div class="section-header">
  <div class="title-row">
    <div class="title-group">
      {#if icon}<span class="icon">{@render icon()}</span>{/if}
      <div>
        <h2 class="title">{title}</h2>
        {#if subtitle}<p class="subtitle">{subtitle}</p>{/if}
      </div>
    </div>
    {#if actions}<div class="actions">{@render actions()}</div>{/if}
  </div>
  <div class="rule">
    <span class="rule-ornament"></span>
  </div>
</div>

<style>
  .section-header { margin-bottom: 1.25rem; }
  .title-row { display: flex; align-items: center; gap: 1rem; }
  .title-group { display: flex; align-items: center; gap: 0.75rem; flex: 1; }
  .icon {
    display: grid; place-items: center;
    width: 2.25rem; height: 2.25rem;
    border-radius: 9999px;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 45%, #6d510f 100%);
    color: #1a0f08;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255, 248, 220, 0.5), 0 1px 2px rgba(0,0,0,0.5);
  }
  .title {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 1.5rem;
    letter-spacing: 0.05em;
    color: inherit;
  }
  .subtitle {
    font-family: 'Crimson Text', serif;
    font-style: italic;
    font-size: 0.875rem;
    color: inherit;
    opacity: 0.72;
  }
  .actions { display: flex; align-items: center; gap: 0.5rem; }

  .rule {
    position: relative;
    margin-top: 0.75rem;
    height: 2px;
    background: linear-gradient(to right, transparent, #8b6914 15%, #c9a84c 50%, #8b6914 85%, transparent);
    opacity: 0.8;
  }
  .rule-ornament {
    position: absolute; top: 50%; left: 50%;
    width: 14px; height: 14px;
    transform: translate(-50%, -50%) rotate(45deg);
    background: radial-gradient(circle, #c9a84c 0%, #6d510f 100%);
    border: 1px solid #4e3909;
    box-shadow: 0 0 0 3px #1a0f08, 0 0 0 4px #8b6914;
  }
</style>
