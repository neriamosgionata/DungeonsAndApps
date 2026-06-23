import { describe, it, expect, vi, beforeEach } from 'vitest';

const mockUser = (overrides: Partial<{ id: string; email: string; display_name: string; role: 'user' | 'admin' }> = {}) =>
  ({
    id: overrides.id ?? 'u1',
    email: overrides.email ?? 'a@b.com',
    display_name: overrides.display_name ?? 'Test',
    role: overrides.role ?? 'user',
    language: 'en',
    created_at: '2024-01-01T00:00:00Z',
  }) as import('$lib/types').User;

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
    auth.set('tok-abc', mockUser());
    expect(auth.token).toBe('tok-abc');
    expect(auth.user?.id).toBe('u1');
    expect(auth.authenticated).toBe(true);
    expect(localStorage.setItem).toHaveBeenCalledWith('dungeonsandapps.token', 'tok-abc');
  });

  it('clear() wipes token and user', () => {
    auth.set('tok-xyz', mockUser({ id: 'u2' }));
    auth.clear();
    expect(auth.token).toBeNull();
    expect(auth.user).toBeNull();
    expect(auth.authenticated).toBe(false);
    expect(localStorage.removeItem).toHaveBeenCalledWith('dungeonsandapps.token');
  });

  it('isAdmin returns true when role is admin', () => {
    auth.set('admin-tok', mockUser({ role: 'admin' }));
    expect(auth.isAdmin).toBe(true);
  });

  it('isAdmin returns false for user role', () => {
    auth.set('user-tok', mockUser());
    expect(auth.isAdmin).toBe(false);
  });

  it('isAppAdmin returns true for admin role', () => {
    auth.set('a-tok', mockUser({ role: 'admin' }));
    expect(auth.isAppAdmin).toBe(true);
    auth.clear();
    expect(auth.isAppAdmin).toBe(false);
  });

  it('cross-tab sync updates token and user on storage event', () => {
    auth.set('initial-tok', mockUser());

    // Simulate a storage event from another tab
    const newUser = JSON.stringify(mockUser({ id: 'u5', role: 'admin' }));
    window.dispatchEvent(new StorageEvent('storage', { key: 'dungeonsandapps.token', newValue: 'new-tok' }));
    expect(auth.token).toBe('new-tok');

    window.dispatchEvent(new StorageEvent('storage', { key: 'dungeonsandapps.user', newValue: newUser }));
    expect(auth.user?.id).toBe('u5');
  });
});
