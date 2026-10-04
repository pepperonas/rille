/** Bottom of the meter scale in dBFS. */
export const METER_FLOOR_DB = -48;
/** Level from which the meter shows the warning colour. */
export const METER_WARN_DB = -6;
/** Level from which the meter shows clipping. */
export const METER_CLIP_DB = -0.5;

export function toDb(linear: number): number {
  return linear > 0 ? 20 * Math.log10(linear) : -Infinity;
}

/** Linear peak → bar height 0..1 on a dB scale. */
export function meterFill(linear: number): number {
  const db = toDb(linear);
  if (!Number.isFinite(db) || db <= METER_FLOOR_DB) return 0;
  return Math.min(1, (db - METER_FLOOR_DB) / -METER_FLOOR_DB);
}
