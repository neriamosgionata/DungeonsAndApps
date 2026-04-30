<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Auth } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';

  let email = $state('');
  let password = $state('');
  let display_name = $state('');
  let language = $state<'en' | 'it'>('en');
  let error = $state('');
  let done = $state<string | null>(null);
  let busy = $state(false);

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    if (!auth.isAppAdmin) { goto('/campaigns'); return; }
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = ''; done = null; busy = true;
    try {
      const r = await Auth.register(email, password, display_name, language);
      done = r.user.email;
      email = ''; password = ''; display_name = '';
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
</script>

<header class="border-b border-neutral-800 bg-neutral-950 px-6 py-3 flex items-center gap-4">
  <a href="/campaigns" class="text-neutral-400 hover:text-neutral-200">←</a>
  <span class="font-bold text-violet-400">{$_('auth.invite_title')}</span>
</header>

<section class="mx-auto max-w-md px-6 py-10">
  <p class="text-sm text-neutral-400">{$_('auth.invite_explain')}</p>

  <form onsubmit={submit} class="mt-6 space-y-4">
    <label class="block"><span class="text-sm text-neutral-300">{$_('auth.display_name')}</span>
      <input required bind:value={display_name}
        class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" /></label>
    <label class="block"><span class="text-sm text-neutral-300">{$_('auth.email')}</span>
      <input type="email" required bind:value={email}
        class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" /></label>
    <label class="block"><span class="text-sm text-neutral-300">{$_('auth.password')}</span>
      <input type="password" required minlength="8" bind:value={password}
        class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" /></label>
    <label class="block"><span class="text-sm text-neutral-300">{$_('auth.language')}</span>
      <select bind:value={language} class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
        <option value="en">English</option>
        <option value="it">Italiano</option>
      </select></label>
    {#if error}<p class="text-sm text-red-400">{error}</p>{/if}
    {#if done}<p class="text-sm text-emerald-400">{$_('auth.invite_ok')}: {done}</p>{/if}
    <button type="submit" disabled={busy} class="w-full rounded-md bg-violet-600 py-2.5 font-medium text-white hover:bg-violet-500 disabled:opacity-50">
      {busy ? '…' : $_('auth.invite_submit')}
    </button>
  </form>
</section>
