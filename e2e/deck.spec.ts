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

test('controller status, vinyl mode and MIDI monitor', async ({ page }) => {
  await expect(page.getByRole('button', { name: 'Kein Controller' })).toBeVisible();
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  const vinyl = page.getByRole('switch', { name: /Vinyl-Modus/ });
  await expect(vinyl).toBeChecked();
  await vinyl.click({ force: true });
  await expect(vinyl).not.toBeChecked();
  await page.getByRole('button', { name: /MIDI-Monitor öffnen/ }).click();
  await expect(page.getByRole('dialog', { name: 'MIDI-Monitor' })).toContainText(
    'Noch keine Nachrichten',
  );
  await page.keyboard.press('Escape');
  await page.keyboard.press('Meta+Alt+KeyM');
  await expect(page.getByRole('dialog', { name: 'MIDI-Monitor' })).toBeVisible();
});

test('about dialog offers donating and rating celox.io', async ({ page }) => {
  await page.addInitScript(() => {
    (window as unknown as { opened: string[] }).opened = [];
    window.open = ((url: string) => {
      (window as unknown as { opened: string[] }).opened.push(url);
      return null;
    }) as typeof window.open;
  });
  await page.reload();
  await page.getByRole('button', { name: 'Über rille' }).click();
  const dialog = page.getByRole('dialog', { name: 'rille' });
  await dialog.getByRole('button', { name: 'Spenden via PayPal' }).click();
  await dialog.getByRole('button', { name: 'celox.io bewerten' }).click();
  const opened = await page.evaluate(() => (window as unknown as { opened: string[] }).opened);
  expect(opened[0]).toContain('paypal.com/donate/?business=martin.pfeffer@celox.io');
  expect(opened[1]).toBe('https://g.page/r/CXgdRV3QysvxEBM/review');
  await expect(dialog).toContainText(`© ${new Date().getFullYear()} Martin Pfeffer | celox.io`);
});

test('EQ kill, filter and transition FX from the mixer', async ({ page }) => {
  await page.goto('/?demo');
  await expect(deck(page, 'b').getByRole('heading')).toHaveText('Neon Avenue (Club Edit)');
  const kill = page.getByRole('button', { name: 'Bässe Kanal 1 stummschalten (Kill)' });
  await kill.click();
  await expect(kill).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('slider', { name: 'Bässe Kanal 1' })).toHaveAttribute('aria-valuetext', 'Kill');

  const filter = page.getByRole('slider', { name: 'Filter Kanal 2' });
  await filter.focus();
  await page.keyboard.press('End');
  await expect(filter).toHaveAttribute('aria-valuetext', 'Hochpass 100 %');
  await filter.dblclick();
  await expect(filter).toHaveAttribute('aria-valuetext', 'aus');

  const fx = page.getByRole('button', { name: /^Transition FX/ });
  await fx.click();
  await expect(fx).toHaveAttribute('aria-pressed', 'true');
  await expect(page.getByRole('button', { name: /^Effekt wechseln/ })).toBeDisabled();
});

for (const [width, height] of [
  [1440, 900],
  [1024, 680],
] as const) {
  test(`mixer fits without overlap at ${width}x${height}`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await page.goto('/?demo');
    await expect(deck(page, 'a').getByRole('heading')).toBeVisible();
    const fader = await page.getByRole('slider', { name: 'Kanalfader Kanal 1' }).boundingBox();
    const cross = await page.getByRole('slider', { name: 'Crossfader' }).boundingBox();
    const fx = await page.getByRole('button', { name: /^Transition FX/ }).boundingBox();
    const mixer = await page.getByRole('region', { name: 'Mixer' }).boundingBox();
    expect(fader && cross && fx && mixer).toBeTruthy();
    if (!fader || !cross || !fx || !mixer) return;
    expect(fader.y + fader.height).toBeLessThanOrEqual(cross.y);
    expect(fader.height).toBeGreaterThanOrEqual(60);
    expect(fx.y + fx.height).toBeLessThanOrEqual(mixer.y + mixer.height);
  });
}

test('quick repeated input accumulates instead of getting lost', async ({ page }) => {
  await page.goto('/?demo');
  await expect(deck(page, 'a').getByRole('heading')).toBeVisible();
  const kill = page.getByRole('button', { name: 'Höhen Kanal 2 stummschalten (Kill)' });
  await kill.dblclick();
  await expect(kill).toHaveAttribute('aria-pressed', 'false');
  const mid = page.getByRole('slider', { name: 'Mitten Kanal 1' });
  await mid.focus();
  for (let i = 0; i < 5; i++) await page.keyboard.press('ArrowUp', { delay: 0 });
  await expect(mid).toHaveAttribute('aria-valuenow', '60');
});

test('tempo fader: top is slower, range chip, double click resets', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  const tempo = a.getByRole('slider', { name: 'Tempo' });
  await expect(tempo).toHaveAttribute('aria-valuetext', '0.00 %');
  await tempo.focus();
  await page.keyboard.press('ArrowUp');
  await expect(tempo).toHaveAttribute('aria-valuetext', '−0.40 %');
  await expect(a.getByText('−0.40 %', { exact: true })).toBeVisible();
  await page.keyboard.press('Home'); // bottom = fastest (the fader's minimum)
  await expect(tempo).toHaveAttribute('aria-valuetext', '+10.00 %');

  const range = a.getByRole('button', { name: /^Tempobereich/ });
  await expect(range).toHaveText('±10 %');
  await range.click();
  await expect(range).toHaveText('±16 %');
  await expect(tempo).toHaveAttribute('aria-valuetext', '+16.00 %');
  await range.dblclick();
  await expect(range).toHaveText('±6 %', { timeout: 2000 });

  await tempo.dblclick();
  await expect(tempo).toHaveAttribute('aria-valuetext', '0.00 %');
});

test('keylock toggles, reverse plays backwards while held', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  const key = a.getByRole('button', { name: 'Keylock' });
  await expect(key).toHaveAttribute('aria-pressed', 'false');
  await key.click();
  await expect(key).toHaveAttribute('aria-pressed', 'true');

  const elapsed = a.getByLabel('Gespielt');
  await a.getByRole('button', { name: 'Play' }).click();
  await expect(elapsed).not.toHaveText(/^0:0[01]\./, { timeout: 4000 });
  const before = await elapsed.textContent();

  const rev = a.getByRole('button', { name: 'Rückwärts (halten)' });
  await rev.hover();
  await page.mouse.down();
  await expect(rev).toHaveAttribute('aria-pressed', 'true');
  await page.waitForTimeout(600);
  const during = await elapsed.textContent();
  await page.mouse.up();
  await expect(rev).toHaveAttribute('aria-pressed', 'false');
  expect(during && before && during < before).toBeTruthy();
});

test('pitch bend acts only while held, also from the keyboard', async ({ page }) => {
  const b = deck(page, 'b');
  await b.getByRole('button', { name: 'Demo-Track laden' }).click();
  const faster = b.getByRole('button', { name: 'Schneller (halten)' });
  await faster.focus();
  await page.keyboard.down('Space');
  await expect(faster).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.up('Space');
  await expect(faster).toHaveAttribute('aria-pressed', 'false');
  // Losing focus while held must not leave the bend stuck.
  await page.keyboard.down('Enter');
  await expect(faster).toHaveAttribute('aria-pressed', 'true');
  await page.keyboard.press('Tab');
  await page.keyboard.up('Enter');
  await expect(faster).toHaveAttribute('aria-pressed', 'false');
});

for (const [width, height] of [
  [1440, 900],
  [1024, 680],
] as const) {
  test(`deck controls fit at ${width}x${height}`, async ({ page }) => {
    await page.setViewportSize({ width, height });
    await page.goto('/?demo');
    for (const d of ['a', 'b'] as const) {
      const panel = deck(page, d);
      await expect(panel.getByRole('heading')).toBeVisible();
      const box = await panel.boundingBox();
      const tempo = await panel.getByRole('slider', { name: 'Tempo' }).boundingBox();
      const play = await panel.getByRole('button', { name: /^(Play|Pause)$/ }).boundingBox();
      const bend = await panel.getByRole('button', { name: 'Schneller (halten)' }).boundingBox();
      const time = await panel.getByLabel('Gespielt').boundingBox();
      expect(box && tempo && play && bend && time).toBeTruthy();
      if (!box || !tempo || !play || !bend || !time) return;
      for (const el of [tempo, play, bend]) {
        expect(el.x).toBeGreaterThanOrEqual(box.x);
        expect(el.x + el.width).toBeLessThanOrEqual(box.x + box.width);
        expect(el.y + el.height).toBeLessThanOrEqual(box.y + box.height);
      }
      expect(tempo.height).toBeGreaterThanOrEqual(60);
      // the bend buttons sit left of the tempo fader, the clock above the transport
      expect(bend.x + bend.width).toBeLessThanOrEqual(tempo.x);
      expect(time.y + time.height).toBeLessThanOrEqual(play.y);
    }
  });
}

test('tempo and keylock survive loading the next track', async ({ page }) => {
  const a = deck(page, 'a');
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  const tempo = a.getByRole('slider', { name: 'Tempo' });
  await tempo.focus();
  await page.keyboard.press('ArrowUp');
  await a.getByRole('button', { name: 'Keylock' }).click();
  await a.getByRole('button', { name: 'Deck 1 auswerfen' }).click();
  await a.getByRole('button', { name: 'Demo-Track laden' }).click();
  await expect(a.getByRole('slider', { name: 'Tempo' })).toHaveAttribute('aria-valuetext', '−0.40 %');
  await expect(a.getByRole('button', { name: 'Keylock' })).toHaveAttribute('aria-pressed', 'true');
});
