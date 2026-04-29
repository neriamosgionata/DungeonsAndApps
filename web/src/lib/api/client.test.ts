import { describe, it, expect, vi } from 'vitest';
import { ApiError, api } from './client';

describe('api client', () => {
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
});
