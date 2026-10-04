#!/usr/bin/env node
/* global document -- used inside page.evaluate, which runs in the browser */
// Renders the README screenshots and the social banner from the in-browser demo (mock backend,
// `?demo`), framed as a macOS window. Reproducible: no audio device, no controller needed.
//
//   pnpm screenshots        → docs/screenshots/*.png, docs/banner.png

import { spawn } from 'node:child_process';
import { mkdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const PORT = 1423;
const BASE = `http://localhost:${PORT}`;
const FRAME = { width: 1440 + 128, height: 900 + 120 };

async function startVite() {
  const vite = spawn('pnpm', ['exec', 'vite', '--port', String(PORT), '--strictPort'], {
    cwd: ROOT,
    stdio: ['ignore', 'pipe', 'inherit'],
  });
  await new Promise((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error('vite did not start')), 30_000);
    vite.stdout.on('data', (d) => {
      if (String(d).includes('Local:')) {
        clearTimeout(timer);
        resolve();
      }
    });
    vite.on('exit', (code) => reject(new Error(`vite exited with ${code}`)));
  });
  return vite;
}

const scenes = [
  {
    file: 'decks.png',
    src: '/?demo',
    act: async (app) => {
      await app.getByRole('heading', { name: 'Deep Hours (Extended Mix)' }).waitFor();
    },
  },
  { file: 'empty.png', src: '/', act: async (app) => app.getByText('Noch keine Tracks').waitFor() },
  {
    file: 'settings.png',
    src: '/?demo',
    act: async (app) => {
      await app.getByRole('heading', { name: 'Neon Avenue (Club Edit)' }).waitFor();
      await app.getByRole('button', { name: 'Einstellungen' }).click();
      await app.getByRole('dialog', { name: 'Einstellungen' }).waitFor();
    },
  },
  {
    file: 'midi-monitor.png',
    src: '/?demo',
    act: async (app) => {
      await app.getByRole('heading', { name: 'Neon Avenue (Club Edit)' }).waitFor();
      await app.getByRole('button', { name: 'Einstellungen' }).click();
      await app.getByRole('button', { name: /MIDI-Monitor öffnen/ }).click();
      await app.getByText('Deck 1 · Jog-Rand +2').first().waitFor();
    },
  },
];

async function main() {
  mkdirSync(`${ROOT}/docs/screenshots`, { recursive: true });
  const vite = await startVite();
  const browser = await chromium.launch({ channel: 'chrome' });
  try {
    for (const scene of scenes) {
      const page = await browser.newPage({ viewport: FRAME, deviceScaleFactor: 2 });
      await page.goto(`${BASE}/tools/frame.html?src=${encodeURIComponent(scene.src)}`);
      const app = page.frameLocator('#app');
      await scene.act(app);
      await page.waitForTimeout(900); // meters and springs settle into a lively frame
      await page.locator('.stage').screenshot({
        path: `${ROOT}/docs/screenshots/${scene.file}`,
        omitBackground: true,
      });
      await page.close();
      console.log(`docs/screenshots/${scene.file}`);
    }
    const page = await browser.newPage({ viewport: { width: 1280, height: 640 } });
    await page.goto(`${BASE}/tools/banner.html`);
    await page.evaluate(() => document.fonts.ready);
    await page.waitForTimeout(300);
    await page.locator('.banner').screenshot({ path: `${ROOT}/docs/banner.png` });
    console.log('docs/banner.png');
  } finally {
    await browser.close();
    vite.kill();
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
