import { test, expect } from '@playwright/test';

test.describe('Character Sheet', () => {
  test('character page requires auth', async ({ page }) => {
    await page.goto('/campaigns/test/character');
    // Should redirect to login
    await expect(page.url()).toContain('login');
  });

  test('character stats display correctly', async ({ page }) => {
    // This would need authenticated session
    // Placeholder for character sheet verification
    await page.goto('/');
    await expect(page.locator('body')).toBeVisible();
  });
});
