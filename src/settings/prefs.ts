// User preferences kept in the webview's localStorage. Pure parsing so broken or old data
// never stops the app; applying them is in `applyPrefs.ts`.
import type { Curve, TempoRange } from '../ipc/types';

export type ThemePref = 'dark' | 'light' | 'system';

export interface Prefs {
  theme: ThemePref;
  /** Tempo range both decks start with. */
  tempoRange: TempoRange;
  curve: Curve;
}

export const DEFAULT_PREFS: Prefs = { theme: 'dark', tempoRange: 'ten', curve: 'smooth' };
export const PREFS_KEY = 'rille.prefs';

const THEMES: readonly ThemePref[] = ['dark', 'light', 'system'];
const RANGES: readonly TempoRange[] = ['six', 'ten', 'sixteen', 'wide'];
const CURVES: readonly Curve[] = ['smooth', 'linear', 'cut'];

function pick<T extends string>(value: unknown, allowed: readonly T[], fallback: T): T {
  return allowed.find((a) => a === value) ?? fallback;
}

/** Stored text → preferences; anything unknown falls back to the default, field by field. */
export function parsePrefs(raw: string | null): Prefs {
  let data: unknown;
  try {
    data = raw === null ? null : JSON.parse(raw);
  } catch {
    data = null;
  }
  const o = typeof data === 'object' && data !== null ? (data as Record<string, unknown>) : {};
  return {
    theme: pick(o.theme, THEMES, DEFAULT_PREFS.theme),
    tempoRange: pick(o.tempoRange, RANGES, DEFAULT_PREFS.tempoRange),
    curve: pick(o.curve, CURVES, DEFAULT_PREFS.curve),
  };
}

/** The theme actually shown: "system" follows the OS. */
export function resolveTheme(pref: ThemePref, systemDark: boolean): 'dark' | 'light' {
  if (pref === 'system') return systemDark ? 'dark' : 'light';
  return pref;
}

export function loadPrefs(): Prefs {
  try {
    return parsePrefs(localStorage.getItem(PREFS_KEY));
  } catch {
    return DEFAULT_PREFS;
  }
}

export function savePrefs(p: Prefs) {
  try {
    localStorage.setItem(PREFS_KEY, JSON.stringify(p));
  } catch {
    // Storage blocked: the setting still applies for this session.
  }
}
