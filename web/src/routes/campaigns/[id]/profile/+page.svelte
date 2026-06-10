<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Auth } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { UserRound, Lock } from '@lucide/svelte';

  let displayName = $state('');
  let language = $state<'en' | 'it'>('en');
  let error = $state('');
  let ok = $state('');
  let busy = $state(false);

  let currentPassword = $state('');
  let newPassword = $state('');
  let confirmPassword = $state('');
  let pwError = $state('');
  let pwOk = $state('');
  let pwBusy = $state(false);

  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    displayName = auth.user?.display_name ?? '';
    language = (auth.user?.language as 'en' | 'it') ?? 'en';
  });

  async function saveProfile() {
    error = ''; ok = ''; busy = true;
    try {
      const user = await Auth.updateMe({
        display_name: displayName.trim(),
        language,
      });
      auth.set(auth.token!, user);
      ok = $_('profile.save_ok');
    } catch (e) { error = (e as Error).message; }
    finally { busy = false; }
  }

  async function changePassword() {
    pwError = ''; pwOk = ''; pwBusy = true;
    if (newPassword !== confirmPassword) {
      pwError = $_('profile.password_mismatch');
      pwBusy = false;
      return;
    }
    if (newPassword.length < 8) {
      pwError = $_('profile.password_weak');
      pwBusy = false;
      return;
    }
    try {
      await Auth.changePassword(currentPassword, newPassword);
      pwOk = $_('profile.password_ok');
      currentPassword = ''; newPassword = ''; confirmPassword = '';
    } catch (e) { pwError = (e as Error).message; }
    finally { pwBusy = false; }
  }
</script>

<section class="mx-auto max-w-3xl px-3 sm:px-6 py-6">
  <h2 class="text-xl font-semibold flex items-center gap-2">
    <UserRound size={20} />
    {$_('profile.title')}
  </h2>

  {#if error}<p class="mt-3 text-sm text-red-600">{error}</p>{/if}
  {#if ok}<p class="mt-3 text-sm text-emerald-600">{ok}</p>{/if}

  <div class="mt-6 space-y-4">
    <div>
      <label for="pf-email" class="block text-sm text-neutral-400 mb-1">{$_('profile.email')}</label>
      <input id="pf-email" value={auth.user?.email ?? ''} disabled
        class="w-full rounded-md bg-neutral-200 border border-neutral-300 px-3 py-2 text-neutral-500 cursor-not-allowed" />
    </div>

    <div>
      <label for="pf-name" class="block text-sm text-neutral-400 mb-1">{$_('profile.display_name')}</label>
      <input id="pf-name" bind:value={displayName} maxlength="64" placeholder={$_('profile.display_name_ph')}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    </div>

    <div>
      <label for="pf-lang" class="block text-sm text-neutral-400 mb-1">{$_('profile.language')}</label>
      <select id="pf-lang" bind:value={language}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2">
        <option value="en">English</option>
        <option value="it">Italiano</option>
      </select>
    </div>

    <button onclick={saveProfile} disabled={busy || !displayName.trim()}
      class="rounded-md bg-amber-500 px-4 py-2 font-semibold text-black hover:bg-amber-400 disabled:opacity-50">
      {busy ? '…' : $_('common.save')}
    </button>
  </div>

  <hr class="my-8 border-neutral-200" />

  <h3 class="text-lg font-semibold flex items-center gap-2">
    <Lock size={18} />
    {$_('profile.change_password')}
  </h3>

  {#if pwError}<p class="mt-3 text-sm text-red-600">{pwError}</p>{/if}
  {#if pwOk}<p class="mt-3 text-sm text-emerald-600">{pwOk}</p>{/if}

  <div class="mt-4 space-y-4">
    <div>
      <label for="pf-cp" class="block text-sm text-neutral-400 mb-1">{$_('profile.current_password')}</label>
      <input id="pf-cp" type="password" bind:value={currentPassword}
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    </div>
    <div>
      <label for="pf-np" class="block text-sm text-neutral-400 mb-1">{$_('profile.new_password')}</label>
      <input id="pf-np" type="password" bind:value={newPassword} minlength="8"
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    </div>
    <div>
      <label for="pf-cnp" class="block text-sm text-neutral-400 mb-1">{$_('profile.confirm_password')}</label>
      <input id="pf-cnp" type="password" bind:value={confirmPassword} minlength="8"
        class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
    </div>

    <button onclick={changePassword} disabled={pwBusy || !currentPassword || !newPassword}
      class="rounded-md bg-amber-500 px-4 py-2 font-semibold text-black hover:bg-amber-400 disabled:opacity-50">
      {pwBusy ? '…' : $_('profile.change_password')}
    </button>
  </div>
</section>
