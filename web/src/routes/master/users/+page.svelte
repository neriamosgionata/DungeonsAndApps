<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Users } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { ArrowLeft, UserPlus, KeyRound, Trash2 } from '@lucide/svelte';

  type User = { id: string; email: string; display_name: string; role: string; language: string; created_at: string };
  let users = $state<User[]>([]);
  let error = $state('');

  async function load() {
    try { users = await Users.list(); } catch (e) { error = (e as Error).message; }
  }
  onMount(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    if (!auth.isMaster) { goto('/campaigns'); return; }
    load();
  });

  async function patch(u: User, patch: { role?: 'user' | 'admin'; display_name?: string; language?: 'en' | 'it' }) {
    try { await Users.update(u.id, patch); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function remove(u: User) {
    if (!confirm($_('users.delete_confirm').replace('{{name}}', u.display_name))) return;
    try { await Users.delete(u.id); await load(); }
    catch (e) { error = (e as Error).message; }
  }

  async function reset(u: User) {
    const pw = prompt($_('users.reset_prompt').replace('{{name}}', u.display_name));
    if (!pw) return;
    if (pw.length < 8) { alert($_('users.reset_short')); return; }
    try {
      await Users.resetPassword(u.id, pw);
      alert($_('users.reset_ok'));
    } catch (e) { error = (e as Error).message; }
  }
</script>

<header class="border-b border-neutral-800 bg-neutral-950 px-6 py-3 flex items-center gap-4">
  <a href="/campaigns" class="text-neutral-400 hover:text-neutral-200"><ArrowLeft size={18} /></a>
  <span class="font-bold text-violet-400">{$_('users.title')}</span>
  <a href="/master/invite" class="ml-auto inline-flex items-center gap-1.5 rounded bg-violet-600 px-3 py-1 text-sm">
    <UserPlus size={14} /> {$_('auth.invite_submit')}
  </a>
</header>

<section class="page-panel">
  <p class="text-sm text-neutral-400">{$_('users.explain')}</p>
  {#if error}<p class="mt-3 text-sm text-red-400">{error}</p>{/if}

  <ul class="mt-6 space-y-2">
    {#each users as u (u.id)}
      {@const isSelf = u.id === auth.user?.id}
      <li class="rounded-lg border border-neutral-800 bg-neutral-900 p-4">
        <div class="flex flex-wrap items-center gap-3">
          <div class="flex-1 min-w-48">
            <div class="font-semibold">
              {u.display_name}
              {#if isSelf}<span class="ml-1 text-xs text-neutral-500">({$_('users.you')})</span>{/if}
            </div>
            <div class="text-xs text-neutral-400">{u.email}</div>
          </div>

          <select value={u.role} disabled={isSelf}
            onchange={(e) => patch(u, { role: (e.currentTarget as HTMLSelectElement).value as 'user' | 'admin' })}
            class="rounded bg-neutral-800 border border-neutral-700 px-2 py-1 text-sm">
            <option value="user">{$_('users.role_user')}</option>
            <option value="admin">{$_('users.role_admin')}</option>
          </select>

          <select value={u.language}
            onchange={(e) => patch(u, { language: (e.currentTarget as HTMLSelectElement).value as 'en' | 'it' })}
            class="rounded bg-neutral-800 border border-neutral-700 px-2 py-1 text-sm">
            <option value="en">EN</option>
            <option value="it">IT</option>
          </select>

          <button onclick={() => reset(u)}
            class="inline-flex items-center gap-1.5 rounded bg-neutral-800 text-neutral-50 px-3 py-1 text-sm hover:bg-neutral-700">
            <KeyRound size={14} /> {$_('users.reset_password')}
          </button>

          {#if !isSelf}
            <button onclick={() => remove(u)}
              class="inline-flex items-center gap-1.5 rounded bg-red-600 px-3 py-1 text-sm text-white">
              <Trash2 size={14} /> {$_('users.delete')}
            </button>
          {/if}
        </div>
      </li>
    {/each}
    {#if users.length === 0}<li class="text-neutral-500 italic">{$_('users.empty')}</li>{/if}
  </ul>
</section>
