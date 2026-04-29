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
  let busy = $state(false);
  let allowed = $state<boolean | null>(null);

  onMount(async () => {
    try {
      const s = await Auth.bootstrapStatus();
      allowed = s.needs_bootstrap;
      if (!s.needs_bootstrap) {
        // already bootstrapped — kick back to login
        goto('/login');
      }
    } catch (e) {
      error = (e as Error).message;
    }
  });

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = ''; busy = true;
    try {
      const r = await Auth.register(email, password, display_name, language);
      auth.set(r.token, r.user);
      goto('/campaigns');
    } catch (e) { error = (e as Error).message; } finally { busy = false; }
  }
</script>

<section class="mx-auto max-w-md px-6 py-16">
  <h1 class="text-3xl font-bold text-amber-300">{$_('auth.bootstrap')}</h1>
  <p class="mt-2 text-sm text-neutral-400">{$_('auth.bootstrap_explain')}</p>

  {#if allowed === false}
    <p class="mt-6 text-sm text-red-400">{$_('auth.bootstrap_done')}</p>
  {:else if allowed === true}
    <form onsubmit={submit} class="mt-8 space-y-4">
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
      <button type="submit" disabled={busy} class="w-full rounded-md bg-amber-500 py-2.5 font-semibold text-black hover:bg-amber-400 disabled:opacity-50">
        {busy ? '…' : $_('auth.create_master')}
      </button>
    </form>
  {/if}
</section>
