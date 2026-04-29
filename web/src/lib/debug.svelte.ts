// Auto-debug: in-browser overlay showing live errors + API/WS activity.
// Injected by src/routes/+layout.svelte in dev only.

import { browser, dev } from '$app/environment';

export type LogEntry = {
  id: number;
  ts: number;
  level: 'error' | 'warn' | 'info' | 'api' | 'ws';
  msg: string;
  detail?: unknown;
};

class Debug {
  #id = 0;
  entries = $state<LogEntry[]>([]);
  open    = $state(false);
  enabled = $state(false);

  attach() {
    if (!browser || !dev || this.enabled) return;
    this.enabled = true;

    // window errors
    window.addEventListener('error', (ev) => {
      this.push('error', `${ev.message}`, { file: ev.filename, line: ev.lineno, col: ev.colno, stack: ev.error?.stack });
    });
    window.addEventListener('unhandledrejection', (ev) => {
      this.push('error', `unhandled: ${String(ev.reason?.message ?? ev.reason)}`, { stack: ev.reason?.stack });
    });

    // console capture
    const orig = { error: console.error.bind(console), warn: console.warn.bind(console) };
    console.error = (...a: unknown[]) => { this.push('error', a.map(fmt).join(' ')); orig.error(...a); };
    console.warn  = (...a: unknown[]) => { this.push('warn',  a.map(fmt).join(' ')); orig.warn(...a); };

    // fetch wrap
    const origFetch = window.fetch.bind(window);
    window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
      const t0 = performance.now();
      const url = typeof input === 'string' ? input : input instanceof URL ? input.toString() : input.url;
      const method = init?.method ?? (typeof input !== 'string' && !(input instanceof URL) ? input.method : 'GET');
      try {
        const res = await origFetch(input, init);
        const ms = Math.round(performance.now() - t0);
        this.push('api', `${method} ${url} → ${res.status} · ${ms}ms`, { status: res.status });
        return res;
      } catch (e) {
        const ms = Math.round(performance.now() - t0);
        this.push('error', `${method} ${url} → fetch failed · ${ms}ms`, { error: String(e) });
        throw e;
      }
    };

    // ws wrap
    const OrigWS = window.WebSocket;
    const dbg = this;
    function WrappedWS(this: WebSocket, ...args: ConstructorParameters<typeof WebSocket>) {
      const ws = new OrigWS(...args);
      const url = String(args[0]);
      dbg.push('ws', `→ connect ${url}`);
      ws.addEventListener('open',    () => dbg.push('ws', `● open ${url}`));
      ws.addEventListener('close',   (e) => dbg.push('ws', `○ close ${e.code}`, { reason: e.reason }));
      ws.addEventListener('error',   () => dbg.push('error', `ws error ${url}`));
      ws.addEventListener('message', (m) => dbg.push('ws', `← ${String(m.data).slice(0, 200)}`));
      return ws;
    }
    WrappedWS.prototype = OrigWS.prototype;
    Object.assign(WrappedWS, OrigWS);
    (window as unknown as { WebSocket: typeof WebSocket }).WebSocket = WrappedWS as unknown as typeof WebSocket;

    this.push('info', 'autodebug attached');
  }

  push(level: LogEntry['level'], msg: string, detail?: unknown) {
    const e: LogEntry = { id: ++this.#id, ts: Date.now(), level, msg, detail };
    // cap at 500 entries; panel stays collapsed until the user opens it
    this.entries = [e, ...this.entries].slice(0, 500);
  }

  clear() { this.entries = []; }
  toggle() { this.open = !this.open; }
}

function fmt(v: unknown): string {
  if (v instanceof Error) return v.stack ?? v.message;
  if (typeof v === 'object') { try { return JSON.stringify(v); } catch { return String(v); } }
  return String(v);
}

export const debug = new Debug();
