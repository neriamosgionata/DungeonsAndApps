const isBrowser = typeof window !== 'undefined';

function apiBase(): string {
  if (import.meta.env.PUBLIC_API_URL) return import.meta.env.PUBLIC_API_URL;
  if (isBrowser) return `${window.location.protocol}//${window.location.hostname}:8080`;
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
    if (isBrowser && res.status === 401 && token) {
      localStorage.removeItem('dungeonsandapps.token');
      localStorage.removeItem('dungeonsandapps.user');
      if (!location.pathname.startsWith('/login') && location.pathname !== '/') {
        location.href = '/login';
      }
    }
    throw new ApiError(res.status, body.error?.key ?? 'errors.internal', body.error?.message ?? res.statusText);
  }
  if (res.status === 204) return undefined as T;
  return res.json() as Promise<T>;
}
