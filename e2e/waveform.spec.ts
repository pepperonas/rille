import { expect, test, type Page } from '@playwright/test';

const deck = (page: Page, d: 'a' | 'b') => page.locator(`section[data-deck='${d}']`);
const detail = (page: Page, d: 'a' | 'b') =>
  deck(page, d).getByRole('img', { name: /^Waveform Deck/ });
const draws = (page: Page) =>
  page.evaluate(() => (window as { __rilleWaveDraws?: number }).__rilleWaveDraws ?? 0);

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error' || m.type() === 'warning') errors.push(m.text());
  });
  await page.goto('/?demo');
  await expect(detail(page, 'a')).toHaveAttribute('data-peaks', 'ready');
});

test.afterEach(() => {
  expect(errors).toEqual([]);
});

test('both decks show detail and overview once the peaks arrive', async ({ page }) => {
  for (const d of ['a', 'b'] as const) {
    await expect(detail(page, d)).toHaveAttribute('data-peaks', 'ready');
    await expect(deck(page, d).locator('[data-peaks="ready"]')).toHaveCount(2);
  }
  // The WebGL canvas really drew something: compare its pixels with the empty background.
  // (A WebGL canvas cannot be read back after compositing, so go through a screenshot.)
  const shot = (await detail(page, 'a').locator('canvas').screenshot()).toString('base64');
  const painted = await page.evaluate(async (png) => {
    const img = new Image();
    img.src = `data:image/png;base64,${png}`;
    await img.decode();
    const c = document.createElement('canvas');
    c.width = img.width;
    c.height = img.height;
    const g = c.getContext('2d');
    if (!g) return 0;
    g.drawImage(img, 0, 0);
    const d = g.getImageData(0, 0, c.width, c.height).data;
    // Background = the top-left corner pixel (above the waveform's tallest peaks).
    const [r0, g0, b0] = [d[0] ?? 0, d[1] ?? 0, d[2] ?? 0];
    let n = 0;
    for (let i = 0; i < d.length; i += 4) {
      const diff =
        Math.abs((d[i] ?? 0) - r0) +
        Math.abs((d[i + 1] ?? 0) - g0) +
        Math.abs((d[i + 2] ?? 0) - b0);
      if (diff > 30) n++;
    }
    return n / (d.length / 4);
  }, shot);
  expect(painted).toBeGreaterThan(0.05);
});

test('nothing is drawn while both decks stand still; playing draws every frame', async ({
  page,
}) => {
  for (const d of ['a', 'b'] as const) {
    await deck(page, d).getByRole('button', { name: 'Pause' }).click();
  }
  await page.waitForTimeout(300);
  const before = await draws(page);
  await page.waitForTimeout(1000);
  expect(await draws(page)).toBe(before);

  await deck(page, 'a').getByRole('button', { name: 'Play' }).click();
  const start = await draws(page);
  await page.waitForTimeout(1000);
  expect((await draws(page)) - start).toBeGreaterThan(20);
});

test('a click into the overview seeks there', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Pause' }).click();
  const overview = a.locator('[title="Klicken springt an diese Stelle"]');
  const box = await overview.boundingBox();
  if (!box) throw new Error('no overview');
  // A quarter into the 3:30 demo track: ~0:52.
  await page.mouse.click(box.x + box.width * 0.25, box.y + box.height / 2);
  await expect(a.getByLabel('Gespielt')).toHaveText(/^0:5[1-3]\.\d$/);
});

test('the mouse wheel and +/− zoom the detail view, shared by both decks', async ({ page }) => {
  const a = detail(page, 'a');
  await expect(a).toHaveAttribute('aria-label', /8 Sekunden/);
  await a.hover();
  await page.mouse.wheel(0, 100);
  await expect(a).toHaveAttribute('aria-label', /16 Sekunden/);
  await expect(detail(page, 'b')).toHaveAttribute('aria-label', /16 Sekunden/);
  await a.focus();
  await page.keyboard.press('+');
  await page.keyboard.press('+');
  await expect(a).toHaveAttribute('aria-label', /4 Sekunden/);
  for (let i = 0; i < 6; i++) await page.keyboard.press('+');
  await expect(a).toHaveAttribute('aria-label', /2 Sekunden/);
});

test('the remaining time warns in the last 30 seconds', async ({ page }) => {
  // Demo deck 2 plays at 3:13 of 3:30.
  const remaining = deck(page, 'b').getByLabel('Restzeit');
  await expect(remaining.locator('xpath=..').locator('xpath=..')).toHaveAttribute(
    'data-warn',
    'true',
  );
  await expect(
    deck(page, 'a').getByLabel('Restzeit').locator('xpath=..').locator('xpath=..'),
  ).toHaveAttribute('data-warn', 'false');
  const animation = await remaining.evaluate((el) => getComputedStyle(el).animationName);
  expect(animation).not.toBe('none');
});

test('reduced motion keeps the warning colour but stops the blinking', async ({ browser }) => {
  const context = await browser.newContext({ reducedMotion: 'reduce' });
  const page = await context.newPage();
  await page.goto('/?demo');
  const remaining = deck(page, 'b').getByLabel('Restzeit');
  await expect(remaining.locator('xpath=../..')).toHaveAttribute('data-warn', 'true');
  expect(await remaining.evaluate((el) => getComputedStyle(el).animationName)).toBe('none');
  await context.close();
});
