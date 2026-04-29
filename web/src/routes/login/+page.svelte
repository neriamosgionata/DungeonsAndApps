<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { goto } from '$app/navigation';
  import { Auth } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';

  let email = $state('');
  let password = $state('');
  let error = $state('');
  let busy = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    error = ''; busy = true;
    try {
      const r = await Auth.login(email, password);
      auth.set(r.token, r.user);
      goto('/campaigns');
    } catch (e) {
      error = (e as Error).message;
    } finally { busy = false; }
  }
</script>

<section class="mx-auto max-w-md px-6 py-16">
  <h1 class="text-3xl font-bold text-violet-400">{$_('auth.login')}</h1>
  <form onsubmit={submit} class="mt-8 space-y-4">
    <label class="block">
      <span class="text-sm text-neutral-300">{$_('auth.email')}</span>
      <input type="email" required bind:value={email}
        class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 text-neutral-100" />
    </label>
    <label class="block">
      <span class="text-sm text-neutral-300">{$_('auth.password')}</span>
      <input type="password" required minlength="8" bind:value={password}
        class="mt-1 block w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 text-neutral-100" />
    </label>
    {#if error}<p class="text-sm text-red-400">{error}</p>{/if}
    <button type="submit" disabled={busy}
      class="w-full rounded-md bg-violet-600 py-2.5 font-medium text-white hover:bg-violet-500 disabled:opacity-50">
      {busy ? '…' : $_('auth.login')}
    </button>
    <p class="text-sm text-neutral-400">
      {$_('auth.no_account')} <a class="underline" style="color:#c9a84c;" href="/register">{$_('auth.register')}</a>
    </p>
  </form>
</section>
