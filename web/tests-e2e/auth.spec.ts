import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
  test('homepage loads', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveTitle(/DungeonsAndApps|CinghialApp/i);
  });

  test('login page accessible', async ({ page }) => {
    await page.goto('/login');
    await expect(page.locator('text=Login')).toBeVisible();
  });

  test('login with invalid credentials shows error', async ({ page }) => {
    await page.goto('/login');
    await page.fill('input[type="email"]', 'invalid@test.com');
    await page.fill('input[type="password"]', 'wrongpassword');
    await page.click('button[type="submit"]');
    // Should show error or redirect
    await expect(page.locator('text=Invalid|Error|401').first()).toBeVisible();
  });
});
