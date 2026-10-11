import { describe, expect, it } from 'vitest';
import { DEFAULT_PREFS, parsePrefs, resolveTheme } from './prefs';

describe('parsePrefs', () => {
  it('falls back to the defaults for missing or broken data', () => {
    expect(parsePrefs(null)).toEqual(DEFAULT_PREFS);
    expect(parsePrefs('{not json')).toEqual(DEFAULT_PREFS);
    expect(parsePrefs('42')).toEqual(DEFAULT_PREFS);
    expect(parsePrefs('null')).toEqual(DEFAULT_PREFS);
  });

  it('keeps valid fields and replaces unknown ones one by one', () => {
    expect(parsePrefs(JSON.stringify({ theme: 'light', tempoRange: 'huge', curve: 'cut' }))).toEqual({
      theme: 'light',
      tempoRange: DEFAULT_PREFS.tempoRange,
      curve: 'cut',
    });
  });

  it('round-trips every value', () => {
    const p = { theme: 'system', tempoRange: 'wide', curve: 'linear' } as const;
    expect(parsePrefs(JSON.stringify(p))).toEqual(p);
  });
});

describe('resolveTheme', () => {
  it('follows the system only for "system"', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
    expect(resolveTheme('light', true)).toBe('light');
  });
});
