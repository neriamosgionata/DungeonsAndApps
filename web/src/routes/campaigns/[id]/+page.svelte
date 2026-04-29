<script lang="ts">
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { onMount } from 'svelte';
  onMount(() => goto(`/campaigns/${page.params.id}/character`, { replaceState: true }));
</script>
