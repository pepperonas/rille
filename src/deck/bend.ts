export type BendDirection = -1 | 1;

/**
 * Bend buttons that are held, in press order. Both can be down at once (rolling from − to +):
 * the most recently pressed one wins, and letting go of it hands back to the other instead of
 * cancelling the bend.
 */
export function holdBend(
  held: readonly BendDirection[],
  direction: BendDirection,
  down: boolean,
): BendDirection[] {
  const rest = held.filter((d) => d !== direction);
  return down ? [...rest, direction] : rest;
}

/** The bend to send for the held buttons. */
export function bendOf(held: readonly BendDirection[]): -1 | 0 | 1 {
  return held.at(-1) ?? 0;
}
