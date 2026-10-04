import { describe, expect, it } from 'vitest';
import { dampingRatio, springs } from './tokens';

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
