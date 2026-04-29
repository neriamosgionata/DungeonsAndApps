<script lang="ts">
  import type { Snippet } from 'svelte';
  import { Plus, X } from '@lucide/svelte';

  type Props = {
    label?: string;
    title?: string;
    alignEnd?: boolean;
    children: Snippet<[{ close: () => void }]>;
  };
  let { label = 'Add', title = '', alignEnd = true, children }: Props = $props();

  // accept legacy "+ Foo" labels — strip the plus, keep the text
  const cleanLabel = $derived(label.replace(/^\+\s*/, ''));

  let open = $state(false);
  function close() { open = false; }
</script>

{#if !open}
  <div class={alignEnd ? 'flex justify-end' : ''}>
    <button type="button" onclick={() => open = true}
      class="inline-flex items-center gap-1.5 rounded-md bg-violet-600 px-4 py-2 text-sm font-display tracking-wide uppercase">
      <Plus size={16} />
      <span>{cleanLabel}</span>
    </button>
  </div>
{:else}
  <div class="mt-3 space-y-3 rounded-lg border border-neutral-800 bg-neutral-900 p-4">
    <div class="flex items-center justify-between">
      {#if title}<h3 class="font-display text-lg">{title}</h3>{/if}
      <button type="button" onclick={close} aria-label="close"
        class="ml-auto text-neutral-400 hover:text-neutral-200"><X size={18} /></button>
    </div>
    {@render children({ close })}
  </div>
{/if}
