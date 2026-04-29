<script lang="ts">
  import { onDestroy, onMount } from 'svelte';
  import { Campaigns } from '$lib/api/resources';
  import { campaignSocket } from '$lib/ws.svelte';
  import { CircleDot, Circle, Users } from '@lucide/svelte';

  let { cid }: { cid: string } = $props();

  type Member = { user_id: string; display_name: string; email: string; role: string };

  let members = $state<Member[]>([]);
  let online = $state<Set<string>>(new Set());
  let open = $state(false);
  let wrap: HTMLDivElement | undefined = $state();

  async function loadMembers() {
    try { members = await Campaigns.members(cid); } catch { members = []; }
  }
  async function loadPresence() {
    try { online = new Set(await Campaigns.presence(cid)); } catch { online = new Set(); }
  }

  let pollTimer: ReturnType<typeof setInterval> | undefined;
  onMount(() => {
    loadMembers();
    loadPresence();
    // Poll every 30s as a safety net for missed WS events.
    pollTimer = setInterval(loadPresence, 30_000);
  });

  let offWs: (() => void) | undefined;
  let offOpen: (() => void) | undefined;
  onMount(() => {
    offWs = campaignSocket.on((ev) => {
      const t = ev.type as string;
      const uid = ev.user_id as string | undefined;
      if (!uid) return;
      if (t === 'presence_joined') { online = new Set([...online, uid]); }
      else if (t === 'presence_left') { const n = new Set(online); n.delete(uid); online = n; }
    });
    // Re-fetch presence on reconnect so the counter is always in sync.
    offOpen = campaignSocket.onOpen(() => loadPresence());
    const click = (e: MouseEvent) => {
      if (!open) return;
      if (wrap && !wrap.contains(e.target as Node)) open = false;
    };
    window.addEventListener('mousedown', click);
    return () => window.removeEventListener('mousedown', click);
  });
  onDestroy(() => { offWs?.(); offOpen?.(); clearInterval(pollTimer); });

  const sorted = $derived([...members].sort((a, b) => {
    const ao = online.has(a.user_id) ? 0 : 1;
    const bo = online.has(b.user_id) ? 0 : 1;
    if (ao !== bo) return ao - bo;
    return a.display_name.localeCompare(b.display_name);
  }));
  const onlineCount = $derived(members.filter((m) => online.has(m.user_id)).length);
</script>

<div class="presence-wrap" bind:this={wrap}>
  <button class="presence-btn" onclick={() => (open = !open)} title="Party presence">
    <Users size={16} />
    <span class="count"><span class="dot on"></span>{onlineCount}/{members.length}</span>
  </button>

  {#if open}
    <div class="presence-panel" role="dialog">
      <header class="presence-head">Party presence</header>
      <ul class="presence-list">
        {#each sorted as m (m.user_id)}
          <li class="row">
            {#if online.has(m.user_id)}
              <CircleDot size={12} class="on" />
            {:else}
              <Circle size={12} class="off" />
            {/if}
            <span class="name">{m.display_name}</span>
            <span class="role">{m.role}</span>
          </li>
        {/each}
        {#if members.length === 0}
          <li class="empty">No members.</li>
        {/if}
      </ul>
    </div>
  {/if}
</div>

<style>
  .presence-wrap { position: relative; display: inline-flex; }
  .presence-btn {
    display: inline-flex; align-items: center; gap: 0.4rem;
    padding: 0.3rem 0.7rem;
    border-radius: 9999px;
    border: 1px solid #4e3909;
    background: linear-gradient(180deg, #3a2313, #1a0f08);
    color: #c9a84c;
    font-family: 'Cinzel', serif;
    font-size: 0.75rem;
    letter-spacing: 0.04em;
  }
  .presence-btn:hover { background: linear-gradient(180deg, #4e3909, #2c1810); color: #f7e2a5; }
  .count { display: inline-flex; align-items: center; gap: 0.3rem; }
  .dot {
    display: inline-block; width: 8px; height: 8px; border-radius: 9999px;
  }
  .dot.on { background: #6b8a4f; box-shadow: 0 0 6px #6b8a4f; }

  .presence-panel {
    position: absolute; top: calc(100% + 0.5rem); right: 0;
    min-width: 16rem;
    max-height: 24rem;
    overflow-y: auto;
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.05 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    color: #2c1810;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 12px 30px rgba(0,0,0,0.55);
    z-index: 40;
  }
  .presence-head {
    padding: 0.55rem 0.85rem;
    border-bottom: 1px solid #d4b896;
    font-family: 'Cinzel', serif;
    font-weight: 700;
  }
  .presence-list { padding: 0.25rem 0; }
  .row {
    display: grid;
    grid-template-columns: auto 1fr auto;
    align-items: center;
    gap: 0.5rem;
    padding: 0.35rem 0.85rem;
    border-bottom: 1px dashed rgba(139,105,20,0.25);
  }
  .row:last-child { border-bottom: 0; }
  .row :global(.on)  { color: #6b8a4f; }
  .row :global(.off) { color: #8b6355; }
  .name { font-weight: 600; color: #2c1810; }
  .role { font-size: 0.7rem; font-family: 'Cinzel', serif; letter-spacing: 0.08em; color: #8b6914; text-transform: uppercase; }
  .empty { padding: 1rem; text-align: center; font-style: italic; color: #8b6355; }
</style>
