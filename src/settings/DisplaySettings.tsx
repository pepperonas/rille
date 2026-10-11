import type { Curve, TempoRange } from '../ipc/types';
import { useAppState } from '../state/store';
import { setPrefs } from './applyPrefs';
import type { ThemePref } from './prefs';
import styles from './AudioSettings.module.css';

const THEMES: { value: ThemePref; label: string }[] = [
  { value: 'dark', label: 'Dunkel' },
  { value: 'light', label: 'Hell' },
  { value: 'system', label: 'Wie das System' },
];

const RANGES: { value: TempoRange; label: string }[] = [
  { value: 'six', label: '±6 %' },
  { value: 'ten', label: '±10 %' },
  { value: 'sixteen', label: '±16 %' },
  { value: 'wide', label: 'Weit (±50 %)' },
];

const CURVES: { value: Curve; label: string }[] = [
  { value: 'smooth', label: 'Blend – beide Seiten weich' },
  { value: 'linear', label: 'Linear' },
  { value: 'cut', label: 'Cut – fürs Scratchen' },
];

/** Theme, the tempo range decks start with, and the crossfader curve. Saved on this Mac. */
export function DisplaySettings() {
  const prefs = useAppState((s) => s.prefs);
  return (
    <div className={styles.settings}>
      <label className={styles.field}>
        <span className={styles.label}>Erscheinungsbild</span>
        <select
          className={styles.select}
          value={prefs.theme}
          onChange={(e) => setPrefs({ theme: e.target.value as ThemePref })}
        >
          {THEMES.map((t) => (
            <option key={t.value} value={t.value}>
              {t.label}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.field}>
        <span className={styles.label}>Tempo-Bereich beim Start</span>
        <span className={styles.hint}>
          Gilt ab dem nächsten Start; spielende Decks behalten ihren Bereich.
        </span>
        <select
          className={styles.select}
          value={prefs.tempoRange}
          onChange={(e) => setPrefs({ tempoRange: e.target.value as TempoRange })}
        >
          {RANGES.map((r) => (
            <option key={r.value} value={r.value}>
              {r.label}
            </option>
          ))}
        </select>
      </label>
      <label className={styles.field}>
        <span className={styles.label}>Crossfader-Kurve</span>
        <select
          className={styles.select}
          value={prefs.curve}
          onChange={(e) => setPrefs({ curve: e.target.value as Curve })}
        >
          {CURVES.map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      </label>
    </div>
  );
}
