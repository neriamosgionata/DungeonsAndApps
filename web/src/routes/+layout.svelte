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

<main class="min-h-full text-neutral-100 pb-4">
  {@render children()}
</main>

<NotifToasts />

<footer class="credits-bar">By Deborah Betteto</footer>

<style>
  .credits-bar {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    height: 1rem;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.55rem;
    letter-spacing: 0.06em;
    color: rgba(180, 154, 120, 0.45);
    background: linear-gradient(180deg, transparent, rgba(26, 15, 8, 0.92) 40%);
    pointer-events: none;
    z-index: 100;
  }
</style>
