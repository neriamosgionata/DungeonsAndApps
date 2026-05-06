<script lang="ts">
  import '../app.css';
  import '$lib/i18n';
  import favicon from '$lib/assets/favicon.svg';
  import NotifToasts from '$lib/components/NotifToasts.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { notifications } from '$lib/notifications.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { goto } from '$app/navigation';

  let { children } = $props();

  onMount(() => {
    if (auth.token) { notifications.connect(); notifications.refresh(); }
  });
  $effect(() => {
    if (auth.token) { notifications.connect(); notifications.refresh(); }
    else notifications.disconnect();
  });
  onDestroy(() => notifications.disconnect());
</script>

<svelte:head>
  <link rel="icon" href={favicon} />
  <title>DungeonsAndApps</title>
</svelte:head>

<main class="min-h-full text-neutral-100">
  {@render children()}
</main>

<NotifToasts />
