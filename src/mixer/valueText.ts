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
  return v < 0.5 ? `Tiefpass ${Math.round((0.5 - v) * 200)} %` : `Hochpass ${Math.round((v - 0.5) * 200)} %`;
}
