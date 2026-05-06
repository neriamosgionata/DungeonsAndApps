import { describe, it, expect } from 'vitest';
import { randomUUID } from '$lib/uuid';

describe('randomUUID', () => {
  it('returns a string', () => {
    const id = randomUUID();
    expect(typeof id).toBe('string');
  });

  it('returns different values on multiple calls', () => {
    const id1 = randomUUID();
    const id2 = randomUUID();
    expect(id1).not.toBe(id2);
  });

  it('returns UUID format (8-4-4-4-12)', () => {
    const id = randomUUID();
    expect(id).toMatch(/^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/);
  });

  it('returns 36 characters including dashes', () => {
    const id = randomUUID();
    expect(id.length).toBe(36);
  });
});
