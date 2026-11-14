import { test } from '@playwright/test';
test('diag crossfader keys immediate', async ({ page }) => {
  await page.addInitScript(() => {
    const w = window as unknown as Record<string, unknown>;
    const log: string[] = [];
    w.__log = log;
    const t = () => Math.round(performance.now());
    window.addEventListener('load', () => log.push(`load@${t()}`));
    window.addEventListener('keydown', (e) => log.push(`key ${e.code}@${t()}`), true);
    const orig = window.addEventListener.bind(window);
    window.addEventListener = ((type: string, ...rest: unknown[]) => {
      if (type === 'keydown') log.push(`listener-added@${t()}`);
      return (orig as (...a: unknown[]) => void)(type, ...rest);
    }) as typeof window.addEventListener;
    new MutationObserver((_, o) => {
      if (document.querySelector('[aria-label=Crossfader]')) {
        log.push(`slider@${t()}`);
        o.disconnect();
      }
    }).observe(document, { childList: true, subtree: true });
  });
  await page.goto('/');
  await page.keyboard.press('Shift+v');
  await page.waitForTimeout(1500);
  console.log('LOG', JSON.stringify(await page.evaluate(() => (window as unknown as Record<string, unknown>).__log)));
  console.log('NOW', await page.getByRole('slider', { name: 'Crossfader' }).getAttribute('aria-valuenow'));
});
