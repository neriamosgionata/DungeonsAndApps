import { describe, it, expect, vi, beforeEach } from 'vitest';
import { ApiError, api } from './client';

describe('api client', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('returns parsed json on 200', async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    ) as typeof fetch;
    const out = await api<{ ok: boolean }>('/health');
    expect(out.ok).toBe(true);
  });

  it('throws ApiError on non-2xx with error body', async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response(JSON.stringify({ error: { key: 'errors.unauthorized', message: 'nope' } }), { status: 401 })
    ) as typeof fetch;
    await expect(api('/auth/me')).rejects.toBeInstanceOf(ApiError);
  });

  it('returns undefined on 204', async () => {
    globalThis.fetch = vi.fn(async () => new Response(null, { status: 204 })) as typeof fetch;
    const out = await api('/something');
    expect(out).toBeUndefined();
  });

  it('injects authorization header when token is provided', async () => {
    const fetchSpy = vi.fn(async () =>
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    ) as typeof fetch;
    globalThis.fetch = fetchSpy;

    await api('/data', {}, 'bearer-token-123');
    const call = fetchSpy.mock.calls[0] as [string, RequestInit];
    const headers = call[1].headers as Record<string, string>;
    expect(headers.authorization).toBe('Bearer bearer-token-123');
    expect(headers['content-type']).toBe('application/json');
  });

  it('does not inject authorization header when no token', async () => {
    const fetchSpy = vi.fn(async () =>
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    ) as typeof fetch;
    globalThis.fetch = fetchSpy;

    await api('/data');
    const call = fetchSpy.mock.calls[0] as [string, RequestInit];
    const headers = call[1].headers as Record<string, string>;
    expect(headers.authorization).toBeUndefined();
  });

  it('handles network errors (fetch rejected)', async () => {
    globalThis.fetch = vi.fn(() => Promise.reject(new TypeError('Failed to fetch'))) as typeof fetch;
    await expect(api('/data')).rejects.toThrow('Failed to fetch');
  });

  it('handles non-json error response gracefully', async () => {
    globalThis.fetch = vi.fn(async () =>
      new Response('<html>Server Error</html>', { status: 500, headers: { 'content-type': 'text/html' } })
    ) as typeof fetch;
    await expect(api('/data')).rejects.toBeInstanceOf(ApiError);
  });

  it('preserves custom init headers when token is provided', async () => {
    const fetchSpy = vi.fn(async () =>
      new Response(JSON.stringify({ ok: true }), { status: 200, headers: { 'content-type': 'application/json' } })
    ) as typeof fetch;
    globalThis.fetch = fetchSpy;

    await api('/data', { headers: { 'x-custom': 'value' } }, 'tok');
    const call = fetchSpy.mock.calls[0] as [string, RequestInit];
    const headers = call[1].headers as Record<string, string>;
    expect(headers.authorization).toBe('Bearer tok');
    expect(headers['x-custom']).toBe('value');
  });
});
