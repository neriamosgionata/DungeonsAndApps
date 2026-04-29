<script lang="ts">
  import { page } from '$app/state';
  import { Lore } from '$lib/api/resources';
  import WorldList from '$lib/components/WorldList.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  const cid = $derived(page.params.id!);
</script>

<WorldList {cid} title="Lore" resource={Lore} wsPrefix="lore_"
  fields={[
    { key: 'title', label: 'Title' },
    { key: 'category', label: 'Category' },
    { key: 'body', label: 'Body', type: 'textarea' },
    { key: 'visibility', label: 'Visibility', type: 'select', options: ['private','players','public'] },
  ]}>
  {#snippet renderHeader(l)}
    <div class="min-w-0">
      <div class="font-semibold truncate">{l.title}</div>
      {#if l.category}<div class="text-xs truncate" style="color:#8b6355;">{l.category}</div>{/if}
    </div>
  {/snippet}
  {#snippet renderItem(l)}
    <Paragraphs text={l.body as string | undefined} emptyLabel="Empty." />
  {/snippet}
</WorldList>
