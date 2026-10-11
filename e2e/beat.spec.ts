import { expect, test, type Page } from '@playwright/test';

const deck = (page: Page, d: 'a' | 'b') => page.locator(`section[data-deck='${d}']`);
const pads = (page: Page, d: 'a' | 'b') => deck(page, d).getByRole('group', { name: /^Pads/ });
const elapsed = (page: Page, d: 'a' | 'b') => deck(page, d).getByLabel('Gespielt');

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  await page.goto('/?demo');
  await expect(deck(page, 'a').getByLabel('Track-Tempo')).toBeVisible();
});

test.afterEach(() => {
  expect(errors).toEqual([]);
});

/** Wait for the demo to start the deck, then stop it (positions must stay put). */
async function pause(page: Page, d: 'a' | 'b') {
  await deck(page, d).getByRole('button', { name: 'Pause' }).click();
  await expect(deck(page, d).getByRole('button', { name: 'Play' })).toBeVisible();
}

test('hot cues: set, jump back, delete with shift', async ({ page }) => {
  const a = deck(page, 'a');
  await pause(page, 'a');
  await a.getByRole('slider', { name: 'Position Deck 1' }).focus();
  await page.keyboard.press('Home');
  await page.keyboard.press('Shift+ArrowRight'); // 0:50
  await expect(elapsed(page, 'a')).toHaveText('0:50.0');
  await pads(page, 'a').getByRole('button', { name: 'Hot Cue 3 setzen' }).click();
  const cue = pads(page, 'a').getByRole('button', { name: 'Hot Cue 3 anspringen' });
  await expect(cue).toHaveAttribute('aria-pressed', 'true');
  // The overview marks it.
  await expect(a.getByRole('slider', { name: 'Position Deck 1' })).toContainText('3');

  await a.getByRole('slider', { name: 'Position Deck 1' }).focus();
  await page.keyboard.press('Home');
  await expect(elapsed(page, 'a')).toHaveText('0:00.0');
  await cue.click();
  // Snapped to the beat nearest 0:50.0 (124 BPM grid): within one beat.
  await expect(elapsed(page, 'a')).toHaveText(/^0:(49|50)\.\d$/);

  await cue.click({ modifiers: ['Shift'] });
  await expect(pads(page, 'a').getByRole('button', { name: 'Hot Cue 3 setzen' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
});

test('beat loop: on, its pad lit, the playhead stays inside, off again', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('radio', { name: 'Loop' }).click();
  const loop = pads(page, 'a').getByRole('button', { name: 'Loop 1 Beats' });
  await loop.click();
  await expect(loop).toHaveAttribute('aria-pressed', 'true');
  // One beat at 122 BPM is ~0.5 s: after 1.5 s the clock has not moved on by more than that.
  const start = await elapsed(page, 'a').textContent();
  await page.waitForTimeout(1500);
  const seconds = (t: string | null) => {
    const [m = '0', s = '0'] = (t ?? '0:0').split(':');
    return Number(m) * 60 + Number(s);
  };
  const now = await elapsed(page, 'a').textContent();
  expect(Math.abs(seconds(now) - seconds(start))).toBeLessThan(0.6);
  await loop.click();
  await expect(loop).toHaveAttribute('aria-pressed', 'false');
});

test('beat jump moves by beats', async ({ page }) => {
  const a = deck(page, 'a');
  await pause(page, 'a');
  await a.getByRole('slider', { name: 'Position Deck 1' }).focus();
  await page.keyboard.press('Home');
  await a.getByRole('radio', { name: 'Jump' }).click();
  // 16 beats at 122 BPM = 7.87 s.
  await pads(page, 'a').getByRole('button', { name: 'Vor 16 Beats' }).click();
  await expect(elapsed(page, 'a')).toHaveText('0:07.8');
  await pads(page, 'a').getByRole('button', { name: 'Zurück 4 Beats' }).click();
  await expect(elapsed(page, 'a')).toHaveText('0:05.9');
});

test('sync matches the other deck, lock follows it', async ({ page }) => {
  const b = deck(page, 'b');
  // Deck 1: 122 BPM at +2.40 % = 124.93; deck 2: 124 BPM.
  await b.getByRole('button', { name: 'SYNC', exact: true }).click();
  await expect(b.getByText('+0.75 %')).toBeVisible();
  const lock = b.getByRole('button', { name: 'Sync Lock' });
  await lock.click();
  await expect(lock).toHaveAttribute('aria-pressed', 'true');
  // Deck 1 slows to 0 %: deck 2 follows to 122 BPM = −1.61 %.
  await deck(page, 'a').getByRole('slider', { name: 'Tempo' }).dblclick();
  await expect(b.getByText('−1.61 %')).toBeVisible();
});

test('the beatgrid editor halves, doubles and taps the tempo', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Beatgrid Deck 1 korrigieren' }).click();
  const panel = page.getByRole('dialog', { name: 'Beatgrid Deck 1' });
  await panel.getByRole('button', { name: 'Tempo verdoppeln' }).click();
  await expect(a.getByLabel('Track-Tempo')).toHaveText('244.0 BPM');
  await panel.getByRole('button', { name: 'Tempo halbieren' }).click();
  await expect(a.getByLabel('Track-Tempo')).toHaveText('122.0 BPM');
  // Two quick clicks count twice, although the library echoes only every 250 ms.
  await panel.getByRole('button', { name: 'Tempo halbieren' }).click();
  await panel.getByRole('button', { name: 'Tempo halbieren' }).click();
  await expect(a.getByLabel('Track-Tempo')).toHaveText('30.5 BPM');
  await panel.getByRole('button', { name: 'Tempo verdoppeln' }).click();
  await panel.getByRole('button', { name: 'Tempo verdoppeln' }).click();
  await expect(a.getByLabel('Track-Tempo')).toHaveText('122.0 BPM');
  await panel.getByRole('button', { name: 'Tempo +0,01 BPM' }).click();
  await expect(a.getByLabel('Track-Tempo')).toHaveText('122.0 BPM'); // shown with one decimal
  // Tapping sets the tempo the taps had (browser clicks are not metronome-exact: ±15 %).
  for (let i = 0; i < 5; i++) {
    await panel.getByRole('button', { name: 'Tap Tempo' }).click();
    await page.waitForTimeout(500);
  }
  await expect(panel).toContainText(/\d+\.\d\d BPM/);
  const bpm = Number((await a.getByLabel('Track-Tempo').textContent())?.split(' ')[0]);
  expect(bpm).toBeGreaterThan(100);
  expect(bpm).toBeLessThan(125);
  expect(bpm).not.toBe(122);
  await page.keyboard.press('Escape');
  await expect(panel).toHaveCount(0);
});
