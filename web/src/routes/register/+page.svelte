<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';
  import { Auth } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';

  let email = $state('');
  let password = $state('');
  let display_name = $state('');
  let language = $state<'en' | 'it'>('en');
  let error = $state('');
  let busy = $state(false);

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
  <h1 class="text-3xl font-bold" style="color:#c9a84c;">{$_('auth.register')}</h1>
  <p class="mt-2 text-sm" style="color:#d4b896;">{$_('auth.register_hint')}</p>
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
    <button type="submit" disabled={busy} class="w-full rounded-md bg-violet-600 py-2.5 font-medium text-white disabled:opacity-50">
      {busy ? '…' : $_('auth.register')}
    </button>
    <p class="text-sm text-neutral-400">
      {$_('auth.have_account')} <a class="underline" style="color:#c9a84c;" href="/login">{$_('auth.login')}</a>
    </p>
  </form>
</section>
