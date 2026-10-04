import { expect, test, type Page } from '@playwright/test';

const deck = (page: Page, d: 'a' | 'b') => page.locator(`section[data-deck='${d}']`);

// Tests in one worker run one after another, so a module-level list is safe.
let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  await page.goto('/');
});

// Any runtime error in the page fails the test.
test.afterEach(() => {
  expect(errors).toEqual([]);
});

test('empty decks explain how to load a track', async ({ page }) => {
  await expect(deck(page, 'a')).toContainText('Kein Track geladen');
  await expect(deck(page, 'b')).toContainText('Audiodatei aus dem Finder');
});

test('load, play, cue back', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  await expect(a.getByRole('heading')).toHaveText('Demo Track Deck 1');

  await a.getByRole('button', { name: 'Play' }).click();
  await expect(a.getByRole('button', { name: 'Pause' })).toHaveAttribute('aria-pressed', 'true');
  await expect(a.getByLabel('Gespielt')).not.toHaveText('0:00.0', { timeout: 2000 });

  // Cue while playing: back to the cue point (start) and pause.
  await a.getByRole('button', { name: 'Cue' }).click();
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');
});

test('eject empties the deck', async ({ page }) => {
  const b = deck(page, 'b');
  await b.getByRole('button', { name: 'Demo-Track laden' }).click();
  await b.getByRole('button', { name: 'Deck 2 auswerfen' }).click();
  await expect(b).toContainText('Kein Track geladen');
});

test('faders work with the keyboard', async ({ page }) => {
  const fader = page.getByRole('slider', { name: 'Kanalfader Kanal 1' });
  await expect(fader).toHaveAttribute('aria-valuenow', '100');
  await fader.focus();
  await page.keyboard.press('Shift+ArrowDown');
  await expect(fader).toHaveAttribute('aria-valuenow', '90');
  await page.keyboard.press('Home');
  await expect(fader).toHaveAttribute('aria-valuenow', '0');
});

test('crossfader curve can be chosen', async ({ page }) => {
  const cut = page.getByRole('radio', { name: 'Cut' });
  await cut.click();
  await expect(cut).toHaveAttribute('aria-checked', 'true');
  await expect(page.getByRole('radio', { name: 'Blend' })).toHaveAttribute('aria-checked', 'false');
});

test('settings show audio device and latency, footer has the year', async ({ page }) => {
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  const dialog = page.getByRole('dialog', { name: 'Einstellungen' });
  await expect(dialog).toContainText('5.3 ms');
  await dialog.getByLabel('Puffergröße').selectOption('512');
  await expect(dialog).toContainText('10.7 ms');
  await expect(dialog).toContainText(`© ${new Date().getFullYear()} Martin Pfeffer | celox.io`);
});

test('play and cue work from the keyboard', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  await a.getByRole('button', { name: 'Play' }).focus();
  await page.keyboard.press('Space');
  await expect(a.getByRole('button', { name: 'Pause' })).toBeVisible();
  await a.getByRole('button', { name: 'Cue' }).focus();
  await page.keyboard.press('Enter');
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');
});

test('holding cue previews and releasing with shift still returns', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  const cue = a.getByRole('button', { name: 'Cue' });
  await cue.hover();
  await page.mouse.down();
  await expect(a.getByLabel('Gespielt')).not.toHaveText('0:00.0', { timeout: 2000 });
  await page.keyboard.down('Shift');
  await page.mouse.up();
  await page.keyboard.up('Shift');
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();
});
