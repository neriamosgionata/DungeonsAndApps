import { describe, it, expect, vi } from 'vitest';
import { wsUrl } from './wsUrl';

describe('wsUrl', () => {
  it('returns a valid WebSocket URL string', () => {
    const url = wsUrl();
    expect(typeof url).toBe('string');
    expect(url).toMatch(/^wss?:\/\//);
    expect(url.endsWith('/ws')).toBe(true);
  });

  it('uses wss when protocol is https', () => {
    vi.stubGlobal('window', {
      location: { protocol: 'https:', hostname: 'example.com' },
    });
    const url = wsUrl();
    expect(url.startsWith('wss://')).toBe(true);
  });

  it('uses ws when protocol is http and not localhost', () => {
    vi.stubGlobal('window', {
      location: { protocol: 'http:', hostname: '192.168.1.1' },
    });
    const url = wsUrl();
    expect(url.startsWith('ws://')).toBe(true);
    expect(url.includes(':8080')).toBe(true);
  });

  it('maps 0.0.0.0 hostname to localhost', () => {
    vi.stubGlobal('window', {
      location: { protocol: 'http:', hostname: '0.0.0.0' },
    });
    const url = wsUrl();
    expect(url).toContain('localhost:8080');
  });

  it('wss production URL does not include port', () => {
    vi.stubGlobal('window', {
      location: { protocol: 'https:', hostname: 'mygame.com' },
    });
    const url = wsUrl();
    expect(url).toBe('wss://mygame.com/ws');
  });
});
