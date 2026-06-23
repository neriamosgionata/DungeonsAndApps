import { auth } from './stores/auth.svelte';
import { wsUrl } from './wsUrl';

type Listener = (event: Record<string, unknown>) => void;

const LS_SEQ_KEY = (cid: string) => `cinghiale.ws.lastSeq.${cid}`;

class CampaignSocket {
  #ws: WebSocket | null = null;
  #listeners = new Set<Listener>();
  #openListeners = new Set<() => void>();
  #campaign = '';
  #retry: ReturnType<typeof setTimeout> | null = null;
  #stopped = true;
  #retryAttempt = 0;
  #replaying = false;
  connected = $state(false);

  connect(campaign: string) {
    if (this.#campaign === campaign && this.#ws
        && (this.#ws.readyState === WebSocket.OPEN || this.#ws.readyState === WebSocket.CONNECTING)) {
      return;
    }
    this.#stop();
    this.#stopped = false;
    this.#campaign = campaign;
    this.#retryAttempt = 0;
    this.#open();
  }

  #open() {
    if (this.#stopped || !this.#campaign) return;
    const tok = auth.token;
    if (!tok) return;
    // Use Sec-WebSocket-Protocol header for auth to avoid token in URL
    // Format: auth.<token> (sent as protocol subprotocol)
    const url = wsUrl();
    const ws = new WebSocket(url, [`auth.${tok}`, `campaign.${this.#campaign}`]);
    ws.onopen  = () => {
      this.connected = true;
      this.#retryAttempt = 0; // reset backoff on successful open
      // M-F6 part 2: fetch missed events on (re)connect. The server's
      // /ws-events endpoint replays any events with seq > lastSeq.
      void this.#replayMissed();
      for (const l of this.#openListeners) l();
    };
    ws.onclose = () => {
      this.connected = false;
      this.#ws = null;
      if (!this.#stopped && this.#campaign) {
        // M-F6: exponential backoff. Pre-fix: fixed 2s retry caused
        // reconnect storms on persistent server failures. Post-fix:
        // 1s, 2s, 4s, 8s, 16s, 30s (cap). Reset to 0 on successful open.
        const delay_ms = Math.min(30_000, 1000 * (2 ** this.#retryAttempt));
        this.#retryAttempt += 1;
        this.#retry = setTimeout(() => this.#open(), delay_ms);
      }
    };
    ws.onerror = () => { this.connected = false; };
    ws.onmessage = (ev) => {
      try {
        const data = JSON.parse(ev.data);
        this.#updateLastSeq(data);
        for (const l of this.#listeners) {
          try { l(data); } catch { /* continue to remaining listeners */ }
        }
      } catch { /* ignore malformed JSON */ }
    };
    this.#ws = ws;
  }

  // M-F6 part 2: fetch missed events since lastSeq. Called on WS open
  // and on initial page load. Replays each event through the listener
  // pipeline (same path as live events) so the frontend doesn't
  // distinguish replayed from live.
  async #replayMissed() {
    if (this.#stopped || !this.#campaign) return;
    if (this.#replaying) return; // guard against double-fetch
    this.#replaying = true;
    try {
      const lastSeq = this.#getLastSeq();
      const tok = auth.token;
      if (!tok) return;
      const url = `/api/v1/ws-events?campaign_id=${encodeURIComponent(this.#campaign)}&since=${lastSeq}&limit=500`;
      const res = await fetch(url, { headers: { Authorization: `Bearer ${tok}` } });
      if (!res.ok) {
        tracing_warn('replay_fetch_failed', res.status);
        return;
      }
      const data = await res.json();
      const events: Array<Record<string, unknown>> = Array.isArray(data?.events) ? data.events : [];
      for (const ev of events) {
        this.#updateLastSeq(ev);
        for (const l of this.#listeners) {
          try { l(ev); } catch { /* continue */ }
        }
      }
    } catch (e) {
      // Best-effort: a failed replay just means the client may be stale
      // until the next loadList() from any combatant_* event.
      tracing_warn('replay_failed', String(e));
    } finally {
      this.#replaying = false;
    }
  }

  // M-F6 part 2: track last received seq per campaign in localStorage.
  // localStorage so it survives page reload — the server replays from
  // this cursor on the next WS open.
  #getLastSeq(): number {
    try {
      const v = localStorage.getItem(LS_SEQ_KEY(this.#campaign));
      if (!v) return 0;
      const n = Number(v);
      return Number.isFinite(n) && n > 0 ? n : 0;
    } catch { return 0; }
  }
  #setLastSeq(seq: number) {
    try { localStorage.setItem(LS_SEQ_KEY(this.#campaign), String(seq)); }
    catch { /* ignore quota / private mode */ }
  }
  #updateLastSeq(ev: Record<string, unknown>) {
    const seq = ev.seq;
    if (typeof seq !== 'number' || !Number.isFinite(seq) || seq <= 0) return;
    const current = this.#getLastSeq();
    if (seq > current) this.#setLastSeq(seq);
  }

  #stop() {
    this.#stopped = true;
    if (this.#retry) { clearTimeout(this.#retry); this.#retry = null; }
    if (this.#ws) { try { this.#ws.close(); } catch { /* ignore */ } this.#ws = null; }
    this.connected = false;
  }

  disconnect() {
    this.#stop();
    this.#campaign = '';
  }

  on(l: Listener) { this.#listeners.add(l); return () => this.#listeners.delete(l); }
  onOpen(l: () => void) {
    this.#openListeners.add(l);
    return () => this.#openListeners.delete(l);
  }
}

// Tiny no-op logger to avoid pulling a logging dep. Real warnings go to
// the browser console (the user's DevTools). Not console.error to keep
// the noise down — a failed replay is recoverable.
function tracing_warn(_tag: string, _detail: unknown) { /* no-op */ }

export const campaignSocket = new CampaignSocket();
