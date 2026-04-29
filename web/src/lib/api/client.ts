import { browser } from '$app/environment';

// In the browser, derive the API host from the page origin so requests always
// hit the same machine — critical when accessed from a non-localhost device.
// Override via PUBLIC_API_URL (e.g. for proxies or separate API hosts).
function apiBase(): string {
  if (import.meta.env.PUBLIC_API_URL) return import.meta.env.PUBLIC_API_URL;
  if (browser) return `${window.location.protocol}//${window.location.hostname}:8080`;
  return 'http://localhost:8080';
}

const BASE = apiBase();

export class ApiError extends Error {
  constructor(public status: number, public key: string, message: string) {
    super(message);
  }
}

export async function api<T>(path: string, init: RequestInit = {}, token?: string): Promise<T> {
  const res = await fetch(`${BASE}/api/v1${path}`, {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...(init.headers ?? {})
    }
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: { key: 'errors.internal', message: res.statusText } }));
    // stale / invalid token → wipe local session + bounce to login
    if (browser && res.status === 401 && token) {
      localStorage.removeItem('cinghialapp.token');
      localStorage.removeItem('cinghialapp.user');
      if (!location.pathname.startsWith('/login') && location.pathname !== '/') {
        location.href = '/login';
      }
    }
    throw new ApiError(res.status, body.error?.key ?? 'errors.internal', body.error?.message ?? res.statusText);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}
