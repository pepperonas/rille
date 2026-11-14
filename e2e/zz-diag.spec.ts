import { expect, test } from '@playwright/test';
test('diag crossfader keys', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    w.__keys = [];
    window.addEventListener('keydown', (e) => (w.__keys as string[]).push(`${e.code}:${e.shiftKey}:${e.defaultPrevented}`), true);
  });
  await page.goto('/');
  const snap = async (label: string) =>
    console.log(label, JSON.stringify(await page.evaluate(() => ({
      focus: document.hasFocus(), vis: document.visibilityState, active: document.activeElement?.tagName,
      dialog: !!document.querySelector('dialog[open]'),
      keys: (window as unknown as Record<string, unknown>).__keys,
      now: document.querySelector('[aria-label=Crossfader]')?.getAttribute('aria-valuenow'),
    }))));
  await snap('before');
  await page.keyboard.press('Shift+v');
  await page.waitForTimeout(1000);
  await snap('after Shift+v');
  await page.keyboard.press('n');
  await page.waitForTimeout(1000);
  await snap('after n');
  await page.mouse.click(5, 300);
  await page.keyboard.press('Shift+v');
  await page.waitForTimeout(1000);
  await snap('after click + Shift+v');
  expect(true).toBe(true);
});
