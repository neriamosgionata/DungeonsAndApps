<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount, onDestroy } from 'svelte';
  import { page } from '$app/state';
  import { goto } from '$app/navigation';
  import { Campaigns } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import { auth } from '$lib/stores/auth.svelte';
  import { provideCampaign } from '$lib/campaignCtx.svelte';
  import type { Campaign } from '$lib/types';
  import {
    ArrowLeft, Circle, CircleDot, Crown, ShieldCheck,
    UserRound, ScrollText, Map, Users, Flag, BookOpen, Newspaper,
    Sparkles, Coins, MessagesSquare, Dices, Swords, UserPlus,
  } from '@lucide/svelte';
  import NotifBell from '$lib/components/NotifBell.svelte';
  import PresenceIndicator from '$lib/components/PresenceIndicator.svelte';

  const iconOf: Record<string, typeof UserRound> = {
    character: UserRound, recap: ScrollText, map: Map, npcs: Users,
    factions: Flag, lore: BookOpen, news: Newspaper, spells: Sparkles,
    group: Coins, messages: MessagesSquare, dice: Dices, initiative: Swords,
    members: UserPlus,
  };

  let { children } = $props();
  const id = $derived(page.params.id!);

  let campaign = $state<Campaign | null>(null);
  let isMaster = $state(false);
  let error = $state('');

  provideCampaign(() => ({
    isMaster,
    campaignId: id,
    leveling: (campaign?.leveling ?? 'xp') as 'xp' | 'milestone',
  }));

  const sections = $derived([
    { slug: 'character',  key: 'nav.character'  },
    { slug: 'recap',      key: 'nav.recap'      },
    { slug: 'map',        key: 'nav.map'        },
    { slug: 'npcs',       key: 'nav.npcs'       },
    { slug: 'factions',   key: 'nav.factions'   },
    { slug: 'lore',       key: 'nav.lore'       },
    { slug: 'news',       key: 'nav.news'       },
    { slug: 'spells',     key: 'nav.spells'     },
    { slug: 'group',      key: 'nav.group'      },
    { slug: 'messages',   key: 'nav.messages'   },
    { slug: 'dice',       key: 'nav.dice'       },
    { slug: 'initiative', key: 'nav.initiative' },
    ...(isMaster ? [{ slug: 'members', key: 'nav.members' }] : []),
  ]);

  $effect(() => {
    if (!auth.authenticated) { goto('/login'); return; }
    // connect immediately so live updates flow across all sub-routes
    campaignSocket.connect(id);
    (async () => {
      try {
        campaign = await Campaigns.get(id);
        isMaster = campaign.master_id === auth.user?.id || auth.isAdmin;
      } catch (e) { error = (e as Error).message; }
    })();

    const off = campaignSocket.on(async (ev) => {
      if (ev.type === 'campaign_updated') {
        try { campaign = await Campaigns.get(id); } catch { /* ignore */ }
      }
    });
    return off;
  });

  onDestroy(() => campaignSocket.disconnect());
</script>

<header class="campaign-banner">
  <a href="/campaigns" aria-label="back" class="back-btn"><ArrowLeft size={18} /></a>
  {#if campaign?.icon_url}
    <img src={campaign.icon_url} alt="" class="banner-icon" />
  {/if}
  <div class="banner-body">
    <a href="/campaigns/{id}" class="banner-title">{campaign?.name ?? '…'}</a>
    <div class="banner-meta">
      <span class="meta-live {campaignSocket.connected ? 'on' : 'off'}">
        {#if campaignSocket.connected}<CircleDot size={12} />{:else}<Circle size={12} />{/if}
        {campaignSocket.connected ? 'live' : 'offline'}
      </span>
      {#if campaign}
        <span class="leveling-toggle" title="Campaign leveling">
          <span class="tl">Leveling:</span>
          {#if isMaster}
            {#each ['xp','milestone'] as m, i (m)}
              {#if i > 0}<span class="sep">/</span>{/if}
              <button type="button"
                class="lv-opt {(campaign.leveling ?? 'xp') === m ? 'active' : ''}"
                onclick={async () => {
                  if ((campaign?.leveling ?? 'xp') === m) return;
                  try { campaign = await Campaigns.update(id, { leveling: m as 'xp' | 'milestone' }); }
                  catch (err) { error = (err as Error).message; }
                }}>
                {m === 'xp' ? 'XP' : 'Milestone'}
              </button>
            {/each}
          {:else}
            <span class="lv-opt active">
              {(campaign.leveling ?? 'xp') === 'xp' ? 'XP' : 'Milestone'}
            </span>
          {/if}
        </span>
      {/if}
      {#if campaign?.description}
        <span class="meta-desc">{campaign.description}</span>
      {/if}
    </div>
  </div>
  {#if isMaster}<PresenceIndicator cid={id} />{/if}
  <NotifBell />
  <div class="banner-user">
    {#if auth.isAdmin}
      <ShieldCheck size={14} class="text-sky-300" />
      <span class="role-badge role-admin">Administrator</span>
    {:else if isMaster}
      <Crown size={14} class="text-amber-400" />
      <span class="role-badge">Game Master</span>
    {:else}
      <span class="role-badge role-player">Player</span>
    {/if}
    <span>{auth.user?.display_name}</span>
  </div>
</header>

<nav class="campaign-tabs">
  <ul>
    {#each sections as s (s.slug)}
      {@const Icon = iconOf[s.slug]}
      {@const active = page.url.pathname.includes(`/${s.slug}`)}
      <li>
        <a href="/campaigns/{id}/{s.slug}" class="tab {active ? 'active' : ''}">
          {#if Icon}<Icon size={16} />{/if}
          <span>{$_(s.key)}</span>
        </a>
      </li>
    {/each}
  </ul>
</nav>

<style>
  .campaign-banner {
    position: relative;
    display: flex; align-items: center; gap: 1rem;
    padding: 0.75rem 1.5rem;
    border-bottom: 1px solid #4e3909;
    background:
      linear-gradient(180deg, rgba(201, 168, 76, 0.14), transparent 60%),
      #2a1d10;
    box-shadow: inset 0 -1px 0 rgba(201, 168, 76, 0.25), 0 4px 12px rgba(0,0,0,0.5);
  }
  .campaign-banner::before,
  .campaign-banner::after {
    content: ""; position: absolute; bottom: -1px;
    width: 40px; height: 2px;
    background: radial-gradient(ellipse, #c9a84c 0%, transparent 70%);
  }
  .campaign-banner::before { left: 20%; }
  .campaign-banner::after  { right: 20%; }

  .back-btn {
    display: grid; place-items: center;
    width: 2rem; height: 2rem;
    border-radius: 9999px;
    border: 1px solid #4e3909;
    background: linear-gradient(180deg, #3a2313, #1a0f08);
    color: #c9a84c;
  }
  .back-btn:hover { background: linear-gradient(180deg, #4e3909, #2c1810); color: #f7e2a5; }

  .banner-icon {
    width: 2.5rem; height: 2.5rem;
    border-radius: 9999px;
    object-fit: cover;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.4), 0 1px 3px rgba(0,0,0,0.6);
  }

  .banner-body { flex: 1; min-width: 0; }
  .banner-title {
    font-family: 'Cinzel', serif;
    font-weight: 700;
    font-size: 1.375rem;
    letter-spacing: 0.05em;
    color: #f7e2a5;
    text-shadow: 0 1px 0 rgba(0,0,0,0.7);
  }
  .banner-meta {
    display: flex; align-items: center; gap: 0.75rem;
    font-size: 0.75rem; color: #b59a78;
    margin-top: 0.125rem;
  }
  .meta-live { display: inline-flex; align-items: center; gap: 0.25rem; }
  .meta-live.on  { color: #6b8a4f; }
  .meta-live.off { color: #8b6355; }
  .leveling-toggle {
    display: inline-flex; align-items: center; gap: 0.3rem;
    color: #c9a84c;
    font-family: 'Cinzel', serif;
    letter-spacing: 0.03em;
    font-size: 0.7rem;
  }
  .leveling-toggle .tl { opacity: 0.8; }
  .leveling-toggle .sep { opacity: 0.5; }
  .lv-opt {
    padding: 0.05rem 0.35rem;
    border-radius: 0.25rem;
    background: transparent;
    color: #b59a78;
    border: 1px solid transparent;
  }
  .lv-opt.active {
    background: linear-gradient(180deg, #c9a84c 0%, #8b6914 60%, #6d510f 100%);
    color: #1a0f08;
    border-color: #4e3909;
    font-weight: 700;
  }
  button.lv-opt:hover:not(.active) { color: #f4d97a; }
  .meta-desc {
    font-style: italic;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 40ch;
  }

  .banner-user {
    display: inline-flex; align-items: center; gap: 0.5rem;
    font-family: 'Cinzel', serif;
    letter-spacing: 0.04em;
    font-size: 0.85rem;
    color: #d4b896;
  }
  .role-badge {
    font-size: 0.65rem;
    letter-spacing: 0.15em;
    text-transform: uppercase;
    padding: 0.1rem 0.5rem;
    border-radius: 9999px;
    border: 1px solid #4e3909;
    background: linear-gradient(180deg, rgba(201,168,76,0.15), rgba(109,81,15,0.1));
    color: #f7e2a5;
  }
  .role-badge.role-player {
    background: linear-gradient(180deg, rgba(139,99,85,0.2), rgba(92,61,46,0.15));
    color: #d4b896;
  }
  .role-badge.role-admin {
    background: linear-gradient(180deg, rgba(111,160,154,0.2), rgba(47,96,88,0.15));
    border-color: #2f6058;
    color: #a8d4cb;
  }

  .campaign-tabs {
    position: relative;
    overflow-x: auto;
    border-bottom: 1px solid #4e3909;
    background: #241810;
  }
  .campaign-tabs ul { display: flex; padding: 0 1rem; gap: 0.25rem; }
  .tab {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.625rem 0.9rem;
    white-space: nowrap;
    font-family: 'Cinzel', serif;
    font-size: 0.8rem;
    letter-spacing: 0.06em;
    color: #b59a78;
    border-bottom: 2px solid transparent;
    transition: color 0.15s, border-color 0.15s, background 0.15s;
  }
  .tab:hover { color: #f7e2a5; background: linear-gradient(180deg, rgba(201,168,76,0.08), transparent); }
  .tab.active {
    color: #f7e2a5;
    border-bottom-color: #c9a84c;
    background: linear-gradient(180deg, rgba(201,168,76,0.12), transparent);
    text-shadow: 0 0 8px rgba(201,168,76,0.35);
  }
</style>

{#if error}<p class="m-4 text-sm text-red-400">{error}</p>{/if}
<div class="page-panel">
  {@render children()}
</div>
