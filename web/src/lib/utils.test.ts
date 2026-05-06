import { describe, it, expect } from 'vitest';

/**
 * String/utility helpers
 */

function slugify(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .replace(/\s+/g, '-')
    .replace(/-+/g, '-')
    .substring(0, 50);
}

function formatNumber(n: number): string {
  if (n >= 1000000) return (n / 1000000).toFixed(1) + 'M';
  if (n >= 1000) return (n / 1000).toFixed(1) + 'k';
  return String(n);
}

function capitalize(str: string): string {
  if (!str) return '';
  return str.charAt(0).toUpperCase() + str.slice(1).toLowerCase();
}

function truncate(str: string, maxLen: number): string {
  if (str.length <= maxLen) return str;
  return str.substring(0, maxLen - 3) + '...';
}

function debounce<T extends (...args: unknown[]) => void>(
  fn: T,
  delay: number
): (...args: Parameters<T>) => void {
  let timeoutId: ReturnType<typeof setTimeout>;
  return (...args: Parameters<T>) => {
    clearTimeout(timeoutId);
    timeoutId = setTimeout(() => fn(...args), delay);
  };
}

// =====================================================================
// Tests
// =====================================================================

describe('slugify', () => {
  it('converts to lowercase', () => {
    expect(slugify('HELLO')).toBe('hello');
  });

  it('replaces spaces with hyphens', () => {
    expect(slugify('hello world')).toBe('hello-world');
  });

  it('removes special characters', () => {
    expect(slugify('hello@world!')).toBe('helloworld');
  });

  it('removes multiple hyphens', () => {
    expect(slugify('hello---world')).toBe('hello-world');
  });

  it('truncates to 50 chars', () => {
    const long = 'a'.repeat(60);
    expect(slugify(long).length).toBe(50);
  });
});

describe('formatNumber', () => {
  it('formats thousands as k', () => {
    expect(formatNumber(1500)).toBe('1.5k');
    expect(formatNumber(1000)).toBe('1.0k');
  });

  it('formats millions as M', () => {
    expect(formatNumber(2500000)).toBe('2.5M');
  });

  it('returns small numbers as-is', () => {
    expect(formatNumber(500)).toBe('500');
    expect(formatNumber(0)).toBe('0');
  });
});

describe('capitalize', () => {
  it('capitalizes first letter', () => {
    expect(capitalize('hello')).toBe('Hello');
  });

  it('lowercases rest', () => {
    expect(capitalize('HELLO')).toBe('Hello');
  });

  it('handles empty string', () => {
    expect(capitalize('')).toBe('');
  });

  it('handles single char', () => {
    expect(capitalize('a')).toBe('A');
  });
});

describe('truncate', () => {
  it('returns short strings unchanged', () => {
    expect(truncate('hello', 10)).toBe('hello');
  });

  it('truncates long strings with ellipsis', () => {
    expect(truncate('hello world', 8)).toBe('hello...');
  });

  it('respects exact length', () => {
    const result = truncate('hello world', 5);
    expect(result.length).toBe(5);
    expect(result).toBe('he...');
  });
});

describe('debounce', () => {
  it('delays function call', async () => {
    let called = false;
    const fn = () => { called = true; };
    const debounced = debounce(fn, 50);

    debounced();
    expect(called).toBe(false);

    await new Promise(r => setTimeout(r, 60));
    expect(called).toBe(true);
  });

  it('cancels previous call', async () => {
    let count = 0;
    const fn = () => { count++; };
    const debounced = debounce(fn, 50);

    debounced();
    debounced();
    debounced();

    await new Promise(r => setTimeout(r, 60));
    expect(count).toBe(1);
  });
});
