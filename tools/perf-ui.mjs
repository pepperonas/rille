#!/usr/bin/env node
/* global window, requestAnimationFrame -- run in the page */
// Measures the interface with two decks playing (in-browser demo, mock backend):
// frame intervals, main-thread work per second, long tasks; then the same with both decks
// paused (nothing should draw).
//
//   node tools/perf-ui.mjs

import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { chromium } from '@playwright/test';

const ROOT = fileURLToPath(new URL('..', import.meta.url));
const PORT = 1424;
const SECONDS = 5;

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
  });
  return vite;
}

async function measure(page, cdp) {
  const metric = async () => {
    const { metrics } = await cdp.send('Performance.getMetrics');
    return Object.fromEntries(metrics.map((m) => [m.name, m.value]));
  };
  const before = await metric();
  const draws0 = await page.evaluate(() => window.__rilleWaveDraws ?? 0);
  const frames = await page.evaluate(
    (seconds) =>
      new Promise((resolve) => {
        const intervals = [];
        const longTasks = [];
        const obs = new PerformanceObserver((l) =>
          l.getEntries().forEach((e) => longTasks.push(e.duration)),
        );
        obs.observe({ type: 'longtask' });
        let last = performance.now();
        const end = last + seconds * 1000;
        const tick = (t) => {
          intervals.push(t - last);
          last = t;
          if (t < end) requestAnimationFrame(tick);
          else {
            obs.disconnect();
            resolve({ intervals, longTasks });
          }
        };
        requestAnimationFrame(tick);
      }),
    SECONDS,
  );
  const after = await metric();
  const draws = (await page.evaluate(() => window.__rilleWaveDraws ?? 0)) - draws0;
  const sorted = [...frames.intervals].sort((a, b) => a - b);
  const q = (p) => sorted[Math.floor((sorted.length - 1) * p)] ?? 0;
  const busy = (after.TaskDuration - before.TaskDuration) / SECONDS;
  return {
    fps: (frames.intervals.length / SECONDS).toFixed(1),
    medianMs: q(0.5).toFixed(1),
    p99Ms: q(0.99).toFixed(1),
    worstMs: (sorted.at(-1) ?? 0).toFixed(1),
    mainThreadPct: (busy * 100).toFixed(1),
    longTasks: frames.longTasks.length,
    waveDrawsPerSec: (draws / SECONDS).toFixed(1),
  };
}

async function main() {
  const vite = await startVite();
  const browser = await chromium.launch({ channel: 'chrome' });
  try {
    const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
    const cdp = await page.context().newCDPSession(page);
    await cdp.send('Performance.enable');
    await page.goto(`http://localhost:${PORT}/?demo`);
    await page.getByRole('heading', { name: 'Neon Avenue (Club Edit)' }).waitFor();
    await page.waitForTimeout(1500);
    console.log('two decks playing:', await measure(page, cdp));
    for (const d of ['a', 'b']) {
      await page.locator(`section[data-deck='${d}']`).getByRole('button', { name: 'Pause' }).click();
    }
    await page.waitForTimeout(1000);
    console.log('both paused:     ', await measure(page, cdp));
  } finally {
    await browser.close();
    vite.kill();
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
