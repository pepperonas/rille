/** Footer line for the About dialog. The year is taken from the clock, never hard-coded. */
export function copyrightLine(now: Date = new Date()): string {
  return `© ${now.getFullYear()} Martin Pfeffer | celox.io`;
}
