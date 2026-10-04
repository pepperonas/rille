import { describe, expect, it } from 'vitest';
import { arcPath, dragValue, valueToAngle } from './knobGeometry';

describe('knob geometry', () => {
  it('maps the value range onto 270 degrees with the centre at 12 o\'clock', () => {
    expect(valueToAngle(0)).toBe(-135);
    expect(valueToAngle(0.5)).toBe(0);
    expect(valueToAngle(1)).toBe(135);
    expect(valueToAngle(2)).toBe(135);
    expect(valueToAngle(Number.NaN)).toBe(0);
  });

  it('draws arcs in either direction and nothing for a zero-length arc', () => {
    expect(arcPath(20, 20, 16, 0, 0)).toBe('');
    expect(arcPath(20, 20, 16, 0, 90)).toBe(arcPath(20, 20, 16, 90, 0));
    expect(arcPath(20, 20, 16, -135, 135)).toContain(' 0 1 1 '); // large arc flag
  });

  it('drags up to increase, Shift for fine control, clamped', () => {
    expect(dragValue(0.5, -100, false)).toBeCloseTo(1);
    expect(dragValue(0.5, 100, false)).toBeCloseTo(0);
    expect(dragValue(0.5, -100, true)).toBeCloseTo(0.55);
    expect(dragValue(0.9, -1000, false)).toBe(1);
  });
});
