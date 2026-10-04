/** Knob travel: 270° from bottom-left (7:30) to bottom-right (4:30), clockwise. */
export const START_DEG = -135;
export const SWEEP_DEG = 270;

/** Value 0..1 → angle in degrees, 0 = 12 o'clock, clockwise positive. */
export function valueToAngle(value: number): number {
  const v = Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0.5;
  return START_DEG + v * SWEEP_DEG;
}

function polar(cx: number, cy: number, r: number, deg: number): [number, number] {
  const rad = ((deg - 90) * Math.PI) / 180;
  return [cx + r * Math.cos(rad), cy + r * Math.sin(rad)];
}

/** SVG arc path between two angles (degrees, as above). Empty when the angles coincide. */
export function arcPath(cx: number, cy: number, r: number, fromDeg: number, toDeg: number): string {
  if (Math.abs(toDeg - fromDeg) < 0.01) return '';
  const [a, b] = fromDeg < toDeg ? [fromDeg, toDeg] : [toDeg, fromDeg];
  const [x1, y1] = polar(cx, cy, r, a);
  const [x2, y2] = polar(cx, cy, r, b);
  const large = b - a > 180 ? 1 : 0;
  return `M ${x1.toFixed(2)} ${y1.toFixed(2)} A ${r} ${r} 0 ${large} 1 ${x2.toFixed(2)} ${y2.toFixed(2)}`;
}

/** Pixels of vertical drag for the full range; Shift makes it ten times finer. */
export const DRAG_RANGE_PX = 200;

export function dragValue(start: number, dy: number, fine: boolean): number {
  const range = DRAG_RANGE_PX * (fine ? 10 : 1);
  return Math.min(1, Math.max(0, start - dy / range));
}
