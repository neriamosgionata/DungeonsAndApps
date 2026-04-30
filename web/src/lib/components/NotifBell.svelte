<script lang="ts">
  import { _ } from 'svelte-i18n';
  import { onMount } from 'svelte';
  import { notifications } from '$lib/notifications.svelte';
  import { Bell, Check, Trash2, X } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { getNotifAction } from '$lib/notifActions';

  let running = $state<Record<string, boolean>>({});

  let open = $state(false);
  let panel: HTMLDivElement | undefined = $state();
  let btn: HTMLButtonElement | undefined = $state();

  onMount(() => {
    const click = (e: MouseEvent) => {
      if (!open) return;
      const t = e.target as Node;
      if (panel?.contains(t) || btn?.contains(t)) return;
      open = false;
    };
    window.addEventListener('mousedown', click);
    return () => window.removeEventListener('mousedown', click);
  });

  function fmtTime(iso: string): string {
    const d = new Date(iso);
    const diff = (Date.now() - d.getTime()) / 1000;
    if (diff < 60)      return `${Math.round(diff)}s`;
    if (diff < 3600)    return `${Math.round(diff / 60)}m`;
    if (diff < 86400)   return `${Math.round(diff / 3600)}h`;
    return d.toLocaleDateString();
  }

  function targetUrl(n: typeof notifications.items[number]): string | null {
    if (!n.campaign_id) return null;
    switch (n.ref_kind) {
      case 'message':   return `/campaigns/${n.campaign_id}/messages`;
      case 'whisper':   return n.ref_id
        ? `/campaigns/${n.campaign_id}/messages?whisper=${n.ref_id}`
        : `/campaigns/${n.campaign_id}/messages`;
      case 'encounter': return `/campaigns/${n.campaign_id}/initiative`;
      case 'news':      return `/campaigns/${n.campaign_id}/news`;
      case 'invitation': return '/invitations';
      case 'campaign':  return `/campaigns/${n.campaign_id}`;
      default: return `/campaigns/${n.campaign_id}`;
    }
  }

  async function openNotif(n: typeof notifications.items[number]) {
    await notifications.markRead(n.id);
    const url = targetUrl(n);
    if (url) { open = false; goto(url); }
  }

  async function runAction(n: typeof notifications.items[number], e: Event) {
    e.stopPropagation();
    const action = getNotifAction(n);
    if (!action) return;
    running[n.id] = true;
    try {
      const keep = await action.run(n, {
        dismiss: () => {},
        markRead: () => notifications.markRead(n.id),
      });
      if (keep !== false) {
        await notifications.markRead(n.id);
        open = false;
      }
    } catch (err) {
      console.error('notification action failed', err);
      alert(`${getNotifAction(n)?.label ?? 'Action'} failed: ${(err as Error).message}`);
    } finally {
      running[n.id] = false;
    }
  }
</script>

<div class="bell-wrap">
  <button bind:this={btn} onclick={() => open = !open} aria-label="notifications"
    class="bell-btn {notifications.unread > 0 ? 'ring' : ''}">
    <span class="icon {notifications.unread > 0 ? 'shake' : ''}"><Bell size={16} /></span>
    {#if notifications.unread > 0}
      <span class="badge">{notifications.unread > 99 ? '99+' : notifications.unread}</span>
    {/if}
  </button>

  {#if open}
    <div bind:this={panel} class="panel" role="dialog">
      <header class="head">
        <span>Notifications</span>
        <div class="tools">
          {#if notifications.unread > 0}
            <button onclick={() => notifications.markAllRead()} title="mark all read"><Check size={14} /></button>
          {/if}
          {#if notifications.items.length > 0}
            <button onclick={() => { if (confirm($_('notifications.clear_all_confirm'))) notifications.clearAll(); }}
              title="clear all"><Trash2 size={14} /></button>
          {/if}
          <button onclick={() => (open = false)} title="close"><X size={14} /></button>
        </div>
      </header>
      <div class="list">
        {#each notifications.items as n (n.id)}
          {@const action = getNotifAction(n)}
          <div class="row {n.read_at ? 'read' : 'unread'}">
            <div class="body" role="button" tabindex="0"
              onclick={() => openNotif(n)}
              onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && openNotif(n)}>
              <div class="title">{n.title}</div>
              {#if n.body}<div class="text">{n.body}</div>{/if}
              <div class="meta">{fmtTime(n.created_at)} · <span class="kind">{n.kind}</span></div>
              {#if action}
                {@const Icon = action.icon}
                <button class="action" onclick={(e) => runAction(n, e)} disabled={running[n.id]}>
                  <Icon size={12} /> {running[n.id] ? '…' : action.label}
                </button>
              {/if}
            </div>
            <button class="del" onclick={() => notifications.remove(n.id)} aria-label="delete"><Trash2 size={12} /></button>
          </div>
        {/each}
        {#if notifications.items.length === 0}
          <p class="empty">No notifications.</p>
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .bell-wrap { position: relative; display: inline-flex; }
  .bell-btn {
    position: relative;
    display: grid; place-items: center;
    width: 2.5rem; height: 2.5rem;
    border-radius: 9999px;
    border: 1.5px solid #8b6914;
    background: linear-gradient(180deg, #4e3909, #1a0f08);
    color: #f4d97a;
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.3),
      0 2px 6px rgba(0,0,0,0.55);
  }
  .bell-btn :global(svg) { width: 22px; height: 22px; }
  .bell-btn:hover {
    background: linear-gradient(180deg, #6d510f, #3a2313);
    color: #fff3c0;
    border-color: #c9a84c;
  }
  .bell-btn.ring {
    color: #fff3c0;
    border-color: #f4d97a;
    animation: glow 1.4s ease-in-out infinite;
  }
  @keyframes glow {
    0%, 100% {
      box-shadow:
        inset 0 1px 0 rgba(255,248,220,0.3),
        0 0 0 0 rgba(201, 168, 76, 0.0),
        0 2px 6px rgba(0,0,0,0.55);
    }
    50% {
      box-shadow:
        inset 0 1px 0 rgba(255,248,220,0.3),
        0 0 0 6px rgba(201, 168, 76, 0.45),
        0 2px 12px rgba(201, 168, 76, 0.6);
    }
  }
  .icon { display: inline-flex; transform-origin: 50% 0%; }
  .icon.shake { animation: shake 1.6s ease-in-out infinite; }
  @keyframes shake {
    0%, 40%, 100% { transform: rotate(0deg); }
    45% { transform: rotate(-18deg); }
    55% { transform: rotate(16deg); }
    65% { transform: rotate(-10deg); }
    75% { transform: rotate(8deg); }
    85% { transform: rotate(-4deg); }
  }
  .badge {
    position: absolute; top: -5px; right: -5px;
    min-width: 20px; height: 20px; padding: 0 5px;
    border-radius: 9999px;
    background: linear-gradient(180deg, #c95a5a, #8b1a1a);
    color: #fff6e0;
    font-size: 0.75rem;
    font-weight: 800;
    display: grid; place-items: center;
    border: 1.5px solid #f4e4c1;
    box-shadow: 0 2px 4px rgba(0,0,0,0.55);
    font-family: 'Cinzel', serif;
  }

  .panel {
    position: absolute; top: calc(100% + 0.5rem); right: 0;
    width: min(22rem, 90vw);
    max-height: 32rem;
    display: flex; flex-direction: column;
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1;
    color: #2c1810;
    box-shadow: 0 12px 30px rgba(0,0,0,0.5);
    z-index: 50;
  }
  .head {
    display: flex; justify-content: space-between; align-items: center;
    padding: 0.625rem 0.875rem;
    border-bottom: 1px solid #d4b896;
    font-family: 'Cinzel', serif;
    font-weight: 700;
  }
  .tools { display: inline-flex; gap: 0.25rem; }
  .tools button {
    display: grid; place-items: center;
    width: 1.5rem; height: 1.5rem;
    border-radius: 9999px;
    background: #e8d5a3;
    color: #5c3d2e;
  }
  .tools button:hover { background: #d4b896; }

  .list { overflow-y: auto; }
  .row {
    display: flex; gap: 0.5rem; align-items: stretch;
    border-bottom: 1px solid #e8d5a3;
  }
  .row.unread { background: linear-gradient(90deg, rgba(201,168,76,0.15), transparent 60%); }
  .body {
    flex: 1;
    text-align: left;
    padding: 0.625rem 0.875rem;
    background: transparent;
  }
  .body:hover { background: rgba(139, 105, 20, 0.08); }
  .title { font-weight: 600; color: #2c1810; }
  .text  { font-size: 0.85rem; color: #5c3d2e; margin-top: 0.125rem; }
  .meta  { font-size: 0.7rem; color: #8b6355; margin-top: 0.25rem; }
  .kind  { font-family: 'Special Elite', monospace; letter-spacing: 0.05em; }
  .del {
    padding: 0 0.5rem;
    color: #a14521;
    background: transparent;
  }
  .del:hover { color: #8b1a1a; background: rgba(139,26,26,0.08); }
  .empty { padding: 2rem 1rem; text-align: center; color: #8b6355; font-style: italic; }
  .action {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    margin-top: 0.4rem;
    padding: 0.25rem 0.55rem;
    border-radius: 0.25rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    border: 1.5px solid #4e3909;
    color: #1a0f08;
    font-size: 0.7rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 1px 3px rgba(0,0,0,0.3);
  }
  .action:hover { background-image: linear-gradient(180deg, #e5c065 0%, #a98517 55%, #7e5e10 100%); }
  .action:disabled { opacity: 0.6; cursor: default; }
</style>
