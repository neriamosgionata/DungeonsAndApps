import { browser } from '$app/environment';
import { auth } from './stores/auth.svelte';
import { api } from './api/client';
import { wsUrl } from './wsUrl';

export type Notif = {
  id: string;
  user_id: string;
  campaign_id: string | null;
  kind: string;
  title: string;
  body: string | null;
  ref_kind: string | null;
  ref_id: string | null;
  read_at: string | null;
  created_at: string;
};

class NotifStore {
  items = $state<Notif[]>([]);
  toasts = $state<Notif[]>([]);
  connected = $state(false);
  #ws: WebSocket | null = null;
  #retry: ReturnType<typeof setTimeout> | null = null;
  #stopped = true;
  #timers = new Map<string, ReturnType<typeof setTimeout>>();

  get unread() { return this.items.filter((n) => !n.read_at).length; }

  pushToast(n: Notif, ttl = 10000) {
    this.toasts = [n, ...this.toasts].slice(0, 5);
    const t = setTimeout(() => this.dismissToast(n.id), ttl);
    this.#timers.set(n.id, t);
  }

  dismissToast(id: string) {
    this.toasts = this.toasts.filter((t) => t.id !== id);
    const t = this.#timers.get(id);
    if (t) { clearTimeout(t); this.#timers.delete(id); }
  }

  async refresh() {
    if (!auth.token) return;
    try {
      this.items = await api<Notif[]>('/notifications?limit=50', {}, auth.token);
    } catch { /* ignore */ }
  }

  connect() {
    if (!browser || this.#ws || !auth.token) return;
    this.#stopped = false;
    this.#open();
  }

  #open() {
    if (this.#stopped || !auth.token) return;
    const ws = new WebSocket(wsUrl(), [`auth.${auth.token}`]);
    ws.onopen = () => {
      this.connected = true;
      this.#retry = null;
      this.refresh();
    };
    ws.onclose = () => {
      this.connected = false;
      this.#ws = null;
      if (!this.#stopped && auth.token) {
        this.#retry = setTimeout(() => this.#open(), 3000);
      }
    };
    ws.onerror = () => { this.connected = false; };
    ws.onmessage = (ev) => {
      try {
        const data = JSON.parse(ev.data);
        if (data.type === 'notification' && data.notification) {
          const n = data.notification as Notif;
          this.items = [n, ...this.items].slice(0, 100);
          this.pushToast(n);
        }
      } catch { /* ignore */ }
    };
    this.#ws = ws;
  }

  disconnect() {
    this.#stopped = true;
    if (this.#retry) { clearTimeout(this.#retry); this.#retry = null; }
    this.#ws?.close();
    this.#ws = null;
    this.connected = false;
  }

  async markRead(id: string) {
    this.items = this.items.map((n) => n.id === id ? { ...n, read_at: new Date().toISOString() } : n);
    try { await api<void>(`/notifications/${id}/read`, { method: 'POST' }, auth.token ?? undefined); }
    catch { /* ignore */ }
  }

  async markAllRead() {
    const now = new Date().toISOString();
    this.items = this.items.map((n) => n.read_at ? n : { ...n, read_at: now });
    try { await api<void>(`/notifications/read-all`, { method: 'POST' }, auth.token ?? undefined); }
    catch { /* ignore */ }
  }

  async remove(id: string) {
    this.items = this.items.filter((n) => n.id !== id);
    try { await api<void>(`/notifications/${id}`, { method: 'DELETE' }, auth.token ?? undefined); }
    catch { /* ignore */ }
  }

  clearAll() {
    // Fix: clear all pending toast timers to prevent memory leak
    for (const t of this.#timers.values()) {
      clearTimeout(t);
    }
    this.#timers.clear();
    this.items = [];
    this.toasts = [];
    // Fire-and-forget the API call
    api<void>(`/notifications`, { method: 'DELETE' }, auth.token ?? undefined).catch(() => {
      // ignore
    });
  }
}

export const notifications = new NotifStore();
