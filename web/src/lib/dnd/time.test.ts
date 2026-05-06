import { describe, it, expect } from 'vitest';

/**
 * Date/time formatting utilities
 */

function formatDuration(minutes: number): string {
  if (minutes < 60) return `${minutes}m`;
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  if (mins === 0) return `${hours}h`;
  return `${hours}h ${mins}m`;
}

function timeAgo(date: Date): string {
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffSecs = Math.floor(diffMs / 1000);
  const diffMins = Math.floor(diffSecs / 60);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffSecs < 60) return 'just now';
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return date.toLocaleDateString();
}

function formatDate(date: Date): string {
  return date.toLocaleDateString('en-US', {
    year: 'numeric',
    month: 'short',
    day: 'numeric'
  });
}

// =====================================================================
// Tests
// =====================================================================

describe('formatDuration', () => {
  it('formats minutes', () => {
    expect(formatDuration(30)).toBe('30m');
    expect(formatDuration(59)).toBe('59m');
  });

  it('formats hours', () => {
    expect(formatDuration(60)).toBe('1h');
    expect(formatDuration(120)).toBe('2h');
  });

  it('formats hours and minutes', () => {
    expect(formatDuration(90)).toBe('1h 30m');
    expect(formatDuration(150)).toBe('2h 30m');
  });
});

describe('timeAgo', () => {
  it('returns just now for recent', () => {
    const now = new Date();
    expect(timeAgo(now)).toBe('just now');
  });

  it('returns minutes ago', () => {
    const date = new Date(Date.now() - 5 * 60 * 1000);
    expect(timeAgo(date)).toBe('5m ago');
  });

  it('returns hours ago', () => {
    const date = new Date(Date.now() - 3 * 60 * 60 * 1000);
    expect(timeAgo(date)).toBe('3h ago');
  });

  it('returns days ago', () => {
    const date = new Date(Date.now() - 2 * 24 * 60 * 60 * 1000);
    expect(timeAgo(date)).toBe('2d ago');
  });
});

describe('formatDate', () => {
  it('formats date nicely', () => {
    const date = new Date('2024-06-15');
    const formatted = formatDate(date);
    expect(formatted).toContain('2024');
    expect(formatted).toContain('Jun');
  });
});
