<script lang="ts">
  /**
   * "+ Add" button → popup modal.
   * Name kept for API compatibility; the form renders inside a walnut dialog
   * on a dimmed overlay instead of expanding inline.
   */
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

<div class={alignEnd ? 'flex justify-end' : ''}>
  <button type="button" onclick={() => (open = true)}
    class="inline-flex items-center gap-1.5 rounded-md bg-violet-600 px-4 py-2 text-sm font-display tracking-wide uppercase">
    <Plus size={16} />
    <span>{cleanLabel}</span>
  </button>
</div>

{#if open}
  <div class="add-backdrop" role="presentation"
    onclick={close}
    onkeydown={(e) => e.key === 'Escape' && close()}>
    <div class="add-modal" role="dialog" aria-modal="true" tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}>
      <header class="add-head">
        {#if title}<h3>{title}</h3>{:else}<h3>{cleanLabel}</h3>{/if}
        <button type="button" onclick={close} aria-label="close" class="close-btn">
          <X size={16} />
        </button>
      </header>
      <div class="add-body">
        {@render children({ close })}
      </div>
    </div>
  </div>
{/if}

<style>
  .add-backdrop {
    position: fixed; inset: 0;
    background: rgba(0,0,0,0.7);
    display: grid; place-items: center;
    z-index: 60; padding: 1rem;
  }
  .add-modal {
    width: min(36rem, 100%);
    max-height: 90vh;
    overflow-y: auto;
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #241810;
    color: #f4e4c1;
    box-shadow:
      inset 0 1px 0 rgba(201,168,76,0.15),
      0 18px 40px rgba(0,0,0,0.65);
  }
  .add-head {
    display: flex; align-items: center; justify-content: space-between;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #4e3909;
    background: linear-gradient(180deg, rgba(201,168,76,0.08), transparent);
  }
  .add-head h3 {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.05em;
    font-size: 1.1rem;
    color: #f4e4c1 !important;
    margin: 0;
  }
  .close-btn {
    display: grid; place-items: center;
    width: 1.75rem; height: 1.75rem;
    border-radius: 9999px;
    background: #3a2313;
    color: #c9a84c;
    border: 1px solid #4e3909;
  }
  .close-btn:hover { background: #4e3909; color: #f7e2a5; }
  .add-body { padding: 1rem; }
</style>
