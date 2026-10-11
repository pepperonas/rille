import { describe, expect, it } from 'vitest';
import { WHEEL_STEP, wheelSteps } from './zoom';

function run(deltas: number[], mode = 0): number[] {
  let acc = 0;
  return deltas.map((d) => {
    const { steps, rest } = wheelSteps(acc, d, mode);
    acc = rest;
    return steps;
  });
}

describe('wheelSteps', () => {
  it('turns one mouse notch into one step', () => {
    expect(run([100])).toEqual([1]);
    expect(run([-100])).toEqual([-1]);
  });

  it('lets a trackpad swipe add up instead of racing through all zoom levels', () => {
    // 40 events of 4 px (with inertia) = 160 px: two steps, not forty.
    const steps = run(Array.from({ length: 40 }, () => 4));
    expect(steps.reduce((a, b) => a + b, 0)).toBe(2);
  });

  it('starts afresh when the direction turns', () => {
    expect(run([60, -30, -60])).toEqual([0, 0, -1]);
  });

  it('understands line and page deltas and ignores nonsense', () => {
    expect(run([3], 1)).toEqual([0]);
    expect(run([6], 1)).toEqual([1]);
    expect(run([1], 2)).toEqual([Math.trunc(400 / WHEEL_STEP)]);
    expect(wheelSteps(10, NaN)).toEqual({ steps: 0, rest: 10 });
  });
});
