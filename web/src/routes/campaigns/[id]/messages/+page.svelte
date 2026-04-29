<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Messages, Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { Send, Lock, Users as UsersIcon } from '@lucide/svelte';

  type Member = { user_id: string; display_name: string; email: string; role: string };
  type Msg = {
    id: string;
    sender_id: string;
    recipient_id: string | null;
    scope: 'campaign' | 'whisper';
    body: string;
    created_at: string;
  };

  const cid = $derived(page.params.id!);
  let members = $state<Member[]>([]);
  let tab = $state<'campaign' | 'whisper'>('campaign');
  let whisperWith = $state<string>('');
  let list = $state<Msg[]>([]);
  let draft = $state('');
  let error = $state('');
  let scrollEl: HTMLDivElement | undefined;

  async function load() {
    try {
      members = (await Campaigns.members(cid)) as unknown as Member[];
      const raw = tab === 'campaign'
        ? await Messages.chat(cid)
        : await Messages.whispers(cid, whisperWith || undefined);
      // backend returns newest first — reverse for chronological
      list = (raw as unknown as Msg[]).slice().reverse();
      await tick();
      scrollToBottom();
    } catch (e) { error = (e as Error).message; }
  }

  function scrollToBottom() {
    if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
  }

  let off: (() => void) | undefined;
  onMount(() => {
    load();
    off = campaignSocket.on((ev) => {
      if ((ev.type === 'message' && tab === 'campaign') ||
          (ev.type === 'whisper' && tab === 'whisper')) load();
    });
  });
  onDestroy(() => off?.());

  $effect(() => { void tab; void whisperWith; load(); });

  async function send(e: SubmitEvent) {
    e.preventDefault();
    if (!draft.trim()) return;
    try {
      if (tab === 'campaign') await Messages.send(cid, draft.trim(), 'campaign');
      else if (whisperWith) await Messages.send(cid, draft.trim(), 'whisper', whisperWith);
      draft = '';
      await load();
    } catch (e) { error = (e as Error).message; }
  }

  function displayName(userId: string): string {
    return members.find((m) => m.user_id === userId)?.display_name ?? userId.slice(0, 6);
  }

  // avatar-color derived from user id (stable pastel)
  function avatarColor(userId: string): string {
    let h = 0;
    for (let i = 0; i < userId.length; i++) h = (h * 31 + userId.charCodeAt(i)) & 0xffff;
    const hue = h % 360;
    return `hsl(${hue} 55% 40%)`;
  }

  function fmtTime(iso: string): string {
    const d = new Date(iso);
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
  }

  function fmtDay(iso: string, locale: string | undefined): string {
    const d = new Date(iso);
    const today = new Date();
    const yest = new Date(); yest.setDate(today.getDate() - 1);
    const sameDay = (a: Date, b: Date) =>
      a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
    if (sameDay(d, today)) return $_('chat.today');
    if (sameDay(d, yest))  return $_('chat.yesterday');
    return d.toLocaleDateString(locale, { day: '2-digit', month: 'short', year: 'numeric' });
  }

  // annotate: show sender display when prev is different, show day separator when day changes
  const decorated = $derived(list.map((m, i) => {
    const prev = i > 0 ? list[i - 1] : undefined;
    const prevDay = prev ? new Date(prev.created_at).toDateString() : '';
    const curDay  = new Date(m.created_at).toDateString();
    const showDay = !prev || prevDay !== curDay;
    const showSender = !prev || prev.sender_id !== m.sender_id || showDay;
    return { m, showDay, showSender };
  }));
</script>

<section class="mx-auto max-w-3xl px-6 py-6">
  <!-- tab bar -->
  <div class="flex flex-wrap items-center gap-2">
    <button class="inline-flex items-center gap-1.5 {tab === 'campaign' ? 'bg-violet-600 text-white rounded-md px-3 py-1.5 text-sm' : 'bg-neutral-800 rounded-md px-3 py-1.5 text-sm'}"
      onclick={() => tab = 'campaign'}>
      <UsersIcon size={14} /> {$_('chat.tab_campaign')}
    </button>
    <button class="inline-flex items-center gap-1.5 {tab === 'whisper' ? 'bg-violet-600 text-white rounded-md px-3 py-1.5 text-sm' : 'bg-neutral-800 rounded-md px-3 py-1.5 text-sm'}"
      onclick={() => tab = 'whisper'}>
      <Lock size={14} /> {$_('chat.tab_whisper')}
    </button>
    {#if tab === 'whisper'}
      <select bind:value={whisperWith} class="rounded-md bg-neutral-900 border border-neutral-700 px-2 py-1 text-sm">
        <option value="">— {$_('chat.select_recipient')} —</option>
        {#each members.filter((m) => m.user_id !== auth.user?.id) as m (m.user_id)}
          <option value={m.user_id}>{m.display_name} ({m.role})</option>
        {/each}
      </select>
    {/if}
  </div>

  <!-- chat panel -->
  <div bind:this={scrollEl}
    class="chat-panel mt-4 h-[65vh] overflow-y-auto rounded-lg border border-neutral-800 p-4 space-y-1">
    {#each decorated as { m, showDay, showSender } (m.id)}
      {@const isMe = m.sender_id === auth.user?.id}
      {#if showDay}
        <div class="my-3 flex justify-center">
          <span class="rounded-full bg-neutral-900/80 px-3 py-0.5 text-[11px] uppercase tracking-widest text-neutral-300 ring-1 ring-white/10">
            {fmtDay(m.created_at, auth.user?.language)}
          </span>
        </div>
      {/if}
      <div class="flex {isMe ? 'justify-end' : 'justify-start'} gap-2 {showSender ? 'mt-3' : 'mt-0.5'}">
        {#if !isMe && showSender}
          <div class="h-7 w-7 flex-none rounded-full text-white text-xs font-semibold grid place-items-center"
               style="background: {avatarColor(m.sender_id)}">
            {displayName(m.sender_id).slice(0, 1).toUpperCase()}
          </div>
        {:else if !isMe}
          <div class="w-7 flex-none"></div>
        {/if}
        <div class="relative max-w-[72%] rounded-xl px-3 py-2 text-sm shadow-sm
                    {isMe ? 'bubble-me rounded-br-sm' : 'bubble-them rounded-bl-sm'}">
          {#if !isMe && showSender}
            <div class="text-xs font-semibold mb-0.5" style="color: {avatarColor(m.sender_id)}">
              {displayName(m.sender_id)}
            </div>
          {/if}
          <div class="whitespace-pre-wrap break-words">{m.body}</div>
          <div class="mt-1 flex items-center justify-end gap-1 text-[10px] {isMe ? 'text-amber-900/70' : 'text-neutral-400'}">
            {#if m.scope === 'whisper'}
              <span title="private" class="lowercase">🔒 {$_('chat.whisper_tag')}{#if m.recipient_id && m.recipient_id !== auth.user?.id}: {displayName(m.recipient_id)}{/if}</span>
              <span class="opacity-60">·</span>
            {/if}
            <time>{fmtTime(m.created_at)}</time>
          </div>
        </div>
      </div>
    {/each}
    {#if list.length === 0}
      <p class="mt-10 text-center text-neutral-500 italic">{$_('chat.empty')}</p>
    {/if}
  </div>

  <!-- composer -->
  <form onsubmit={send} class="mt-3 flex gap-2">
    <input required
      placeholder={tab === 'whisper' ? $_('chat.whisper_ph') : $_('chat.ph')}
      bind:value={draft}
      class="flex-1 rounded-full bg-neutral-900 border border-neutral-700 px-4 py-2" />
    <button aria-label="send"
      class="inline-flex items-center gap-1.5 rounded-full bg-violet-600 px-6 py-2 text-white">
      <Send size={16} /> {$_('chat.send')}
    </button>
  </form>
  {#if error}<p class="mt-2 text-sm text-red-400">{error}</p>{/if}
</section>

<style>
  .chat-panel {
    background-color: #1a0f08;
    background-image:
      linear-gradient(180deg, rgba(139, 105, 20, 0.05), transparent 30%),
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='p'><feTurbulence baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 0.09  0 0 0 0 0.06  0 0 0 0 0.03  0 0 0 0.3 0'/></filter><rect width='100%' height='100%' filter='url(%23p)' opacity='0.6'/></svg>");
  }
  .bubble-me   { background: linear-gradient(180deg, #e5c065 0%, #c9a84c 100%); color: #1a0f08; }
  .bubble-them { background: #2a1d10; color: #f4e4c1; border: 1px solid #4e3909; }
</style>
