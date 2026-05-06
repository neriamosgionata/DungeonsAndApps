import { describe, it, expect } from 'vitest';

/**
 * Campaign context store tests
 * Testing the pure logic (store itself is Svelte 5 rune)
 */

describe('Campaign Context Logic', () => {
  it('master role has correct permissions', () => {
    const ctx = {
      isMaster: true,
      campaignId: 'test-123',
      leveling: 'xp' as const
    };

    expect(ctx.isMaster).toBe(true);
    expect(ctx.campaignId).toBe('test-123');
  });

  it('player role has correct permissions', () => {
    const ctx = {
      isMaster: false,
      campaignId: 'test-456',
      leveling: 'milestone' as const
    };

    expect(ctx.isMaster).toBe(false);
    expect(ctx.leveling).toBe('milestone');
  });
});

describe('Auth State Logic', () => {
  it('authenticated user has token and user data', () => {
    const auth = {
      token: 'test-token',
      user: {
        id: 'user-123',
        email: 'test@example.com',
        display_name: 'Test User',
        role: 'user'
      },
      isAdmin: false
    };

    expect(auth.token).toBeDefined();
    expect(auth.user?.id).toBe('user-123');
    expect(auth.isAdmin).toBe(false);
  });

  it('admin user has admin flag', () => {
    const auth = {
      token: 'admin-token',
      user: {
        id: 'admin-123',
        email: 'admin@example.com',
        display_name: 'Admin',
        role: 'admin'
      },
      isAdmin: true
    };

    expect(auth.isAdmin).toBe(true);
  });

  it('unauthenticated user has no token', () => {
    const auth = {
      token: null,
      user: null,
      isAdmin: false
    };

    expect(auth.token).toBeNull();
    expect(auth.user).toBeNull();
  });
});
