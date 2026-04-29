<script lang="ts">
  import { notifications, type Notif } from '$lib/notifications.svelte';
  import { Bell, X } from '@lucide/svelte';
  import { goto } from '$app/navigation';
  import { getNotifAction } from '$lib/notifActions';

  let running = $state<Record<string, boolean>>({});

  function targetUrl(n: Notif): string | null {
    if (!n.campaign_id) return null;
    switch (n.ref_kind) {
      case 'message':   return `/campaigns/${n.campaign_id}/messages`;
      case 'whisper':   return n.ref_id
        ? `/campaigns/${n.campaign_id}/messages?whisper=${n.ref_id}`
        : `/campaigns/${n.campaign_id}/messages`;
      case 'encounter': return `/campaigns/${n.campaign_id}/initiative`;
      case 'news':      return `/campaigns/${n.campaign_id}/news`;
      case 'campaign':  return `/campaigns/${n.campaign_id}`;
      default: return `/campaigns/${n.campaign_id}`;
    }
  }

  async function open(n: Notif) {
    notifications.dismissToast(n.id);
    await notifications.markRead(n.id);
    const url = targetUrl(n);
    if (url) goto(url);
  }

  async function runAction(n: Notif, e: Event) {
    e.stopPropagation();
    const action = getNotifAction(n);
    if (!action) return;
    running[n.id] = true;
    try {
      const keep = await action.run(n, {
        dismiss: () => notifications.dismissToast(n.id),
        markRead: () => notifications.markRead(n.id),
      });
      if (keep !== false) {
        notifications.dismissToast(n.id);
        await notifications.markRead(n.id);
      }
    } catch (err) {
      console.error('notification action failed', err);
      alert(`${getNotifAction(n)?.label ?? 'Action'} failed: ${(err as Error).message}`);
    } finally {
      running[n.id] = false;
    }
  }
</script>

<div class="toast-stack" aria-live="polite">
  {#each notifications.toasts as n (n.id)}
    {@const action = getNotifAction(n)}
    <div class="toast" role="status">
      <div class="body" role="button" tabindex="0"
        onclick={() => open(n)}
        onkeydown={(e) => (e.key === 'Enter' || e.key === ' ') && open(n)}>
        <div class="row">
          <Bell size={14} />
          <div class="title">{n.title}</div>
        </div>
        {#if n.body}<div class="text">{n.body}</div>{/if}
        {#if action}
          {@const Icon = action.icon}
          <button class="action" onclick={(e) => runAction(n, e)} disabled={running[n.id]}>
            <Icon size={13} /> {running[n.id] ? '…' : action.label}
          </button>
        {/if}
      </div>
      <button class="close" aria-label="dismiss" onclick={() => notifications.dismissToast(n.id)}>
        <X size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-stack {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
    z-index: 100;
    pointer-events: none;
    max-width: min(22rem, calc(100vw - 2rem));
  }
  .toast {
    pointer-events: auto;
    display: flex;
    align-items: stretch;
    gap: 0.25rem;
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    box-shadow:
      inset 0 1px 0 rgba(255,248,220,0.55),
      0 8px 20px rgba(0,0,0,0.45);
    animation: slide-in 0.25s ease-out;
  }
  @keyframes slide-in {
    from { transform: translateX(120%); opacity: 0; }
    to   { transform: translateX(0);   opacity: 1; }
  }
  .body {
    flex: 1;
    text-align: left;
    padding: 0.625rem 0.75rem;
    background: transparent;
    color: inherit;
  }
  .body:hover { background: rgba(139,105,20,0.08); }
  .row { display: flex; align-items: center; gap: 0.5rem; color: #8b6914; }
  .title { font-weight: 700; color: #2c1810; font-family: 'Cinzel', serif; letter-spacing: 0.02em; }
  .text  { font-size: 0.85rem; color: #5c3d2e; margin-top: 0.25rem; }
  .close {
    display: grid;
    place-items: center;
    width: 2rem;
    padding: 0 0.25rem;
    background: transparent;
    color: #8b6355;
    border-left: 1px solid #d4b896;
  }
  .close:hover { background: rgba(139,26,26,0.08); color: #8b1a1a; }
  .action {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    margin-top: 0.5rem;
    padding: 0.3rem 0.7rem;
    border-radius: 0.3rem;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    border: 1.5px solid #4e3909;
    color: #1a0f08;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.03em;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.4);
  }
  .action:hover { background-image: linear-gradient(180deg, #e5c065 0%, #a98517 55%, #7e5e10 100%); }
  .action:disabled { opacity: 0.6; cursor: default; }
</style>
