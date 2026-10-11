import type { TransitionKind } from '../ipc/types';

export const TRANSITION_NAMES: Record<TransitionKind, string> = {
  echoOut: 'Echo-Out',
  filterOut: 'Filter-Out',
};

/** EQ position (0..1) as dB text for screen readers. */
export function eqText(v: number, killed: boolean): string {
  if (killed || v <= 0) return 'Kill';
  if (v <= 0.5) return `${(40 * Math.log10(v * 2)).toFixed(1)} dB`;
  return `+${((v - 0.5) * 12).toFixed(1)} dB`;
}

/** Filter position as text: low-pass, off, high-pass. */
export function filterText(v: number): string {
  if (Math.abs(v - 0.5) <= 0.03) return 'aus';
  return v < 0.5
    ? `Tiefpass ${Math.round((0.5 - v) * 200)} %`
    : `Hochpass ${Math.round((v - 0.5) * 200)} %`;
}

/** Headphone blend: `Cue`, `Master` at the ends, else both shares. */
export function cueMixText(value: number): string {
  const v = Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0.5;
  if (v <= 0.005) return 'Nur Cue';
  if (v >= 0.995) return 'Nur Master';
  return `Cue ${Math.round((1 - v) * 100)} % · Master ${Math.round(v * 100)} %`;
}
