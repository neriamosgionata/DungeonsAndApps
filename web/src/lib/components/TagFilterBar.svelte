<script lang="ts">
  import { onMount } from 'svelte';
  import { Tags } from '$lib/api/resources';

  // Campaign tag filter row: click a tag to filter, click again to clear.
  let {
    cid,
    filter = $bindable(''),
  }: {
    cid: string;
    filter?: string;
  } = $props();

  let tags = $state<Array<{ id: string; name: string; color: string }>>([]);

  onMount(async () => {
    try { tags = (await Tags.list(cid)).tags; } catch { tags = []; }
  });
</script>

{#if tags.length}
  <div class="flex flex-wrap items-center gap-1">
    <button type="button" onclick={() => filter = ''}
      class="rounded px-2 py-0.5 text-xs {!filter ? 'font-bold' : ''}"
      style="background:{!filter ? '#8b6914' : 'rgba(139,105,20,0.25)'};color:#f4e4c1;">All</button>
    {#each tags as t (t.id)}
      <button type="button" onclick={() => filter = filter === t.id ? '' : t.id}
        class="rounded px-2 py-0.5 text-xs {filter === t.id ? 'font-bold' : ''}"
        style="background:{filter === t.id ? t.color : 'rgba(139,105,20,0.25)'};color:#f4e4c1;border:1px solid {t.color};">
        {t.name}
      </button>
    {/each}
  </div>
{/if}
