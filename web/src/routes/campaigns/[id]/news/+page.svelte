<script lang="ts">
  import { page } from '$app/state';
  import { News } from '$lib/api/resources';
  import WorldList from '$lib/components/WorldList.svelte';
  const cid = $derived(page.params.id!);
</script>

<WorldList {cid} title="News" resource={News} wsPrefix="news_"
  fields={[
    { key: 'title', label: 'Title' },
    { key: 'body', label: 'Body', type: 'textarea' },
    { key: 'visibility', label: 'Visibility', type: 'select', options: ['private','players','public'] },
  ]}>
  {#snippet renderItem(n)}
    <div class="font-semibold">{n.title}</div>
    {#if n.published_at}<div class="text-xs text-neutral-500">{new Date(n.published_at as string).toLocaleString()}</div>{/if}
    <p class="mt-1 text-sm text-neutral-300 whitespace-pre-wrap">{n.body}</p>
  {/snippet}
</WorldList>
