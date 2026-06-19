<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    onClose,
    title,
    dark = false,
    children,
  }: {
    onClose: () => void;
    title?: string;
    dark?: boolean;
    children: Snippet;
  } = $props();
</script>

<div
  class="fixed inset-0 z-50 flex items-center justify-center"
  role="presentation"
  style="background:rgba(0,0,0,0.75);"
  onclick={onClose}
  onkeydown={(e) => e.key === 'Escape' && onClose()}
>
  <div
    class="max-w-lg w-full max-h-[80vh] overflow-y-auto rounded-lg border p-4 space-y-2"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
    style={dark
      ? 'border-color:#a6855c; background:#171717; color:#f4e4c1;'
      : 'border-color:#a6855c; background:#f4e4c1; color:#2c1810;'}
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    {#if title}
      <div class="flex items-center justify-between">
        <h3
          class="font-display font-bold text-lg"
          style={dark ? 'color:#c9a84c;' : 'color:#2c1810;'}>
          {title}
        </h3>
        <button onclick={onClose} class="text-sm" style="color:#8b6355;">✕</button>
      </div>
    {/if}
    {@render children()}
  </div>
</div>
