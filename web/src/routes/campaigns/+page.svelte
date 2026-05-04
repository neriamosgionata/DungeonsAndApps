<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { goto } from '$app/navigation';
  import { Auth, Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import CollapsibleAdd from '$lib/components/CollapsibleAdd.svelte';
  import SectionHeader from '$lib/components/SectionHeader.svelte';
  import EmptyState from '$lib/components/EmptyState.svelte';
  import ImageUpload from '$lib/components/ImageUpload.svelte';
  import type { Campaign } from '$lib/types';
  import { Crown, ShieldCheck, LogOut, Shield, UserPlus, Swords, ChevronRight, Search } from '@lucide/svelte';
  import NotifBell from '$lib/components/NotifBell.svelte';

  let items = $state<Campaign[]>([]);
  let name = $state('');
  let description = $state('');
  let iconUrl = $state<string | null>(null);
  let error = $state('');
  let campaignSearch = $state('');

  onMount(async () => {
    if (!auth.authenticated) { goto('/login'); return; }
    try { items = await Campaigns.list(); } catch (e) { error = (e as Error).message; }
  });

  async function create(close: () => void) {
    try {
      const c = await Campaigns.create(name, description || undefined, iconUrl);
      items = [c, ...items];
      name = ''; description = ''; iconUrl = null;
      close();
    } catch (e) { error = (e as Error).message; }
  }

  async function logout() {
    try { await Auth.logout(); } catch { /* ignore */ }
    auth.clear();
    goto('/');
  }

  // 3-char sigil generated from campaign name
  function sigil(s: string): string {
    return s.trim().split(/\s+/).map((w) => w[0]).join('').slice(0, 3).toUpperCase();
  }
</script>

<header class="border-b border-amber-900/40 bg-[#2a1d10] px-6 py-3 flex items-center justify-between">
  <a href="/campaigns" class="inline-flex items-center gap-2 font-display tracking-widest text-violet-400">
    <span class="app-mark">
      <svg viewBox="0 0 32 32" class="h-6 w-6" xmlns="http://www.w3.org/2000/svg" aria-hidden="true">
        <circle cx="16" cy="16" r="14" fill="#c9a84c" stroke="#4e3909" stroke-width="1"/>
        <path d="M7 20 q2-3 5-3 q1-3 3-3 l2-2 q2-2 5-1 q3 1 4 4 l-1 2 l2 2 v2 l-3 2 l-1-1 l-1 2 l-5 0 l-2-2 l-2 2 l-5-2 l-1 0 z" fill="#2c1810"/>
      </svg>
    </span>
    CINGHIALAPP
  </a>
  <div class="text-sm text-neutral-400 flex items-center gap-3">
    <a href="/invitations"
      class="inline-flex items-center gap-1 rounded bg-violet-600/20 text-violet-300 px-2 py-0.5 hover:bg-violet-600/40">
      {$_('invitations.title')}
    </a>
    {#if auth.isAdmin}
      <a href="/master/users"
        class="inline-flex items-center gap-1 rounded bg-violet-600/20 text-violet-300 px-2 py-0.5 hover:bg-violet-600/40">
        <Shield size={14} /> {$_('users.title')}
      </a>
    {/if}
    <NotifBell />
    <span class="inline-flex items-center gap-2">
      {#if auth.isAdmin}
        <ShieldCheck size={14} class="text-sky-300" />
        <span class="rounded-full border px-2 py-0.5 text-[10px] tracking-widest uppercase"
          style="border-color:#2f6058; color:#a8d4cb; background:linear-gradient(180deg,rgba(111,160,154,0.2),rgba(47,96,88,0.15));">
          Administrator
        </span>
      {/if}
      {auth.user?.display_name}
    </span>
    <button class="inline-flex items-center gap-1 text-violet-400 hover:text-violet-300" onclick={logout}>
      <LogOut size={14} /> logout
    </button>
  </div>
</header>

<section class="page-panel">
  <SectionHeader title={$_('campaigns.title')} subtitle={$_('campaigns.subtitle')}>
    {#snippet icon()}<Swords size={18} />{/snippet}
    {#snippet actions()}
      {#if auth.authenticated}
        <CollapsibleAdd label={$_('campaigns.new')} title={$_('campaigns.new')} alignEnd={false}>
          {#snippet children({ close })}
            <form onsubmit={(e) => { e.preventDefault(); create(close); }} class="space-y-3">
              <ImageUpload bind:value={iconUrl} kind="campaign" size={72} label={$_('campaigns.icon')} />
              <input required placeholder={$_('campaigns.new_name')} bind:value={name}
                class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2" />
              <textarea placeholder={$_('campaigns.new_desc')} bind:value={description} rows="5"
                class="w-full rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 resize-y min-h-32"></textarea>
              <div class="flex justify-end">
                <button class="rounded-md bg-violet-600 px-6 py-2 text-white">{$_('campaigns.create')}</button>
              </div>
            </form>
          {/snippet}
        </CollapsibleAdd>
      {/if}
    {/snippet}
  </SectionHeader>

  {#if error}<p class="mt-4 text-sm text-red-400">{error}</p>{/if}

  <div class="mt-3 flex items-center gap-2">
    <Search size={14} class="text-neutral-500 shrink-0" />
    <input placeholder={$_('common.search')} bind:value={campaignSearch}
      class="w-full max-w-sm rounded-md bg-neutral-900 border border-neutral-700 px-3 py-2 text-sm" />
  </div>
  {#if items.filter((c) => {
    const q = campaignSearch.trim().toLowerCase();
    return !q || c.name.toLowerCase().includes(q);
  }).length === 0}
    <EmptyState title={$_('campaigns.empty')} hint={$_('campaigns.hint_master')} />
  {:else}
    <ul class="mt-2 grid gap-4 sm:grid-cols-2">
      {#each items.filter((c) => {
        const q = campaignSearch.trim().toLowerCase();
        return !q || c.name.toLowerCase().includes(q);
      }) as c (c.id)}
        <li>
          <a href="/campaigns/{c.id}" class="campaign-card group">
            {#if c.icon_url}
              <img src={c.icon_url} alt="" class="sigil-img" />
            {:else}
              <div class="sigil">{sigil(c.name)}</div>
            {/if}
            <div class="body">
              <div class="name">{c.name}</div>
              {#if c.description}<div class="desc">{c.description}</div>{/if}
            </div>
            <span class="arrow"><ChevronRight size={18} /></span>
          </a>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .app-mark {
    display: inline-grid; place-items: center;
    filter: drop-shadow(0 1px 2px rgba(0,0,0,0.6));
  }

  .campaign-card {
    position: relative;
    display: flex; align-items: center; gap: 1rem;
    padding: 1rem;
    border-radius: 0.5rem;
    border: 1.5px solid #4e3909;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='400' height='400'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.07 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 4px 8px rgba(0,0,0,0.5);
    transition: transform 0.15s, box-shadow 0.15s, border-color 0.15s;
  }
  .campaign-card:hover {
    transform: translateY(-2px);
    border-color: #c9a84c;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.6), 0 8px 18px rgba(0,0,0,0.7), 0 0 16px rgba(201,168,76,0.15);
  }
  .sigil {
    flex: none;
    width: 3rem; height: 3rem;
    display: grid; place-items: center;
    border-radius: 9999px;
    background: radial-gradient(circle at 35% 30%, #f7e2a5 0%, #c9a84c 45%, #6d510f 100%);
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 1px 3px rgba(0,0,0,0.6);
    font-family: 'Cinzel', serif;
    font-weight: 900;
    color: #1a0f08;
    letter-spacing: 0.03em;
    font-size: 0.9rem;
  }
  .sigil-img {
    flex: none;
    width: 3rem; height: 3rem;
    border-radius: 9999px;
    object-fit: cover;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 1px 3px rgba(0,0,0,0.6);
  }
  .body { flex: 1; min-width: 0; }
  .name {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 1.15rem;
    letter-spacing: 0.03em;
    color: #2c1810;
  }
  .desc {
    margin-top: 0.25rem;
    font-family: 'Crimson Text', serif;
    font-style: italic;
    color: #5c3d2e;
    overflow: hidden;
    text-overflow: ellipsis;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }
  .arrow { color: #8b6914; transition: transform 0.15s; }
  .campaign-card:hover .arrow { transform: translateX(3px); color: #c9a84c; }
</style>
