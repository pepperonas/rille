import { describe, expect, it } from 'vitest';
import { METER_FLOOR_DB, meterFill, toDb } from './meterScale';

describe('meter scale', () => {
  it('is empty for silence and below the floor', () => {
    expect(meterFill(0)).toBe(0);
    expect(meterFill(10 ** (METER_FLOOR_DB / 20) / 2)).toBe(0);
  });

  it('is full at 0 dBFS and clamps above', () => {
    expect(meterFill(1)).toBe(1);
    expect(meterFill(2)).toBe(1);
  });

  it('is linear in dB', () => {
    expect(meterFill(10 ** (-24 / 20))).toBeCloseTo(0.5);
  });

  it('converts to dB', () => {
    expect(toDb(0.5)).toBeCloseTo(-6.02, 2);
    expect(toDb(0)).toBe(-Infinity);
  });
});
