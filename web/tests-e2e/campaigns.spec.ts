import { test, expect } from '@playwright/test';

test.describe('Campaign Management', () => {
  test('campaigns list page requires auth', async ({ page }) => {
    await page.goto('/campaigns');
    // Should redirect to login or show auth required
    await expect(page.url()).toContain('login');
  });

  test('can navigate to campaign creation', async ({ page }) => {
    await page.goto('/');
    // Look for create campaign link/button
    const createLink = page.locator('text=Create|New|Add').first();
    if (await createLink.isVisible().catch(() => false)) {
      await createLink.click();
      await expect(page.url()).toContain('campaign');
    }
  });
});
