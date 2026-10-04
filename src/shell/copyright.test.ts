import { describe, expect, it } from 'vitest';
import { copyrightLine } from './copyright';

describe('copyrightLine', () => {
  it('uses the year of the given date', () => {
    expect(copyrightLine(new Date(2031, 0, 1))).toBe('© 2031 Martin Pfeffer | celox.io');
  });

  it('defaults to the current year', () => {
    expect(copyrightLine()).toContain(String(new Date().getFullYear()));
  });
});
