import { describe, expect, it } from 'vitest';
import type { DeckFrame } from '../ipc/types';
import { MAX_EXTRAPOLATION_MS, formatRemaining, formatTime, positionAt } from './extrapolate';

const deck = (over: Partial<DeckFrame> = {}): DeckFrame => ({
  trackId: 1,
  position: 48_000,
  frames: 48_000 * 60,
  playing: true,
  cue: 0,
  previewing: false,
  peak: [0, 0],
  ...over,
});

describe('positionAt', () => {
  it('stays put while paused', () => {
    expect(positionAt(deck({ playing: false }), 48_000, 1000, 1500)).toBe(48_000);
  });

  it('advances linearly while playing', () => {
    expect(positionAt(deck(), 48_000, 1000, 1100)).toBeCloseTo(48_000 + 4_800);
  });

  it('honours the playback rate', () => {
    expect(positionAt(deck(), 48_000, 1000, 1100, 2)).toBeCloseTo(48_000 + 9_600);
  });

  it('never runs past the end', () => {
    const d = deck({ position: 48_000 * 60 - 10 });
    expect(positionAt(d, 48_000, 0, 200)).toBe(48_000 * 60);
  });

  it('stops extrapolating when frames stop arriving', () => {
    const far = positionAt(deck(), 48_000, 0, 10_000);
    expect(far).toBeCloseTo(48_000 + (MAX_EXTRAPOLATION_MS / 1000) * 48_000);
  });

  it('ignores clock skew backwards', () => {
    expect(positionAt(deck(), 48_000, 1000, 900)).toBe(48_000);
  });
});

describe('formatTime', () => {
  it('formats minutes, seconds and tenths', () => {
    expect(formatTime(0, 48_000)).toBe('0:00.0');
    expect(formatTime(48_000 * 75.35, 48_000)).toBe('1:15.3');
    expect(formatTime(48_000 * 600, 48_000)).toBe('10:00.0');
  });

  it('survives bad input', () => {
    expect(formatTime(Number.NaN, 48_000)).toBe('0:00.0');
    expect(formatTime(100, 0)).toBe('0:00.0');
  });
});

describe('formatRemaining', () => {
  it('counts down and reaches zero exactly at the end', () => {
    expect(formatRemaining(0, 48_000 * 90, 48_000)).toBe('−1:30.0');
    expect(formatRemaining(48_000 * 90, 48_000 * 90, 48_000)).toBe('−0:00.0');
  });

  it('rounds up so the last tenth is still shown', () => {
    expect(formatRemaining(48_000 * 90 - 10, 48_000 * 90, 48_000)).toBe('−0:00.1');
  });
});
