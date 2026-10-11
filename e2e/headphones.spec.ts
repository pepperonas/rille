import { expect, test } from '@playwright/test';

let errors: string[] = [];

test.beforeEach(async ({ page }) => {
  errors = [];
  page.on('pageerror', (e) => errors.push(e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') errors.push(m.text());
  });
  await page.goto('/?demo');
});

test.afterEach(() => {
  expect(errors).toEqual([]);
});

test('mixer: headphone buttons per channel and master, blend and level', async ({ page }) => {
  const mixer = page.getByRole('region', { name: 'Mixer' });
  for (const name of ['Kopfhörer Kanal 1', 'Kopfhörer Kanal 2', 'Kopfhörer Master']) {
    const button = mixer.getByRole('button', { name });
    await expect(button).toHaveAttribute('aria-pressed', 'false');
    await button.click();
    await expect(button).toHaveAttribute('aria-pressed', 'true');
  }
  await mixer.getByRole('button', { name: 'Kopfhörer Kanal 2' }).click();
  await expect(mixer.getByRole('button', { name: 'Kopfhörer Kanal 2' })).toHaveAttribute(
    'aria-pressed',
    'false',
  );
  const mix = mixer.getByRole('slider', { name: 'Kopfhörer Cue/Master' });
  await mix.focus();
  await page.keyboard.press('Home');
  await expect(mix).toHaveAttribute('aria-valuetext', 'Nur Cue');
  await page.keyboard.press('End');
  await expect(mix).toHaveAttribute('aria-valuetext', 'Nur Master');
  const level = mixer.getByRole('slider', { name: 'Kopfhörer-Lautstärke' });
  await level.focus();
  await page.keyboard.press('Home');
  await expect(level).toHaveAttribute('aria-valuetext', '0 %');
});

test('settings: headphones on a second device, outputs 3/4 explain what is missing', async ({
  page,
}) => {
  await page.getByRole('button', { name: 'Einstellungen' }).click();
  const section = page.getByRole('dialog', { name: 'Einstellungen' }).getByRole('region', {
    name: 'Kopfhörer',
  });
  const output = section.getByRole('combobox', { name: 'Kopfhörer-Ausgang' });
  await expect(output).toHaveValue('off');
  // The main output (mock) has two outputs only.
  await output.selectOption('channels34');
  await expect(section.getByRole('alert')).toContainText('keine Ausgänge 3/4');
  // A second device: shown with its latency, no dropouts.
  await output.selectOption({ label: 'USB-Kopfhörer' });
  await expect(section).toContainText('USB-Kopfhörer');
  await expect(section.getByRole('alert')).toHaveCount(0);
  await expect(section).toContainText('Aussetzer');
  await section.getByRole('combobox', { name: 'Puffer Kopfhörer' }).selectOption('512');
  await expect(section).toContainText('32.0 ms');
  await output.selectOption('off');
  await expect(section).not.toContainText('Latenz');
});
