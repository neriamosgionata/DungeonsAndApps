<script lang="ts">
  import '../app.css';
  import '$lib/i18n';
  import favicon from '$lib/assets/favicon.svg';
  import NotifToasts from '$lib/components/NotifToasts.svelte';
  import { onDestroy, onMount } from 'svelte';
  import { notifications } from '$lib/notifications.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { Auth } from '$lib/api/resources';
  import { goto } from '$app/navigation';
  import { LogOut } from '@lucide/svelte';

  let { children } = $props();

  async function logout() {
    try { await Auth.logout(); } catch { /* ignore */ }
    auth.clear();
    goto('/login');
  }

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

{#if auth.authenticated}
  <div class="fixed top-0 right-0 z-50 p-3">
    <button onclick={logout}
      class="inline-flex items-center gap-1.5 rounded-md bg-neutral-900/80 border border-neutral-700 px-3 py-1.5 text-xs text-neutral-300 hover:text-white hover:border-neutral-500 backdrop-blur-sm transition-colors"
      title="logout">
      <LogOut size={13} />
      <span class="hidden sm:inline">logout</span>
    </button>
  </div>
{/if}

<main class="min-h-full text-neutral-100">
  {@render children()}
</main>

<NotifToasts />
