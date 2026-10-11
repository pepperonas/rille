import { expect, test, type Page } from '@playwright/test';

const library = (page: Page) => page.getByRole('region', { name: 'Library' });
const list = (page: Page) => page.getByRole('listbox', { name: 'Tracks' });
const deck = (page: Page, d: 'a' | 'b') => page.locator(`section[data-deck='${d}']`);
/** Cells of a row: status, title, artist, BPM, duration. */
const cell = (n: 2 | 3 | 4 | 5) => `:scope > span:nth-child(${n})`;

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

test('an empty library explains itself and imports a folder with progress', async ({ page }) => {
  await page.goto('/');
  await expect(library(page)).toContainText('Noch keine Tracks');
  await library(page).getByRole('button', { name: 'Ordner importieren' }).click();
  await expect(library(page).getByRole('status')).toContainText(/Analysiere \d+ von 12/);
  await expect(list(page).getByRole('option')).toHaveCount(12);
  // Analysis finishes, the progress disappears, tempos show up.
  await expect(library(page).getByRole('status')).toHaveCount(0, { timeout: 5000 });
  await expect(list(page).getByLabel('Wird analysiert')).toHaveCount(0);
  await expect(library(page)).toContainText('12 Tracks');
});

test.describe('with a library', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/?demo');
    await expect(library(page)).toContainText('64 Tracks');
  });

  test('the list is virtualised', async ({ page }) => {
    const rendered = await list(page).getByRole('option').count();
    expect(rendered).toBeGreaterThan(5);
    expect(rendered).toBeLessThan(64);
    await list(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
    // The rows follow the scroll position on the next render.
    await expect
      .poll(() =>
        list(page)
          .getByRole('option')
          .evaluateAll((rows) => rows.map((r) => r.id)),
      )
      .toContain('track-64');
  });

  test('search filters by text and tempo', async ({ page }) => {
    const search = page.getByRole('searchbox', { name: 'Library durchsuchen' });
    await search.fill('ada lux');
    const rows = list(page).getByRole('option');
    const count = await rows.count();
    expect(count).toBeGreaterThan(0);
    for (const text of await rows.locator(cell(3)).allTextContents()) expect(text).toBe('Ada Lux');
    await expect(library(page)).toContainText(`${count} von 64`);

    await search.fill('zzz nichts');
    await expect(library(page)).toContainText('Keine Treffer');
  });

  test('sorting by BPM orders the column and puts unknown tempos last', async ({ page }) => {
    await page.getByRole('button', { name: 'Nach BPM sortieren' }).click();
    await expect(page.getByRole('button', { name: 'Nach BPM sortieren' })).toHaveAttribute(
      'aria-sort',
      'ascending',
    );
    const bpms = async () =>
      (await list(page).getByRole('option').locator(cell(4)).allTextContents())
        .filter((t) => t !== '–')
        .map(Number);
    const asc = await bpms();
    expect(asc).toEqual([...asc].sort((a, b) => a - b));
    await list(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expect(list(page).getByRole('option').last().locator(cell(4))).toHaveText('–');
    await list(page).evaluate((el) => (el.scrollTop = 0));
    await page.getByRole('button', { name: 'Nach BPM sortieren' }).click();
    const desc = await bpms();
    expect(desc).toEqual([...desc].sort((a, b) => b - a));
    // Unknown tempos stay at the bottom in both directions.
    await list(page).evaluate((el) => (el.scrollTop = el.scrollHeight));
    await expect(list(page).getByRole('option').last().locator(cell(4))).toHaveText('–');
  });

  test('keyboard: move the selection and load onto a deck', async ({ page }) => {
    await deck(page, 'a').getByRole('button', { name: 'Deck 1 auswerfen' }).click();
    await deck(page, 'b').getByRole('button', { name: 'Deck 2 auswerfen' }).click();
    await list(page).focus();
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('ArrowDown');
    const selected = list(page).getByRole('option', { selected: true });
    await expect(selected).toHaveCount(1);
    const title = (await selected.locator(cell(2)).textContent()) ?? '';

    await page.keyboard.press('2');
    await expect(deck(page, 'b').getByRole('heading')).toHaveText(title);
    await expect(deck(page, 'b').getByLabel('Track-Tempo')).toContainText('BPM');

    // Enter loads onto the first empty deck (deck 1 here).
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    await expect(deck(page, 'a').getByRole('heading')).not.toHaveText('Kein Track geladen');
    await expect(deck(page, 'a')).not.toContainText('Kein Track geladen');
  });

  test('double-click loads, drag onto a deck loads there', async ({ page }) => {
    const rows = list(page).getByRole('option');
    const third = rows.nth(2);
    const title = (await third.locator(cell(2)).textContent()) ?? '';
    // Drag with the mouse onto deck 2.
    const from = await third.boundingBox();
    const to = await deck(page, 'b').boundingBox();
    if (!from || !to) throw new Error('no layout');
    await page.mouse.move(from.x + 40, from.y + from.height / 2);
    await page.mouse.down();
    await page.mouse.move(from.x + 60, from.y - 40, { steps: 4 });
    await page.mouse.move(to.x + to.width / 2, to.y + to.height / 2, { steps: 8 });
    await expect(deck(page, 'b')).toHaveAttribute('data-drop-target', '');
    await page.mouse.up();
    await expect(deck(page, 'b').getByRole('heading')).toHaveText(title);
    await expect(deck(page, 'b')).not.toHaveAttribute('data-drop-target');

    const fifth = rows.nth(4);
    const other = (await fifth.locator(cell(2)).textContent()) ?? '';
    await deck(page, 'a').getByRole('button', { name: 'Deck 1 auswerfen' }).click();
    await fifth.dblclick();
    await expect(deck(page, 'a').getByRole('heading')).toHaveText(other);
  });

  test('after filtering, Enter loads what is shown, not a hidden selection', async ({ page }) => {
    await deck(page, 'a').getByRole('button', { name: 'Deck 1 auswerfen' }).click();
    await list(page).getByRole('option').first().click();
    const search = page.getByRole('searchbox', { name: 'Library durchsuchen' });
    await search.fill('dub atlas');
    const first = list(page).getByRole('option').first();
    await expect(first).toHaveAttribute('aria-selected', 'true');
    const title = (await first.locator(cell(2)).textContent()) ?? '';
    await search.press('Enter');
    await expect(deck(page, 'a').getByRole('heading')).toHaveText(title);
    await expect(deck(page, 'a')).toContainText('Dub Atlas');
  });

  test('a short press is a click, not a drag', async ({ page }) => {
    const before = await deck(page, 'b').getByRole('heading').textContent();
    const row = list(page).getByRole('option').nth(1);
    await row.click();
    await expect(row).toHaveAttribute('aria-selected', 'true');
    await expect(deck(page, 'b').getByRole('heading')).toHaveText(before ?? '');
  });
});
