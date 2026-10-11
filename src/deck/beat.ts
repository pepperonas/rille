// Beat arithmetic on the frontend: pad layouts, tap tempo, grid corrections, and the same
// sync rules as the engine (`crates/rille-engine/src/beat.rs`) for the mock backend.
import type { Grid, PadMode, TempoRange } from '../ipc/types';
import { RANGE_SPAN } from '../state/extrapolate';

/** Beat loop length per pad (pads 1–8); mirrors `LOOP_BEATS` in controller_service.rs. */
export const LOOP_BEATS = [0.25, 0.5, 1, 2, 4, 8, 16, 32] as const;
/** Beat jump per pad: top row back, bottom row forward; mirrors `JUMP_BEATS`. */
export const JUMP_BEATS = [-0.5, -1, -4, -16, 0.5, 1, 4, 16] as const;

export const PAD_MODES: Array<{ mode: PadMode; label: string }> = [
  { mode: 'hotCue', label: 'Hot Cue' },
  { mode: 'beatLoop', label: 'Loop' },
  { mode: 'beatJump', label: 'Jump' },
];

/** `¼`, `½`, `1` … for loop and jump labels. */
export function beatsLabel(beats: number): string {
  const sign = beats < 0 ? '−' : '';
  const a = Math.abs(beats);
  const text = a === 0.25 ? '¼' : a === 0.5 ? '½' : String(a);
  return `${sign}${text}`;
}

export function beatAt(grid: Grid, frame: number, sampleRate: number): number {
  return ((frame / sampleRate - grid.firstBeat) * grid.bpm) / 60;
}

export function frameOfBeat(grid: Grid, beat: number, sampleRate: number): number {
  return (grid.firstBeat + (beat * 60) / grid.bpm) * sampleRate;
}

export function beatFrames(grid: Grid, sampleRate: number): number {
  return (60 / grid.bpm) * sampleRate;
}

/** `x` folded into [-0.5, 0.5). */
export function wrapHalf(x: number): number {
  return x - Math.floor(x + 0.5);
}

/** Rate (and own beats per target beat) to follow `targetBpm`, choosing ×½/×1/×2. */
export function follow(
  ownBpm: number,
  targetBpm: number,
): { rate: number; beatsPerTargetBeat: number } | null {
  if (!(ownBpm > 0 && targetBpm > 0)) return null;
  return [0.5, 1, 2]
    .map((k) => ({ rate: (targetBpm * k) / ownBpm, beatsPerTargetBeat: k }))
    .reduce((a, b) => (Math.abs(Math.log(b.rate)) < Math.abs(Math.log(a.rate)) ? b : a));
}

const RANGES: TempoRange[] = ['six', 'ten', 'sixteen', 'wide'];

/** Range and fader position for `rate`: the current range if it reaches, else the narrowest. */
export function tempoForRate(
  rate: number,
  current: TempoRange,
): { range: TempoRange; tempo: number } | null {
  for (const range of [current, ...RANGES]) {
    const tempo = (rate - 1) / RANGE_SPAN[range];
    if (Number.isFinite(tempo) && Math.abs(tempo) <= 1) return { range, tempo };
  }
  return null;
}

/** Hot cue position: on the nearest beat if within ⅛ beat, else exactly here. */
export function quantizeCue(grid: Grid | null, frame: number, sampleRate: number): number {
  if (!grid) return frame;
  const beat = beatAt(grid, frame, sampleRate);
  return Math.abs(beat - Math.round(beat)) <= 0.125
    ? Math.max(0, frameOfBeat(grid, Math.round(beat), sampleRate))
    : frame;
}

/** Tempo from tapped times (ms): median interval, after dropping taps more than 30 % off it
 *  (a missed or double tap). Needs four taps; a pause over two seconds starts afresh. */
export function tapTempo(times: number[]): number | null {
  let start = 0;
  for (let i = 1; i < times.length; i++) {
    if ((times[i] ?? 0) - (times[i - 1] ?? 0) > 2000) start = i;
  }
  const taps = times.slice(start);
  if (taps.length < 4) return null;
  const gaps = taps.slice(1).map((t, i) => t - (taps[i] ?? 0));
  const sorted = [...gaps].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
  const good = gaps.filter((g) => Math.abs(g - median) <= 0.3 * median);
  if (good.length < 3 || median <= 0) return null;
  const mean = good.reduce((a, b) => a + b, 0) / good.length;
  return Math.round((60_000 / mean) * 100) / 100;
}

/** Grid corrections from the deck's grid editor. The first beat stays in [0, period). */
export function shiftGrid(grid: Grid, seconds: number): Grid {
  const period = 60 / grid.bpm;
  return { ...grid, firstBeat: (((grid.firstBeat + seconds) % period) + period) % period };
}

export function scaleGrid(grid: Grid, factor: number): Grid {
  const bpm = Math.min(400, Math.max(20, grid.bpm * factor));
  return shiftGrid({ ...grid, bpm }, 0);
}

/** A grid with the tapped tempo whose beat lies on `frame` (the tap was on the beat). */
export function gridFromTap(bpm: number, frame: number, sampleRate: number): Grid {
  return shiftGrid({ bpm, firstBeat: 0 }, frame / sampleRate);
}
