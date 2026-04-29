<script lang="ts">
  import { page } from '$app/state';
  import { onMount, onDestroy, tick } from 'svelte';
  import { _ } from 'svelte-i18n';
  import { Messages, Campaigns } from '$lib/api/resources';
  import { auth } from '$lib/stores/auth.svelte';
  import { campaignSocket } from '$lib/ws.svelte';
  import { Send, Lock, Users as UsersIcon, MessageCircle, ChevronRight } from '@lucide/svelte';

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

  // React to `?whisper=<user_id>` — opens the whisper tab with that recipient.
  $effect(() => {
    const qw = page.url.searchParams.get('whisper');
    if (qw && qw !== whisperWith) {
      tab = 'whisper';
      whisperWith = qw;
    }
  });
  let list = $state<Msg[]>([]);
  let draft = $state('');
  let error = $state('');
  let scrollEl: HTMLDivElement | undefined = $state();

  async function load() {
    try {
      members = (await Campaigns.members(cid)) as unknown as Member[];
      const raw = tab === 'campaign'
        ? await Messages.chat(cid)
        : await Messages.whispers(cid, whisperWith || undefined);
      list = (raw as unknown as Msg[]).slice().reverse();
      await tick();
      scrollToBottom();
    } catch (e) { error = (e as Error).message; }
  }

  function scrollToBottom() { if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight; }

  let off: (() => void) | undefined;
  onMount(() => {
    load();
    off = campaignSocket.on((ev) => {
      // Whispers now arrive over the user channel with campaign_id embedded.
      // Ignore whispers belonging to a different campaign.
      if (ev.type === 'whisper' && tab === 'whisper') {
        if (ev.campaign_id && ev.campaign_id !== cid) return;
        load();
        return;
      }
      if (ev.type === 'message' && tab === 'campaign') load();
      if (ev.type === 'message_deleted') {
        if (ev.campaign_id && ev.campaign_id !== cid) return;
        load();
      }
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

  const decorated = $derived(list.map((m, i) => {
    const prev = i > 0 ? list[i - 1] : undefined;
    const prevDay = prev ? new Date(prev.created_at).toDateString() : '';
    const curDay  = new Date(m.created_at).toDateString();
    const showDay = !prev || prevDay !== curDay;
    const showSender = !prev || prev.sender_id !== m.sender_id || showDay;
    return { m, showDay, showSender };
  }));

  function pickWhisper(uid: string) {
    tab = 'whisper';
    whisperWith = uid;
  }

  // contact list (everyone except me)
  const contacts = $derived(members.filter((m) => m.user_id !== auth.user?.id));
</script>

<section class="chat">
  <header class="chat-head">
    <div class="hdr-icon"><MessageCircle size={26} style="color:#8b6914;" /></div>
    <div class="hdr-center">
      <h2 class="hdr-title">{$_('chat.title')}</h2>
      <div class="hdr-sub">
        <span class="fleuron">❦</span>
        {$_('chat.subtitle')}
        <span class="fleuron">❦</span>
      </div>
    </div>
  </header>

  <div class="rule"></div>

  <!-- tabs -->
  <div class="tabs">
    <button class="tab {tab === 'campaign' ? 'active' : ''}"
      onclick={() => { tab = 'campaign'; whisperWith = ''; }}>
      <UsersIcon size={14} /> <span>{$_('chat.tab_campaign')}</span>
    </button>
    <button class="tab {tab === 'whisper' ? 'active' : ''}"
      onclick={() => tab = 'whisper'}>
      <Lock size={14} /> <span>{$_('chat.tab_whisper')}</span>
    </button>
    {#if tab === 'whisper' && whisperWith}
      <span class="whisper-with">
        <span class="dot" style="background: {avatarColor(whisperWith)};"></span>
        <span>{displayName(whisperWith)}</span>
        <button class="switch" onclick={() => (whisperWith = '')}>{$_('chat.change')}</button>
      </span>
    {/if}
  </div>

  <!-- body: whisper-picker when needed, otherwise chat panel -->
  {#if tab === 'whisper' && !whisperWith}
    <div class="contact-picker">
      <div class="picker-head">
        <Lock size={14} style="color:#8b6914;" />
        <span>{$_('chat.pick_recipient')}</span>
      </div>
      {#if contacts.length === 0}
        <p class="italic px-3 py-4" style="color:#8b6355;">{$_('chat.no_members')}</p>
      {:else}
        <ul>
          {#each contacts as m (m.user_id)}
            <li>
              <button type="button" class="contact" onclick={() => pickWhisper(m.user_id)}>
                <span class="contact-avatar" style="background: {avatarColor(m.user_id)};">
                  {m.display_name.slice(0, 1).toUpperCase()}
                </span>
                <span class="contact-name">{m.display_name}</span>
                <span class="contact-role">{m.role}</span>
                <ChevronRight size={14} style="color:#8b6914;" />
              </button>
            </li>
          {/each}
        </ul>
      {/if}
    </div>
  {:else}
    <!-- chat panel -->
    <div bind:this={scrollEl} class="chat-panel">
      {#each decorated as { m, showDay, showSender } (m.id)}
        {@const isMe = m.sender_id === auth.user?.id}
        {#if showDay}
          <div class="day-sep">
            <span>{fmtDay(m.created_at, auth.user?.language)}</span>
          </div>
        {/if}
        <div class="row {isMe ? 'mine' : 'theirs'} {showSender ? 'spaced' : ''}">
          {#if !isMe && showSender}
            <div class="avatar" style="background: {avatarColor(m.sender_id)}">
              {displayName(m.sender_id).slice(0, 1).toUpperCase()}
            </div>
          {:else if !isMe}
            <div class="avatar-spacer"></div>
          {/if}
          <div class="bubble {isMe ? 'bubble-me' : 'bubble-them'} {m.scope === 'whisper' ? 'bubble-whisper' : ''}">
            {#if !isMe && showSender}
              <div class="sender" style="color: {avatarColor(m.sender_id)}">
                {displayName(m.sender_id)}
              </div>
            {/if}
            <div class="body">{m.body}</div>
            <div class="meta {isMe ? 'meta-me' : 'meta-them'}">
              {#if m.scope === 'whisper'}
                <span class="whisper-tag" title={$_('chat.whisper_tag')}>
                  <Lock size={10} /> {$_('chat.whisper_tag')}{#if m.recipient_id && m.recipient_id !== auth.user?.id}: {displayName(m.recipient_id)}{/if}
                </span>
                <span class="sep">·</span>
              {/if}
              <time>{fmtTime(m.created_at)}</time>
            </div>
          </div>
        </div>
      {/each}
      {#if list.length === 0}
        <p class="empty">{$_('chat.empty')}</p>
      {/if}
    </div>

    <!-- composer -->
    <form onsubmit={send} class="composer">
      <input required
        placeholder={tab === 'whisper' ? $_('chat.whisper_ph') : $_('chat.ph')}
        bind:value={draft} />
      <button aria-label="send" class="send-btn">
        <Send size={16} /> <span>{$_('chat.send')}</span>
      </button>
    </form>
  {/if}

  {#if error}<p class="err">{error}</p>{/if}
</section>

<style>
  .chat { max-width: 48rem; margin: 0 auto; padding: 1rem 1.25rem; display: flex; flex-direction: column; height: calc(100vh - 10rem); min-height: 32rem; }

  .chat-head { display: flex; align-items: center; gap: 0.75rem; }
  .hdr-icon { display: flex; justify-content: center; width: 2.25rem; }
  .hdr-center { text-align: center; flex: 1; }
  .hdr-title {
    font-family: 'IM Fell English SC', 'Cinzel', serif;
    font-size: clamp(1.5rem, 2.8vw, 2rem);
    font-weight: 900; letter-spacing: 0.08em;
    color: #2c1810 !important; line-height: 1;
  }
  .hdr-sub {
    margin-top: 0.2rem;
    font-family: 'Crimson Text', serif; font-style: italic;
    font-size: 0.8rem; color: #6d510f;
  }
  .fleuron { color: #8b6914; margin: 0 0.35rem; font-style: normal; }

  .rule {
    height: 3px; margin: 0.75rem 0 1rem;
    background: linear-gradient(90deg, transparent, #8b6914 10%, #c9a84c 50%, #8b6914 90%, transparent);
    border-top: 1px solid rgba(139,105,20,0.35);
    border-bottom: 1px solid rgba(139,105,20,0.35);
    position: relative;
  }
  .rule::before {
    content: "❦"; position: absolute; top: 50%; left: 50%;
    transform: translate(-50%, -50%);
    color: #6d510f; background: #f4e4c1;
    padding: 0 0.5rem; font-size: 0.9rem;
  }

  .tabs {
    display: flex; align-items: center; gap: 0.5rem;
    flex-wrap: wrap;
    margin-bottom: 0.75rem;
  }
  .tab {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.35rem 0.85rem;
    border-radius: 0.35rem;
    border: 1.5px solid #4e3909;
    background: rgba(139,105,20,0.1);
    color: #6d510f !important;
    font-family: 'Cinzel', serif;
    font-size: 0.75rem;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .tab:hover { background: rgba(201,168,76,0.25); color: #2c1810 !important; }
  .tab.active {
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08 !important;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55), 0 2px 4px rgba(0,0,0,0.45);
  }
  .tab span { color: inherit !important; }

  .whisper-with {
    display: inline-flex; align-items: center; gap: 0.4rem;
    margin-left: auto;
    padding: 0.25rem 0.7rem;
    border-radius: 9999px;
    border: 1.5px solid #8b6914;
    background: rgba(201,168,76,0.15);
    color: #6d510f;
    font-size: 0.8rem;
  }
  .whisper-with .dot { display: inline-block; width: 0.6rem; height: 0.6rem; border-radius: 9999px; }
  .whisper-with .switch {
    font-size: 0.6rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: #8b6914;
    background: transparent;
    text-decoration: underline;
  }

  /* contact picker for whisper */
  .contact-picker {
    flex: 1; min-height: 0;
    border: 1.5px solid #8b6914;
    border-radius: 0.5rem;
    background: #f4e4c1
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='300' height='300'><filter id='p'><feTurbulence baseFrequency='0.02 0.04' numOctaves='3'/><feColorMatrix values='0 0 0 0 0.35  0 0 0 0 0.22  0 0 0 0 0.08  0 0 0 0.06 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    overflow-y: auto;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.55);
  }
  .picker-head {
    display: flex; align-items: center; gap: 0.5rem;
    padding: 0.75rem 1rem;
    border-bottom: 1px solid #d4b896;
    font-family: 'IM Fell English SC', serif;
    color: #6d510f;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    font-size: 0.8rem;
  }
  .contact {
    display: grid; grid-template-columns: auto 1fr auto auto;
    align-items: center; gap: 0.75rem;
    width: 100%;
    padding: 0.6rem 1rem;
    background: transparent;
    text-align: left;
    border-bottom: 1px dashed rgba(139,105,20,0.25);
    color: #2c1810;
  }
  .contact:hover { background: rgba(201,168,76,0.18); }
  .contact-avatar {
    display: grid; place-items: center;
    width: 2rem; height: 2rem;
    border-radius: 9999px;
    color: white;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.3);
  }
  .contact-name { font-family: 'Cinzel', serif; font-weight: 700; }
  .contact-role { font-size: 0.7rem; text-transform: uppercase; letter-spacing: 0.1em; color: #8b6914; font-family: 'Cinzel', serif; }

  /* chat panel */
  .chat-panel {
    flex: 1; min-height: 0; overflow-y: auto;
    border: 1.5px solid #4e3909;
    border-radius: 0.5rem;
    padding: 1rem;
    background-color: #1a0f08;
    background-image:
      linear-gradient(180deg, rgba(139, 105, 20, 0.08), transparent 30%),
      url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='200' height='200'><filter id='p'><feTurbulence baseFrequency='0.85' numOctaves='2' stitchTiles='stitch'/><feColorMatrix values='0 0 0 0 0.09  0 0 0 0 0.06  0 0 0 0 0.03  0 0 0 0.25 0'/></filter><rect width='100%' height='100%' filter='url(%23p)'/></svg>");
    box-shadow: inset 0 2px 6px rgba(0,0,0,0.55);
    display: flex; flex-direction: column; gap: 0.2rem;
  }

  .day-sep {
    display: flex; justify-content: center;
    margin: 0.75rem 0;
  }
  .day-sep span {
    padding: 0.15rem 0.8rem;
    border-radius: 9999px;
    background: rgba(44,24,16,0.6);
    color: #c9a84c;
    font-family: 'IM Fell English SC', serif;
    font-size: 0.7rem;
    letter-spacing: 0.2em;
    text-transform: uppercase;
    border: 1px solid rgba(201,168,76,0.3);
  }

  .row { display: flex; gap: 0.5rem; }
  .row.spaced { margin-top: 0.6rem; }
  .row.mine { justify-content: flex-end; }
  .row.theirs { justify-content: flex-start; }
  .avatar {
    width: 1.75rem; height: 1.75rem; flex: none;
    border-radius: 9999px;
    display: grid; place-items: center;
    color: white;
    font-size: 0.75rem; font-weight: 600;
    border: 1.5px solid #4e3909;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.3);
  }
  .avatar-spacer { width: 1.75rem; flex: none; }

  .bubble {
    position: relative;
    max-width: 72%;
    padding: 0.45rem 0.7rem;
    border-radius: 0.8rem;
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
    line-height: 1.35;
    box-shadow: 0 2px 4px rgba(0,0,0,0.35);
  }
  .bubble-me {
    background: linear-gradient(180deg, #e5c065 0%, #c9a84c 100%);
    color: #1a0f08;
    border: 1px solid #4e3909;
    border-bottom-right-radius: 0.25rem;
  }
  .bubble-them {
    background: #2a1d10;
    color: #f4e4c1;
    border: 1px solid #4e3909;
    border-bottom-left-radius: 0.25rem;
  }
  .bubble-whisper {
    /* teal tint for whispers to make them visually distinct */
    box-shadow:
      inset 0 0 0 1px rgba(74,127,118,0.5),
      0 2px 4px rgba(0,0,0,0.4);
  }
  .bubble-me.bubble-whisper  { background: linear-gradient(180deg, #a8d4cb, #6fa39a); }
  .bubble-them.bubble-whisper { background: #1b2e2a; color: #c6e3dd; border-color: #2f6058; }

  .sender {
    font-family: 'Cinzel', serif;
    font-size: 0.7rem;
    letter-spacing: 0.05em;
    font-weight: 700;
    margin-bottom: 0.15rem;
  }
  .body { white-space: pre-wrap; word-break: break-word; }
  .meta {
    display: flex; justify-content: flex-end; align-items: center;
    gap: 0.25rem;
    margin-top: 0.2rem;
    font-size: 0.65rem;
    font-family: 'Special Elite', monospace;
  }
  .meta-me   { color: rgba(26,15,8,0.7); }
  .meta-them { color: rgba(244,228,193,0.55); }
  .meta .sep { opacity: 0.6; }
  .whisper-tag { display: inline-flex; align-items: center; gap: 0.2rem; }

  .empty { padding: 3rem; text-align: center; font-style: italic; color: #6d510f; }

  .composer {
    display: flex; gap: 0.5rem;
    margin-top: 0.75rem;
  }
  .composer input {
    flex: 1;
    border: 1.5px solid #4e3909 !important;
    background: #2c1810 !important;
    color: #f4e4c1 !important;
    border-radius: 9999px !important;
    padding: 0.55rem 1rem !important;
    font-family: 'Crimson Text', serif;
    font-size: 0.95rem;
  }
  .composer input:focus {
    border-color: #c9a84c !important;
    box-shadow: 0 0 0 2px rgba(201,168,76,0.25) !important;
  }
  .send-btn {
    display: inline-flex; align-items: center; gap: 0.35rem;
    padding: 0.55rem 1.1rem;
    border-radius: 9999px;
    border: 1.5px solid #4e3909;
    background-image: linear-gradient(180deg, #c9a84c 0%, #8b6914 55%, #6d510f 100%);
    color: #1a0f08 !important;
    font-family: 'Cinzel', serif;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    font-size: 0.8rem;
    box-shadow: inset 0 1px 0 rgba(255,248,220,0.5), 0 2px 4px rgba(0,0,0,0.4);
  }
  .send-btn:hover { background-image: linear-gradient(180deg, #e5c065 0%, #a98517 55%, #7e5e10 100%); }
  .send-btn span { color: inherit !important; }

  .err { color: #c95a5a; margin-top: 0.5rem; font-size: 0.85rem; }
</style>
