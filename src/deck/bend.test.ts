import { describe, expect, it } from 'vitest';
import { type BendDirection, bendOf, holdBend } from './bend';

describe('bend buttons', () => {
  it('bend while held and stop on release', () => {
    let held: BendDirection[] = [];
    held = holdBend(held, 1, true);
    expect(bendOf(held)).toBe(1);
    held = holdBend(held, 1, false);
    expect(bendOf(held)).toBe(0);
  });

  it('letting go of one keeps the other held one', () => {
    let held: BendDirection[] = [];
    held = holdBend(held, -1, true);
    held = holdBend(held, 1, true);
    expect(bendOf(held)).toBe(1); // the newest press wins
    held = holdBend(held, 1, false);
    expect(bendOf(held)).toBe(-1); // − is still down
    held = holdBend(held, -1, false);
    expect(bendOf(held)).toBe(0);
  });

  it('a repeated press does not stack', () => {
    let held = holdBend([], 1, true);
    held = holdBend(held, 1, true);
    held = holdBend(held, 1, false);
    expect(bendOf(held)).toBe(0);
  });
});
