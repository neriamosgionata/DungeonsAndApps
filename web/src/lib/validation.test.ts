import { describe, it, expect } from 'vitest';

/**
 * Validation utilities
 */

function isValidEmail(email: string): boolean {
  return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email);
}

function isValidPassword(password: string): {
  valid: boolean;
  errors: string[];
} {
  const errors: string[] = [];

  if (password.length < 8) {
    errors.push('Password must be at least 8 characters');
  }

  const hasUpper = /[A-Z]/.test(password);
  const hasLower = /[a-z]/.test(password);
  const hasDigit = /\d/.test(password);
  const hasSpecial = /[!@#$%^&*()_+\-=\[\]{};':"\\|,.<>\/?]/.test(password);

  const types = [hasUpper, hasLower, hasDigit, hasSpecial].filter(Boolean).length;
  if (types < 3) {
    errors.push('Password must contain at least 3 of: uppercase, lowercase, digits, special characters');
  }

  return { valid: errors.length === 0, errors };
}

function isValidCampaignName(name: string): boolean {
  return name.length >= 1 && name.length <= 100;
}

function sanitizeHtml(input: string): string {
  return input
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

// =====================================================================
// Tests
// =====================================================================

describe('isValidEmail', () => {
  it('accepts valid emails', () => {
    expect(isValidEmail('test@example.com')).toBe(true);
    expect(isValidEmail('user.name@domain.co.uk')).toBe(true);
  });

  it('rejects invalid emails', () => {
    expect(isValidEmail('not-an-email')).toBe(false);
    expect(isValidEmail('@example.com')).toBe(false);
    expect(isValidEmail('test@')).toBe(false);
    expect(isValidEmail('')).toBe(false);
  });
});

describe('isValidPassword', () => {
  it('accepts strong password', () => {
    const result = isValidPassword('Test123!Pass');
    expect(result.valid).toBe(true);
    expect(result.errors).toHaveLength(0);
  });

  it('rejects short password', () => {
    const result = isValidPassword('Test1!');
    expect(result.valid).toBe(false);
    expect(result.errors).toContain('Password must be at least 8 characters');
  });

  it('rejects weak password (only 2 char types)', () => {
    const result = isValidPassword('testtest1');
    expect(result.valid).toBe(false);
    expect(result.errors.some(e => e.includes('3 of'))).toBe(true);
  });

  it('accepts password with exactly 3 char types', () => {
    const result = isValidPassword('Testtest1'); // upper, lower, digit
    expect(result.valid).toBe(true);
  });
});

describe('isValidCampaignName', () => {
  it('accepts valid names', () => {
    expect(isValidCampaignName('My Campaign')).toBe(true);
    expect(isValidCampaignName('A')).toBe(true);
  });

  it('rejects empty names', () => {
    expect(isValidCampaignName('')).toBe(false);
  });

  it('rejects very long names', () => {
    expect(isValidCampaignName('A'.repeat(101))).toBe(false);
  });
});

describe('sanitizeHtml', () => {
  it('escapes HTML tags', () => {
    expect(sanitizeHtml('<script>alert("xss")</script>'))
      .toBe('&lt;script&gt;alert(&quot;xss&quot;)&lt;/script&gt;');
  });

  it('escapes ampersands', () => {
    expect(sanitizeHtml('A & B')).toBe('A &amp; B');
  });

  it('handles empty string', () => {
    expect(sanitizeHtml('')).toBe('');
  });

  it('leaves safe text unchanged', () => {
    expect(sanitizeHtml('Hello World')).toBe('Hello World');
  });
});
