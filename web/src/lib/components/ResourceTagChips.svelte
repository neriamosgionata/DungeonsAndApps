<script lang="ts">
  import { onMount } from 'svelte';
  import { Tags } from '$lib/api/resources';

  // Per-resource tag chips. Masters toggle; players see badges read-only.
  let {
    cid,
    resourceType,
    resourceId,
    tagIds = $bindable<string[]>([]),
    isMaster = false,
  }: {
    cid: string;
    resourceType: string;
    resourceId: string;
    tagIds?: string[];
    isMaster?: boolean;
  } = $props();

  let tags = $state<Array<{ id: string; name: string; color: string }>>([]);

  onMount(async () => {
    try { tags = (await Tags.list(cid)).tags; } catch { tags = []; }
  });

  async function toggle(tagId: string) {
    if (!isMaster) return;
    const has = tagIds.includes(tagId);
    try {
      if (has) {
        await Tags.remove(cid, tagId, resourceType, resourceId);
        tagIds = tagIds.filter((t) => t !== tagId);
      } else {
        await Tags.apply(cid, tagId, resourceType, resourceId);
        tagIds = [...tagIds, tagId];
      }
    } catch { /* best-effort */ }
  }
</script>

{#if tags.length}
  <div class="flex flex-wrap gap-1">
    {#each tags as t (t.id)}
      <button type="button" onclick={() => toggle(t.id)} disabled={!isMaster}
        class="rounded px-1.5 py-0.5 text-[10px]"
        style="background:{tagIds.includes(t.id) ? t.color : 'transparent'};color:#f4e4c1;border:1px solid {t.color};{isMaster ? 'cursor:pointer' : 'cursor:default'}">
        {t.name}
      </button>
    {/each}
  </div>
{/if}
