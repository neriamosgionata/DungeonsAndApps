<script lang="ts">
  import { page } from '$app/state';
  import { Factions } from '$lib/api/resources';
  import WorldList from '$lib/components/WorldList.svelte';
  import Paragraphs from '$lib/components/Paragraphs.svelte';
  const cid = $derived(page.params.id!);
</script>

<WorldList {cid} title="Factions" resource={Factions} wsPrefix="faction_"
  fields={[
    { key: 'name', label: 'Name' },
    { key: 'banner_color', label: 'Color (hex)' },
    { key: 'attitude', label: 'Attitude' },
    { key: 'description', label: 'Description', type: 'textarea' },
    { key: 'visibility', label: 'Visibility', type: 'select', options: ['private','players','public'] },
  ]}>
  {#snippet renderHeader(f)}
    <div class="flex items-center gap-3 min-w-0">
      {#if f.banner_color}
        <span class="h-6 w-6 rounded-full border border-amber-900 shrink-0"
          style="background: {f.banner_color}"></span>
      {/if}
      <div class="min-w-0">
        <div class="font-semibold truncate">{f.name}</div>
        {#if f.attitude}<div class="text-xs truncate" style="color:#8b6355;">{f.attitude}</div>{/if}
      </div>
    </div>
  {/snippet}
  {#snippet renderItem(f)}
    <Paragraphs text={f.description as string | undefined} emptyLabel="No description." />
  {/snippet}
</WorldList>
