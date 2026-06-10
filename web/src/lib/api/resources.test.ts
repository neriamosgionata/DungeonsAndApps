import { describe, it, expect, vi } from 'vitest';
import { Auth } from './resources';
import { auth } from '$lib/stores/auth.svelte';

describe('Auth.updateMe', () => {
  it('sends PATCH with display_name and language', async () => {
    let capturedInit: RequestInit | undefined;
    globalThis.fetch = vi.fn(async (_input, init) => {
      capturedInit = init as RequestInit;
      return new Response(JSON.stringify({
        id: 'u1', email: 'x@x.com', display_name: 'New Name', role: 'user',
        language: 'it', avatar_url: null, created_at: new Date().toISOString(),
      }), { status: 200, headers: { 'content-type': 'application/json' } });
    }) as typeof fetch;

    auth.token = 'test-token';
    const user = await Auth.updateMe({ display_name: 'New Name', language: 'it' });

    expect(capturedInit?.method).toBe('PATCH');
    expect(JSON.parse(capturedInit?.body as string)).toEqual({ display_name: 'New Name', language: 'it' });
    expect(user.display_name).toBe('New Name');
    expect(user.language).toBe('it');
  });

  it('sends only display_name when language is omitted', async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = vi.fn(async (_input, init) => {
      capturedBody = (init as RequestInit).body as string;
      return new Response(JSON.stringify({
        id: 'u2', email: 'y@y.com', display_name: 'Only Name', role: 'user',
        language: 'en', avatar_url: null, created_at: new Date().toISOString(),
      }), { status: 200, headers: { 'content-type': 'application/json' } });
    }) as typeof fetch;

    auth.token = 'test-token';
    await Auth.updateMe({ display_name: 'Only Name' });

    expect(JSON.parse(capturedBody!)).toEqual({ display_name: 'Only Name' });
  });
});

describe('Auth.changePassword', () => {
  it('sends POST with current and new password', async () => {
    let capturedBody: string | undefined;
    globalThis.fetch = vi.fn(async (_input, init) => {
      capturedBody = (init as RequestInit).body as string;
      return new Response(null, { status: 204 });
    }) as typeof fetch;

    auth.token = 'test-token';
    await Auth.changePassword('Current1!', 'NewPass1!');

    expect(JSON.parse(capturedBody!)).toEqual({ current_password: 'Current1!', new_password: 'NewPass1!' });
  });
});
