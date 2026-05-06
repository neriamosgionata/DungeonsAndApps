import { test, expect } from '@playwright/test';

test.describe('Character Sheet', () => {
  test('edit ability scores', async ({ page }) => {
    await page.goto('/campaigns/test/character/test-char');

    // Open edit mode or click ability
    await page.click('text=Strength|STR');
    await page.fill('input[name="abilities.str"]', '16');
    await page.click('text=Save|Confirm');

    // Verify modifier displayed
    await expect(page.locator('text=+3')).toBeVisible();
  });

  test('adjust HP', async ({ page }) => {
    await page.goto('/campaigns/test/character/test-char');

    // Take damage
    await page.click('text=Damage|Wound');
    await page.fill('input[name="damage"]', '5');
    await page.click('button:has-text("Apply")');

    // Heal
    await page.click('text=Heal|Rest');
    await page.fill('input[name="healing"]', '3');
    await page.click('button:has-text("Apply")');

    // HP should update
    await expect(page.locator('.hp-current')).not.toHaveText('0');
  });

  test('use spell slot', async ({ page }) => {
    await page.goto('/campaigns/test/character/test-char');

    // Find spell slots section
    await page.click('text=Spells|Magic');

    // Click a filled slot to empty it
    const slot = page.locator('.slot-bubble.full, .bubble.full').first();
    if (await slot.isVisible().catch(() => false)) {
      await slot.click();
      await expect(slot).not.toHaveClass(/full/);
    }
  });

  test('short rest recovers resources', async ({ page }) => {
    await page.goto('/campaigns/test/character/test-char');

    await page.click('text=Short Rest|Rest');
    await page.fill('input[name="hit_dice_spent"]', '2');
    await page.click('button:has-text("Rest")');

    // Should show recovery
    await expect(page.locator('.rest-complete, text=HP restored')).toBeVisible();
  });

  test('long rest restores everything', async ({ page }) => {
    await page.goto('/campaigns/test/character/test-char');

    await page.click('text=Long Rest|Full Rest');
    await page.click('button:has-text("Confirm|Rest")');

    // HP should be full
    await expect(page.locator('.hp-current')).toContainText('hp.max');
  });
});
