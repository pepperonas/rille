// @vitest-environment node
import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { dampingRatio, shapes, sizes, springs } from './tokens';

describe('spring tokens', () => {
  it('lets spatial springs overshoot a little, but not wobble', () => {
    for (const s of [springs.spatialFast, springs.spatialDefault, springs.spatialSlow]) {
      const r = dampingRatio(s);
      expect(r).toBeGreaterThan(0.6);
      expect(r).toBeLessThan(1);
    }
  });

  it('never lets effects springs overshoot (colour/opacity must not bounce)', () => {
    for (const s of [springs.effectsFast, springs.effectsDefault, springs.effectsSlow]) {
      expect(dampingRatio(s)).toBeGreaterThanOrEqual(0.99);
    }
  });
});

describe('numeric mirrors of tokens.css', () => {
  const css = readFileSync(join(process.cwd(), 'src/design/tokens.css'), 'utf8');
  const px = (name: string) => {
    const m = css.match(new RegExp(`--${name}:\\s*(\\d+)px`));
    return m ? Number(m[1]) : NaN;
  };

  it('keeps sizes in sync', () => {
    expect(sizes.playButton).toBe(px('size-play-button'));
    expect(sizes.transportButton).toBe(px('size-transport-button'));
  });

  it('keeps shapes in sync', () => {
    expect(shapes.md).toBe(px('shape-md'));
    expect(shapes.lg).toBe(px('shape-lg'));
    expect(shapes.lgInc).toBe(px('shape-lg-inc'));
    expect(shapes.xl).toBe(px('shape-xl'));
  });
});
