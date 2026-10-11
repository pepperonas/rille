import { expect, test, type Page } from '@playwright/test';

const deck = (page: Page, d: 'a' | 'b') => page.locator(`section[data-deck='${d}']`);

let errors: string[] = [];

test.beforeEach(({ page }) => {
  errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
});

test.afterEach(() => {
  expect(errors).toEqual([]);
});

async function loadDemo(page: Page, d: 'a' | 'b') {
  await deck(page, d).getByRole('button', { name: 'Demo-Track laden' }).click();
  await expect(deck(page, d).getByRole('button', { name: 'Play' })).toBeVisible();
}

test('without an output device a notice offers the fix', async ({ page }) => {
  await page.goto('/?noaudio');
  const notice = page.getByRole('alert').filter({ hasText: 'Kein Audio-Gerät' });
  await expect(notice).toBeVisible();
  await notice.getByRole('button', { name: 'Ausgabe wählen' }).click();
  const settings = page.getByRole('dialog', { name: 'Einstellungen' });
  await expect(settings).toBeVisible();
  await settings.getByLabel('Puffergröße').selectOption('512');
  await expect(notice).toBeHidden();
});

test('empty decks point to the shortcut overview', async ({ page }) => {
  await page.goto('/');
  await expect(deck(page, 'a')).toContainText('zeigt alle Tastenkürzel');
});

test('deck keys: play, cue, pads, pad mode, sync lock', async ({ page }) => {
  await page.goto('/');
  await loadDemo(page, 'a');
  const a = deck(page, 'a');

  await page.keyboard.press('f');
  await expect(a.getByRole('button', { name: 'Pause' })).toHaveAttribute('aria-pressed', 'true');
  await expect(a.getByLabel('Gespielt')).not.toHaveText('0:00.0', { timeout: 2000 });
  await page.keyboard.press('f');
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();
  await page.keyboard.press('f');
  // D while playing: back to the cue point and stop.
  await page.keyboard.press('d');
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');

  // Hold D: preview plays while held, back to the cue point on release.
  await page.keyboard.down('d');
  await expect(a.getByLabel('Gespielt')).not.toHaveText('0:00.0', { timeout: 2000 });
  await page.keyboard.up('d');
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');

  await page.keyboard.press('2');
  await expect(a.getByRole('button', { name: 'Hot Cue 2 anspringen' })).toBeVisible();
  await page.keyboard.press('Shift+2');
  await expect(a.getByRole('button', { name: 'Hot Cue 2 setzen' })).toBeVisible();

  await page.keyboard.press('5');
  await expect(a.getByRole('radio', { name: 'Loop' })).toHaveAttribute('aria-checked', 'true');

  // Deck 2 keys reach deck 2 only.
  await loadDemo(page, 'b');
  await page.keyboard.press('j');
  await expect(deck(page, 'b').getByRole('button', { name: 'Pause' })).toBeVisible();
  await expect(a.getByRole('button', { name: 'Play' })).toBeVisible();

  await page.keyboard.press('Shift+s');
  await expect(a.getByRole('button', { name: 'Sync Lock' })).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Shift+s');
  await expect(a.getByRole('button', { name: 'Sync Lock' })).toHaveAttribute('aria-pressed', 'false');
});

test('crossfader keys', async ({ page }) => {
  await page.goto('/');
  const xf = page.getByRole('slider', { name: 'Crossfader' });
  await page.keyboard.press('Shift+v');
  await expect(xf).toHaveAttribute('aria-valuenow', '0');
  await page.keyboard.press('n');
  await expect(xf).toHaveAttribute('aria-valuenow', '10');
  await page.keyboard.press('b');
  await expect(xf).toHaveAttribute('aria-valuenow', '50');
});

test('shortcuts rest while typing and while a dialog is open', async ({ page }) => {
  await page.goto('/');
  await loadDemo(page, 'a');
  const play = deck(page, 'a').getByRole('button', { name: 'Play' });
  // Real key presses (fill() would not fire keydown at all).
  await page.getByLabel('Library durchsuchen').pressSequentially('fj');
  await expect(page.getByLabel('Library durchsuchen')).toHaveValue('fj');
  // A key that did reach the deck needs a round trip to show; give it the time.
  await page.waitForTimeout(400);
  await expect(play).toBeVisible();

  await page.getByLabel('Library durchsuchen').blur();
  await page.keyboard.press('?');
  const overview = page.getByRole('dialog', { name: 'Tastenkürzel' });
  await expect(overview).toBeVisible();
  await expect(overview).toContainText('Pitch-Bend');
  await page.keyboard.press('f');
  await page.waitForTimeout(400);
  await page.keyboard.press('Escape');
  await expect(play).toBeVisible();
  await expect(overview).toBeHidden();

  await page.keyboard.press('Meta+f');
  await expect(page.getByLabel('Library durchsuchen')).toBeFocused();
});

test('a held key lets go when the window loses focus', async ({ page }) => {
  await page.goto('/');
  await loadDemo(page, 'a');
  const a = deck(page, 'a');
  await page.keyboard.down('d');
  await expect(a.getByLabel('Gespielt')).not.toHaveText('0:00.0', { timeout: 2000 });
  await page.evaluate(() => window.dispatchEvent(new Event('blur')));
  await expect(a.getByLabel('Gespielt')).toHaveText('0:00.0');
  await page.keyboard.up('d');
});

test('light theme applies at once and survives a reload', async ({ page }) => {
  await page.goto('/');
  const dark = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  await page.getByLabel('Erscheinungsbild').selectOption('light');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-window-theme', 'light');
  const light = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
  expect(light).not.toBe(dark);

  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
});

test('system theme follows the OS', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto('/');
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  await page.getByLabel('Erscheinungsbild').selectOption('system');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await expect(page.locator('html')).toHaveAttribute('data-window-theme', 'system');
  await page.emulateMedia({ colorScheme: 'dark' });
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});

test('start tempo range and crossfader curve are saved', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  await page.getByLabel('Tempo-Bereich beim Start').selectOption('sixteen');
  await page.getByRole('combobox', { name: 'Crossfader-Kurve' }).selectOption('cut');
  await page.keyboard.press('Escape');
  await loadDemo(page, 'a');
  await expect(deck(page, 'a').getByRole('button', { name: /^Tempobereich ±16/ })).toBeVisible();
  await expect(page.getByRole('radio', { name: 'Cut' })).toHaveAttribute('aria-checked', 'true');

  // The mixer chips change the saved curve too.
  await page.getByRole('radio', { name: 'Linear' }).click();
  await page.reload();
  await expect(page.getByRole('radio', { name: 'Linear' })).toHaveAttribute('aria-checked', 'true');
  await loadDemo(page, 'b');
  await expect(deck(page, 'b').getByRole('button', { name: /^Tempobereich ±16/ })).toBeVisible();
});

test('keys the track list uses do not also hit the pads', async ({ page }) => {
  await page.goto('/?demo');
  const a = deck(page, 'a');
  await expect(a.getByRole('button', { name: 'Hot Cue 2 setzen' })).toBeVisible();
  const list = page.getByRole('listbox', { name: 'Tracks' });
  await list.focus();
  await page.keyboard.press('ArrowDown');
  await page.keyboard.press('2');
  await expect(deck(page, 'b').getByRole('heading')).not.toHaveText('Neon Avenue (Club Edit)');
  await page.waitForTimeout(400);
  await expect(a.getByRole('button', { name: 'Hot Cue 2 setzen' })).toBeVisible();
});
