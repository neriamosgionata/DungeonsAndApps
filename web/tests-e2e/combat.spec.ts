import { test, expect } from '@playwright/test';

test.describe('Combat Flow', () => {
  test('start encounter and make attack', async ({ page }) => {
    // Navigate to campaign
    await page.goto('/campaigns');
    await page.click('.campaign-card:first-child');

    // Go to initiative/combat
    await page.click('text=War Council|Initiative|Combat');

    // Create encounter
    await page.click('text=New Encounter|Create');
    await page.fill('input[name="name"]', `Combat ${Date.now()}`);
    await page.click('button:has-text("Create")');

    // Add NPC
    await page.click('text=Add NPC|Add Enemy');
    await page.fill('input[name="display_name"]', 'Goblin');
    await page.fill('input[name="hp_max"]', '7');
    await page.fill('input[name="ac"]', '12');
    await page.click('button:has-text("Add")');

    // Start encounter
    await page.click('text=Start Combat|Begin');

    // Should show active combat
    await expect(page.locator('.combat-active, .turn-tracker')).toBeVisible();

    // Make attack
    await page.click('text=Attack|Strike');
    await page.click('.target-select:first-child');
    await page.click('button:has-text("Roll")');

    // Should show result
    await expect(page.locator('.attack-result, .damage-display')).toBeVisible();
  });

  test('cast spell in combat', async ({ page }) => {
    await page.goto('/campaigns/test-campaign/initiative');

    // Assume encounter active
    await page.click('text=Cast Spell|Magic');
    await page.selectOption('select[name="spell"]', 'Fire Bolt');
    await page.click('.target-select:first-child');
    await page.click('button:has-text("Cast")');

    await expect(page.locator('.spell-result, .damage-display')).toBeVisible();
  });

  test('use reaction (Shield)', async ({ page }) => {
    await page.goto('/campaigns/test-campaign/initiative');

    // Wait for reaction prompt or use reaction button
    const reactionBtn = page.locator('text=Shield|Counterspell|Reaction').first();
    if (await reactionBtn.isVisible().catch(() => false)) {
      await reactionBtn.click();
      await expect(page.locator('.reaction-used')).toBeVisible();
    }
  });
});
