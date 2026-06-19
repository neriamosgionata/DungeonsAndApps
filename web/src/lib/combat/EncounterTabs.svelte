<script lang="ts">
  import { _ } from 'svelte-i18n';
  import type { Encounter } from '$lib/types';

  let {
    encounters,
    selectedId,
    onSelect,
  }: {
    encounters: Encounter[];
    selectedId: string | null;
    onSelect: (id: string) => void;
  } = $props();
</script>

<nav class="enc-tabs">
  {#each encounters as e (e.id)}
    <button
      type="button"
      class="enc-tab {selectedId === e.id ? 'active' : ''}"
      onclick={() => onSelect(e.id as string)}>
      <span>{e.name}</span>
      <span class="enc-status status-{e.status}">{$_(`initiative.status_${e.status}`)}</span>
    </button>
  {/each}
</nav>

<style>
  .enc-tabs {
    display: flex; gap: 0.4rem; flex-wrap: wrap;
    border-bottom: 1.5px solid rgba(139,105,20,0.3);
    margin-bottom: 0.75rem;
  }
  .enc-tab {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.4rem 0.9rem;
    background: rgba(244,228,193,0.5);
    color: #2c1810;
    border: 1px solid rgba(139,105,20,0.3);
    border-bottom: 0;
    border-radius: 0.4rem 0.4rem 0 0;
    cursor: pointer;
    font-family: 'Cinzel', serif;
    font-size: 0.85rem;
  }
  .enc-tab.active {
    background: #f4e4c1;
    color: #1a0f08;
    font-weight: 700;
    border-color: #8b6914;
  }
  .enc-status {
    font-size: 0.65rem;
    padding: 0.1rem 0.35rem;
    border-radius: 9999px;
    text-transform: uppercase;
    font-family: 'IM Fell English SC', serif;
  }
  .enc-status.status-planned { background: rgba(201,168,76,0.2); color: #6d510f; }
  .enc-status.status-active { background: rgba(40,160,80,0.25); color: #1f5d2a; }
  .enc-status.status-ended { background: rgba(80,80,80,0.25); color: #555; }
</style>
