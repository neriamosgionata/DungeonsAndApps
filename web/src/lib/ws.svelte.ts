import { auth } from './stores/auth.svelte';
import { browser } from '$app/environment';

function wsBase(): string {
  if (import.meta.env.PUBLIC_WS_URL) return import.meta.env.PUBLIC_WS_URL as string;
  if (browser) {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const host = window.location.hostname;
    const wsHost = (host === 'localhost' || host === '0.0.0.0' || host === '127.0.0.1') ? 'localhost' : host;
    return `${proto}//${wsHost}:8080/ws`;
  }
  return 'ws://localhost:8080/ws';
}

type Listener = (event: Record<string, unknown>) => void;

class CampaignSocket {
  #ws: WebSocket | null = null;
  #listeners = new Set<Listener>();
  #openListeners = new Set<() => void>();
  #campaign = '';
  #retry: ReturnType<typeof setTimeout> | null = null;
  #stopped = true;
  connected = $state(false);

  connect(campaign: string) {
    if (this.#campaign === campaign && this.#ws
        && (this.#ws.readyState === WebSocket.OPEN || this.#ws.readyState === WebSocket.CONNECTING)) {
      return;
    }
    this.#stop();
    this.#stopped = false;
    this.#campaign = campaign;
    this.#open();
  }

  #open() {
    if (this.#stopped || !this.#campaign) return;
    const tok = auth.token;
    if (!tok) return;
    // Use Sec-WebSocket-Protocol header for auth to avoid token in URL
    // Format: auth.<token> (sent as protocol subprotocol)
    const url = wsBase();
    const ws = new WebSocket(url, [`auth.${tok}`, `campaign.${this.#campaign}`]);
    ws.onopen  = () => {
      this.connected = true;
      for (const l of this.#openListeners) l();
    };
    ws.onclose = () => {
      this.connected = false;
      this.#ws = null;
      if (!this.#stopped && this.#campaign) {
        this.#retry = setTimeout(() => this.#open(), 2000);
      }
    };
    ws.onerror = () => { this.connected = false; };
    ws.onmessage = (ev) => {
      try {
        const data = JSON.parse(ev.data);
        for (const l of this.#listeners) l(data);
      } catch { /* ignore */ }
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
