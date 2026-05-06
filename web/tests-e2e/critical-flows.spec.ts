import { test, expect } from '@playwright/test';

test.describe('Critical User Flows', () => {
  test.beforeEach(async ({ page }) => {
    // Start at homepage
    await page.goto('/');
  });

  test('complete auth flow: register → login → logout', async ({ page }) => {
    // Navigate to login
    await page.click('text=Login');
    await expect(page).toHaveURL(/.*login/);

    // Register new account
    await page.fill('input[type="email"]', `test-${Date.now()}@example.com`);
    await page.fill('input[type="password"]', 'Test123!Pass');
    await page.fill('input[name="display_name"]', 'TestUser');
    await page.click('button[type="submit"]');

    // Should redirect to campaigns
    await expect(page).toHaveURL(/.*campaigns/);

    // Logout
    await page.click('text=Logout');
    await expect(page).toHaveURL(/.*login|.*\/$/);
  });

  test('campaign creation flow', async ({ page }) => {
    // Login first (assume test account exists)
    await page.goto('/login');
    await page.fill('input[type="email"]', 'test@example.com');
    await page.fill('input[type="password"]', 'Test123!Pass');
    await page.click('button[type="submit"]');

    // Create campaign
    await page.click('text=Create|New Campaign');
    await page.fill('input[name="name"]', `Test Campaign ${Date.now()}`);
    await page.selectOption('select[name="leveling"]', 'xp');
    await page.click('button:has-text("Create")');

    // Should be on campaign page
    await expect(page).toHaveURL(/.*campaigns\/[^/]+/);
  });

  test('character creation flow', async ({ page }) => {
    // Navigate to a campaign
    await page.goto('/campaigns');
    await page.click('.campaign-card:first-child');

    // Create character
    await page.click('text=New Character|Create Character');
    await page.fill('input[name="name"]', 'Test Character');
    await page.fill('input[name="race"]', 'Human');
    await page.fill('input[name="class_primary"]', 'Fighter');
    await page.fill('input[name="level_total"]', '1');
    await page.click('button:has-text("Save")');

    // Should show character sheet
    await expect(page.locator('text=Test Character')).toBeVisible();
  });
});
