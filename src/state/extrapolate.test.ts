import { describe, expect, it } from 'vitest';
import type { DeckFrame } from '../ipc/types';
import {
  MAX_EXTRAPOLATION_MS,
  NEXT_RANGE,
  faderToTempo,
  formatRange,
  formatRemaining,
  formatTempo,
  formatTime,
  positionAt,
  tempoToFader,
} from './extrapolate';

const deck = (over: Partial<DeckFrame> = {}): DeckFrame => ({
  trackId: 1,
  position: 48_000,
  frames: 48_000 * 60,
  playing: true,
  cue: 0,
  previewing: false,
  peak: [0, 0],
  tempo: 0,
  tempoRange: 'ten',
  keylock: false,
  rate: 1,
  reverse: false,
  scratching: false,
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
    expect(positionAt(deck({ rate: 1.06 }), 48_000, 1000, 1100)).toBeCloseTo(48_000 + 5_088);
  });

  it('runs backwards in reverse and stops at the start', () => {
    expect(positionAt(deck({ rate: -1 }), 48_000, 1000, 1100)).toBeCloseTo(48_000 - 4_800);
    expect(positionAt(deck({ rate: -1, position: 100 }), 48_000, 1000, 1100)).toBe(0);
  });

  it('holds still while scratching with the platter held', () => {
    expect(positionAt(deck({ rate: 0, scratching: true }), 48_000, 1000, 1100)).toBe(48_000);
  });

  it('survives a broken rate', () => {
    expect(positionAt(deck({ rate: Number.NaN }), 48_000, 1000, 1100)).toBe(48_000);
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

describe('formatTempo', () => {
  it('shows the change in percent of the current range', () => {
    expect(formatTempo(0, 'ten')).toBe('0.00 %');
    expect(formatTempo(1, 'ten')).toBe('+10.00 %');
    expect(formatTempo(-1, 'sixteen')).toBe('−16.00 %');
    expect(formatTempo(0.25, 'six')).toBe('+1.50 %');
    expect(formatTempo(-0.5, 'wide')).toBe('−25.00 %');
  });

  it('never shows a negative zero and clamps bad input', () => {
    expect(formatTempo(-0.00001, 'ten')).toBe('0.00 %');
    expect(formatTempo(Number.NaN, 'ten')).toBe('0.00 %');
    expect(formatTempo(7, 'six')).toBe('+6.00 %');
  });
});

describe('tempo fader', () => {
  it('puts slower at the top, as on the controller', () => {
    expect(faderToTempo(1)).toBe(-1);
    expect(faderToTempo(0)).toBe(1);
    expect(faderToTempo(0.5)).toBe(0);
    expect(tempoToFader(-1)).toBe(1);
  });

  it('round-trips and clamps', () => {
    for (const t of [-1, -0.37, 0, 0.5, 1]) expect(faderToTempo(tempoToFader(t))).toBeCloseTo(t);
    expect(tempoToFader(Number.NaN)).toBe(0.5);
    expect(faderToTempo(3)).toBe(-1);
  });
});

describe('tempo ranges', () => {
  it('cycle like Shift + Sync on the controller', () => {
    expect([NEXT_RANGE.six, NEXT_RANGE.ten, NEXT_RANGE.sixteen, NEXT_RANGE.wide]).toEqual([
      'ten',
      'sixteen',
      'wide',
      'six',
    ]);
    expect(formatRange('sixteen')).toBe('±16 %');
    expect(formatRange('wide')).toBe('WIDE');
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
