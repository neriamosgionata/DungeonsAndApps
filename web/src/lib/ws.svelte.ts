import { auth } from './stores/auth.svelte';
import { wsUrl } from './wsUrl';

type Listener = (event: Record<string, unknown>) => void;

class CampaignSocket {
  #ws: WebSocket | null = null;
  #listeners = new Set<Listener>();
  #openListeners = new Set<() => void>();
  #campaign = '';
  #retry: ReturnType<typeof setTimeout> | null = null;
  #stopped = true;
  #retryAttempt = 0;
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
        for (const l of this.#listeners) {
          try { l(data); } catch { /* continue to remaining listeners */ }
        }
      } catch { /* ignore malformed JSON */ }
    };
    this.#ws = ws;
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

export const campaignSocket = new CampaignSocket();
