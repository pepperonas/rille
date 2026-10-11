import { describe, expect, it } from 'vitest';
import {
  beatsIn,
  columnMax,
  seekTarget,
  binCount,
  binRate,
  chooseLevel,
  clampZoom,
  encodePeaks,
  parsePeaks,
  peaksDuration,
  PeaksError,
  syntheticPeaks,
  ZOOM_SECONDS,
} from './peaks';

describe('parsePeaks', () => {
  const p = syntheticPeaks(20, 128, 3);

  it('round-trips through the binary format', () => {
    const back = parsePeaks(encodePeaks(p));
    expect(back.sampleRate).toBe(p.sampleRate);
    expect(back.framesPerBin).toBe(p.framesPerBin);
    expect(back.levels.map((l) => Array.from(l))).toEqual(p.levels.map((l) => Array.from(l)));
  });

  it('matches the Rust layout byte for byte', () => {
    // RLPK, v1, 3 bands, 1 level, 48 kHz, 256 frames/bin, 2 bins.
    const bytes = new Uint8Array([
      82, 76, 80, 75, 1, 0, 3, 1, 0x80, 0xbb, 0, 0, 0, 1, 0, 0, 2, 0, 0, 0, 10, 20, 30, 40, 50, 60,
    ]);
    const parsed = parsePeaks(bytes.buffer);
    expect(parsed.sampleRate).toBe(48_000);
    expect(parsed.framesPerBin).toBe(256);
    expect(Array.from(parsed.levels[0] ?? [])).toEqual([10, 20, 30, 40, 50, 60]);
  });

  it('rejects other files, other versions and truncation', () => {
    const bytes = new Uint8Array(encodePeaks(p));
    expect(() => parsePeaks(bytes.slice(0, bytes.length - 1).buffer)).toThrow(PeaksError);
    const longer = new Uint8Array(bytes.length + 1);
    longer.set(bytes);
    expect(() => parsePeaks(longer.buffer)).toThrow(PeaksError);
    expect(() => parsePeaks(new TextEncoder().encode('nope, not peaks!').buffer)).toThrow(
      'keine Peaks-Datei',
    );
    const future = bytes.slice();
    future[4] = 9;
    expect(() => parsePeaks(future.buffer)).toThrow('Version 9');
    // A bin count far beyond the file must not allocate or read out of bounds.
    const huge = bytes.slice(0, 20);
    new DataView(huge.buffer).setUint32(16, 0xffffffff, true);
    expect(() => parsePeaks(huge.buffer)).toThrow(PeaksError);
  });
});

describe('time and zoom', () => {
  const p = syntheticPeaks(60, 120, 1);

  it('relates bins to seconds at every level', () => {
    expect(binRate(p, 0)).toBeCloseTo(44_100 / 235);
    expect(binRate(p, 1)).toBeCloseTo(binRate(p, 0) / 4);
    expect(peaksDuration(p)).toBeCloseTo(60, 1);
    expect(binCount(p, 3)).toBe(Math.ceil(binCount(p, 2) / 4));
  });

  it('picks the finest level that keeps a pixel to a few bins', () => {
    // 8 s over 800 px: 0.01 s/px → ~1.9 bins/px at level 0.
    expect(chooseLevel(p, 8 / 800)).toBe(0);
    // A whole minute over 300 px: 0.2 s/px → level 2 (~2.3 bins/px).
    expect(chooseLevel(p, 60 / 300)).toBe(2);
    // Never past the coarsest level.
    expect(chooseLevel(p, 1000)).toBe(3);
    for (const spp of [0.001, 0.01, 0.1, 1]) {
      const level = chooseLevel(p, spp);
      if (level < 3) expect(spp * binRate(p, level)).toBeLessThanOrEqual(4);
    }
  });

  it('clamps zoom steps', () => {
    expect(clampZoom(-3)).toBe(0);
    expect(clampZoom(99)).toBe(ZOOM_SECONDS.length - 1);
    expect(clampZoom(1.4)).toBe(1);
  });
});

describe('beatsIn', () => {
  it('lists the beats of a window with every fourth as downbeat', () => {
    const beats = beatsIn(120, 0.25, 0, 2.3);
    expect(beats.map((b) => b.time)).toEqual([0.25, 0.75, 1.25, 1.75, 2.25]);
    expect(beats.map((b) => b.down)).toEqual([true, false, false, false, true]);
  });

  it('counts downbeats from the first beat, also before it', () => {
    const beats = beatsIn(60, 10, 7.5, 10.5);
    expect(beats.map((b) => [b.time, b.down])).toEqual([
      [8, false],
      [9, false],
      [10, true],
    ]);
  });

  it('refuses nonsense grids and caps the count', () => {
    expect(beatsIn(0, 0, 0, 10)).toEqual([]);
    expect(beatsIn(120, NaN, 0, 10)).toEqual([]);
    expect(beatsIn(120, 0, 5, 5)).toEqual([]);
    expect(beatsIn(10_000, 0, 0, 3600, 100)).toHaveLength(100);
  });
});

describe('columnMax', () => {
  const p = {
    sampleRate: 100,
    framesPerBin: 10,
    levels: [new Uint8Array([1, 2, 3, 9, 0, 0, 0, 8, 0, 4, 4, 4])],
  }; // 10 bins/s, 4 bins

  it('takes the per-band maximum of the covered bins', () => {
    const out = [0, 0, 0];
    columnMax(p, 0, 0, 0.2, out);
    expect(out).toEqual([9, 2, 3]);
    columnMax(p, 0, 0.1, 0.4, out);
    expect(out).toEqual([9, 8, 4]);
  });

  it('always covers at least one bin and is silent outside the track', () => {
    const out = [7, 7, 7];
    columnMax(p, 0, 0.21, 0.22, out);
    expect(out).toEqual([0, 8, 0]);
    columnMax(p, 0, -1, -0.5, out);
    expect(out).toEqual([0, 0, 0]);
    columnMax(p, 0, -0.05, 0.05, out);
    expect(out).toEqual([1, 2, 3]);
    columnMax(p, 0, 5, 6, out);
    expect(out).toEqual([0, 0, 0]);
  });
});

describe('seekTarget', () => {
  it('maps a fraction to a frame, short of the very end', () => {
    expect(seekTarget(0.25, 1000)).toBe(250);
    expect(seekTarget(1, 1000)).toBe(999);
    expect(seekTarget(7, 1000)).toBe(999);
    expect(seekTarget(-1, 1000)).toBe(0);
    expect(seekTarget(NaN, 1000)).toBe(0);
    expect(seekTarget(0.5, 0)).toBe(0);
  });
});
