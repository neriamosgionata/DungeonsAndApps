import { browser } from '$app/environment';

export function wsUrl(): string {
  const url = import.meta.env.PUBLIC_WS_URL;
  if (url) return url as string;
  if (!browser) return 'ws://localhost:8080/ws';
  const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const host = window.location.hostname === '0.0.0.0' ? 'localhost' : window.location.hostname;
  // Production: use standard WSS port (443) via nginx proxy
  // Development: use port 8080 for local backend
  if (proto === 'wss:') return `${proto}//${host}/ws`;
  return `${proto}//${host}:8080/ws`;
}
