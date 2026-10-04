// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { SEEDS, buildTheme, toCss } from './gen-tokens.mjs';

function luminance(hex) {
  const c = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = c.map((v) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4));
  return 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
}
function contrast(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

describe.each([
  ['dark', true],
  ['light', false],
])('%s theme', (_name, isDark) => {
  const t = buildTheme(SEEDS, isDark);

  it('has every token as a hex colour', () => {
    for (const [k, v] of Object.entries(t)) expect(v, k).toMatch(/^#[0-9a-f]{6}$/);
  });

  it('keeps body text readable on every surface container (>= 4.5:1)', () => {
    for (const s of ['surface', 'surface-container', 'surface-container-high', 'surface-container-highest']) {
      expect(contrast(t['md-on-surface'], t[`md-${s}`]), s).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('keeps text on deck accents readable (>= 4.5:1)', () => {
    for (const d of ['a', 'b']) {
      expect(contrast(t[`deck-${d}-on`], t[`deck-${d}`])).toBeGreaterThanOrEqual(4.5);
      expect(contrast(t[`deck-${d}-on-container`], t[`deck-${d}-container`])).toBeGreaterThanOrEqual(4.5);
    }
  });

  it('makes deck accents visible as UI elements on the surface (>= 3:1)', () => {
    for (const d of ['a', 'b']) {
      expect(contrast(t[`deck-${d}`], t['md-surface'])).toBeGreaterThanOrEqual(3);
    }
  });

  it('keeps the two decks apart', () => {
    expect(t['deck-a']).not.toBe(t['deck-b']);
  });
});

describe('generated file', () => {
  it('is in sync with the seeds (run `pnpm tokens` after changing them)', () => {
    const file = readFileSync(new URL('../src/design/color.generated.css', import.meta.url), 'utf8');
    expect(file).toBe(toCss(SEEDS));
  });
});
