// Waveform peaks as written by `crates/rille-library/src/analysis/peaks.rs` (format "RLPK" v1),
// and the time/zoom arithmetic the waveforms draw with.

export const BANDS = 3;
/** Zoom factor between levels. */
export const LEVEL_FACTOR = 4;

export interface Peaks {
  sampleRate: number;
  /** Source frames per bin at level 0. */
  framesPerBin: number;
  /** `levels[k]` holds `bins × [low, mid, high]` (0–255), interleaved. */
  levels: Uint8Array[];
}

export class PeaksError extends Error {}

/** Parse "RLPK" v1. Throws `PeaksError` for anything else or a truncated file. */
export function parsePeaks(buffer: ArrayBuffer): Peaks {
  const view = new DataView(buffer);
  const need = (at: number, n: number) => {
    if (at + n > buffer.byteLength) throw new PeaksError('Peaks-Datei ist unvollständig');
  };
  need(0, 16);
  const magic = String.fromCharCode(...new Uint8Array(buffer, 0, 4));
  if (magic !== 'RLPK') throw new PeaksError('keine Peaks-Datei');
  const version = view.getUint16(4, true);
  if (version !== 1) throw new PeaksError(`Peaks-Version ${version} wird nicht unterstützt`);
  if (view.getUint8(6) !== BANDS) throw new PeaksError('keine Peaks-Datei');
  const count = view.getUint8(7);
  const sampleRate = view.getUint32(8, true);
  const framesPerBin = view.getUint32(12, true);
  let at = 16;
  const levels: Uint8Array[] = [];
  for (let k = 0; k < count; k++) {
    need(at, 4);
    const bins = view.getUint32(at, true);
    at += 4;
    need(at, bins * BANDS);
    levels.push(new Uint8Array(buffer, at, bins * BANDS));
    at += bins * BANDS;
  }
  if (at !== buffer.byteLength) throw new PeaksError('Peaks-Datei ist unvollständig');
  if (sampleRate <= 0 || framesPerBin <= 0) throw new PeaksError('keine Peaks-Datei');
  return { sampleRate, framesPerBin, levels };
}

/** The inverse of `parsePeaks` (mock backend, tests). */
export function encodePeaks(p: Peaks): ArrayBuffer {
  const size = 16 + p.levels.reduce((n, l) => n + 4 + l.length, 0);
  const out = new Uint8Array(size);
  const view = new DataView(out.buffer);
  out.set([82, 76, 80, 75], 0); // "RLPK"
  view.setUint16(4, 1, true);
  view.setUint8(6, BANDS);
  view.setUint8(7, p.levels.length);
  view.setUint32(8, p.sampleRate, true);
  view.setUint32(12, p.framesPerBin, true);
  let at = 16;
  for (const l of p.levels) {
    view.setUint32(at, l.length / BANDS, true);
    out.set(l, at + 4);
    at += 4 + l.length;
  }
  return out.buffer;
}

/** Bins per second at level `k`. */
export function binRate(p: Peaks, level = 0): number {
  return p.sampleRate / (p.framesPerBin * LEVEL_FACTOR ** level);
}

export function binCount(p: Peaks, level: number): number {
  return (p.levels[level]?.length ?? 0) / BANDS;
}

/** Track length the peaks cover, in seconds. */
export function peaksDuration(p: Peaks): number {
  return binCount(p, 0) / binRate(p, 0);
}

/** Finest level whose bins are at least `minBinsPerPixel`-th of a pixel wide, so a pixel never
 *  has to combine more than `LEVEL_FACTOR × …` bins. Clamped to the levels present. */
export function chooseLevel(p: Peaks, secondsPerPixel: number, maxBinsPerPixel = 4): number {
  let level = 0;
  while (level < p.levels.length - 1 && secondsPerPixel * binRate(p, level) > maxBinsPerPixel) {
    level++;
  }
  return level;
}

/** Visible span of the detail view, in seconds, per zoom step. */
export const ZOOM_SECONDS = [2, 4, 8, 16, 32] as const;
export const DEFAULT_ZOOM = 2;

export function clampZoom(z: number): number {
  return Math.max(0, Math.min(ZOOM_SECONDS.length - 1, Math.round(z)));
}

/** Beat times (seconds) within [from, to] for a grid; every fourth from `firstBeat` is a
 *  downbeat. Capped so a broken grid cannot produce millions of lines. */
export function beatsIn(
  bpm: number,
  firstBeat: number,
  from: number,
  to: number,
  max = 512,
): Array<{ time: number; down: boolean }> {
  if (!(bpm > 0) || !Number.isFinite(firstBeat) || !(to > from)) return [];
  const period = 60 / bpm;
  const first = Math.ceil((from - firstBeat) / period);
  const out: Array<{ time: number; down: boolean }> = [];
  for (let n = first; out.length < max; n++) {
    const time = firstBeat + n * period;
    if (time > to) break;
    out.push({ time, down: ((n % 4) + 4) % 4 === 0 });
  }
  return out;
}

/** Deterministic synthetic peaks for the mock backend: a kick pulse at `bpm`, a breakdown in
 *  the middle, hats on the off-beats. */
export function syntheticPeaks(seconds: number, bpm: number, seed: number, firstBeat = 0): Peaks {
  const sampleRate = 44_100;
  const framesPerBin = 235;
  const rate = sampleRate / framesPerBin;
  const bins = Math.floor(seconds * rate);
  const level0 = new Uint8Array(bins * BANDS);
  const period = 60 / bpm;
  let noise = seed * 9301 + 49297;
  const rand = () => {
    noise = (noise * 9301 + 49297) % 233280;
    return noise / 233280;
  };
  for (let i = 0; i < bins; i++) {
    const t = i / rate;
    const phase = ((((t - firstBeat) % period) + period) % period) / period;
    const breakdown = t > seconds * 0.45 && t < seconds * 0.55;
    const intro = Math.min(1, t / 8);
    const kick = breakdown ? 0 : Math.exp(-phase * 9);
    const hat = breakdown ? 0 : Math.exp(-Math.abs(phase - 0.5) * 30);
    const pad = 0.25 + 0.15 * Math.sin(t * 0.7 + seed);
    level0[i * 3] = Math.round(255 * Math.min(1, intro * (0.15 + 0.85 * kick)));
    level0[i * 3 + 1] = Math.round(255 * Math.min(1, pad + 0.2 * rand()));
    level0[i * 3 + 2] = Math.round(255 * Math.min(1, 0.1 + 0.6 * hat + 0.15 * rand()));
  }
  const levels = [level0];
  while (levels.length < 4) {
    const prev = levels[levels.length - 1] ?? level0;
    const n = Math.ceil(prev.length / BANDS / LEVEL_FACTOR);
    const next = new Uint8Array(n * BANDS);
    for (let i = 0; i < prev.length / BANDS; i++) {
      const j = Math.floor(i / LEVEL_FACTOR) * BANDS;
      for (let b = 0; b < BANDS; b++) {
        next[j + b] = Math.max(next[j + b] ?? 0, prev[i * BANDS + b] ?? 0);
      }
    }
    levels.push(next);
  }
  return { sampleRate, framesPerBin, levels };
}

/** Largest value per band over the bins covering [from, to) seconds at `level`; zero outside
 *  the track. Writes into `out` (no allocation per pixel). */
export function columnMax(p: Peaks, level: number, from: number, to: number, out: number[]): void {
  out[0] = out[1] = out[2] = 0;
  const data = p.levels[level];
  if (!data) return;
  const rate = binRate(p, level);
  const bins = data.length / BANDS;
  if (to <= 0) return;
  const first = Math.max(0, Math.floor(from * rate));
  const last = Math.min(bins, Math.max(first + 1, Math.ceil(to * rate)));
  for (let i = first; i < last; i++) {
    for (let b = 0; b < BANDS; b++) {
      const v = data[i * BANDS + b] ?? 0;
      if (v > (out[b] ?? 0)) out[b] = v;
    }
  }
}

/** Where a seek may land: never on the very last frame, which would end the track at once. */
export function seekTarget(fraction: number, frames: number): number {
  const f = Number.isFinite(fraction) ? Math.min(1, Math.max(0, fraction)) : 0;
  return Math.max(0, Math.min(frames - 1, Math.round(f * frames)));
}
