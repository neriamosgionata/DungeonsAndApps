import { describe, it, expect, vi, beforeEach } from 'vitest';

// =====================================================================
// AuthStore — test real class logic (Svelte 5 $state rune)
// =====================================================================

describe('AuthStore', () => {
  let auth: typeof import('./auth.svelte').auth;
  let storage: Record<string, string | null>;

  beforeEach(async () => {
    // Reset the global localStorage mock before each test
    storage = {};
    vi.stubGlobal('localStorage', {
      getItem: vi.fn((key: string) => storage[key] ?? null),
      setItem: vi.fn((key: string, value: string) => { storage[key] = value; }),
      removeItem: vi.fn((key: string) => { delete storage[key]; }),
    });
    // Reload the module to get a fresh AuthStore per test
    vi.resetModules();
    auth = (await import('./auth.svelte')).auth;
  });

  it('starts unauthenticated with no stored token', () => {
    expect(auth.token).toBeNull();
    expect(auth.user).toBeNull();
    expect(auth.authenticated).toBe(false);
    expect(auth.isAdmin).toBe(false);
  });

  it('set() stores token, user, and marks authenticated', () => {
    auth.set('tok-abc', { id: 'u1', email: 'a@b.com', display_name: 'A', role: 'user' });
    expect(auth.token).toBe('tok-abc');
    expect(auth.user?.id).toBe('u1');
    expect(auth.authenticated).toBe(true);
    expect(localStorage.setItem).toHaveBeenCalledWith('dungeonsandapps.token', 'tok-abc');
  });

  it('clear() wipes token and user', () => {
    auth.set('tok-xyz', { id: 'u2', email: 'x@y.com', display_name: 'X', role: 'user' });
    auth.clear();
    expect(auth.token).toBeNull();
    expect(auth.user).toBeNull();
    expect(auth.authenticated).toBe(false);
    expect(localStorage.removeItem).toHaveBeenCalledWith('dungeonsandapps.token');
  });

  it('isAdmin returns true when role is admin', () => {
    auth.set('admin-tok', { id: 'adm', email: 'a@a.com', display_name: 'Admin', role: 'admin' });
    expect(auth.isAdmin).toBe(true);
  });

  it('isAdmin returns false for user role', () => {
    auth.set('user-tok', { id: 'u3', email: 'b@b.com', display_name: 'B', role: 'user' });
    expect(auth.isAdmin).toBe(false);
  });

  it('isMaster is alias for isAdmin (app-wide)', () => {
    auth.set('m-tok', { id: 'adm2', email: 'c@c.com', display_name: 'C', role: 'admin' });
    expect(auth.isMaster).toBe(true);
    auth.clear();
    expect(auth.isMaster).toBe(false);
  });

  it('cross-tab sync updates token and user on storage event', () => {
    auth.set('initial-tok', { id: 'u4', email: 'd@d.com', display_name: 'D', role: 'user' });

    // Simulate a storage event from another tab
    const newUser = JSON.stringify({ id: 'u5', email: 'e@e.com', display_name: 'E', role: 'admin' });
    window.dispatchEvent(new StorageEvent('storage', { key: 'dungeonsandapps.token', newValue: 'new-tok' }));
    expect(auth.token).toBe('new-tok');

    window.dispatchEvent(new StorageEvent('storage', { key: 'dungeonsandapps.user', newValue: newUser }));
    expect(auth.user?.id).toBe('u5');
  });
});
