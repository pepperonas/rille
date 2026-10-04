// Spring tokens for `motion`. Mirrors the M3 Expressive motion scheme; CSS durations live in
// tokens.css. Components import these instead of writing their own stiffness/damping values.

export interface SpringToken {
  readonly type: 'spring';
  readonly stiffness: number;
  readonly damping: number;
}

const spring = (stiffness: number, damping: number): SpringToken => ({
  type: 'spring',
  stiffness,
  damping,
});

/** Spatial springs move things (position, size, shape) and may overshoot. */
export const springs = {
  spatialFast: spring(1400, 52),
  spatialDefault: spring(700, 36),
  spatialSlow: spring(300, 30),
  /** Effects springs change colour/opacity and never overshoot. */
  effectsFast: spring(3800, 123),
  effectsDefault: spring(1600, 80),
  effectsSlow: spring(800, 57),
} as const;

/** Critical damping ratio; spatial springs stay below 1 (bounce), effects at or above 1. */
export function dampingRatio(token: SpringToken, mass = 1): number {
  return token.damping / (2 * Math.sqrt(token.stiffness * mass));
}

/** Sizes JS needs as numbers (shape morphs). Mirrors tokens.css; a test keeps them in sync. */
export const sizes = {
  playButton: 72,
  transportButton: 56,
} as const;

/** Corner radii in px. Mirrors the --shape-* tokens. */
export const shapes = {
  md: 12,
  lg: 16,
  lgInc: 20,
  xl: 28,
} as const;
