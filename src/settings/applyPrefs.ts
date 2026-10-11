// Applying preferences: theme on the document and the native window, tempo range and
// crossfader curve on the engine.
import { backend } from '../ipc';
import { store } from '../state/store';
import { loadPrefs, resolveTheme, savePrefs, type Prefs, type ThemePref } from './prefs';

const DARK_QUERY = '(prefers-color-scheme: dark)';

function systemDark(): boolean {
  return typeof matchMedia === 'function' ? matchMedia(DARK_QUERY).matches : true;
}

/** Set `data-theme` only; safe before React renders (no flash of the wrong theme). */
export function paintTheme(pref: ThemePref) {
  document.documentElement.dataset.theme = resolveTheme(pref, systemDark());
}

function applyTheme(pref: ThemePref) {
  // The window first: with "system" its appearance decides what `prefers-color-scheme` says.
  backend
    .setWindowTheme(pref === 'system' ? null : pref)
    .catch((e: unknown) => console.error('window theme', e))
    .finally(() => paintTheme(store.get().prefs.theme));
  paintTheme(pref);
}

/** Theme first and as early as possible: the native window must match before the first paint
 *  of a light UI, or the vibrancy shows dark material under it. */
export function initTheme() {
  const prefs = loadPrefs();
  store.update((s) => ({ ...s, prefs }));
  applyTheme(prefs.theme);
}

/** Apply the saved preferences to the engine and follow the system theme. The start tempo
 *  range is applied here only: changing it later must not retune a deck that is playing.
 *  Returns a cleanup function. */
export function initPrefs(): () => void {
  const { prefs } = store.get();
  for (const deck of ['a', 'b'] as const) {
    backend.deck(deck, { type: 'tempoRange', range: prefs.tempoRange });
  }
  backend.mixer({ type: 'curve', curve: prefs.curve });
  if (typeof matchMedia !== 'function') return () => undefined;
  const media = matchMedia(DARK_QUERY);
  const onChange = () => {
    if (store.get().prefs.theme === 'system') paintTheme('system');
  };
  media.addEventListener('change', onChange);
  return () => media.removeEventListener('change', onChange);
}

/** Change, save and apply some preferences. */
export function setPrefs(change: Partial<Prefs>) {
  const next = { ...store.get().prefs, ...change };
  store.update((s) => ({ ...s, prefs: next }));
  savePrefs(next);
  if (change.theme !== undefined) applyTheme(next.theme);
  // The start tempo range waits for the next start (see initPrefs).
  if (change.curve !== undefined) backend.mixer({ type: 'curve', curve: next.curve });
}
