import { backend } from '../ipc';
import type { Curve, Deck } from '../ipc/types';
import { Fader } from '../components/Fader';
import { Meter } from '../components/Meter';
import type { AppState } from '../state/store';
import { useAppState } from '../state/store';
import styles from './MixerPanel.module.css';

const peakA = (s: AppState) => s.frame?.decks[0].peak;
const peakB = (s: AppState) => s.frame?.decks[1].peak;
const peakMaster = (s: AppState) => s.frame?.masterPeak;

const CURVES: { value: Curve; label: string; hint: string }[] = [
  { value: 'smooth', label: 'Blend', hint: 'Konstante Leistung, für Übergänge' },
  { value: 'linear', label: 'Linear', hint: 'Lineare Überblendung' },
  { value: 'cut', label: 'Cut', hint: 'Scharfe Kurve zum Scratchen' },
];

function Channel({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const value = useAppState((s) => s.frame?.channelFader[i] ?? 1);
  const label = deck === 'a' ? 'Kanal 1' : 'Kanal 2';
  return (
    <div className={styles.channel} data-deck={deck}>
      <span className={styles.channelLabel}>{deck === 'a' ? '1' : '2'}</span>
      <div className={styles.strip}>
        <Meter label={`Pegel ${label}`} select={deck === 'a' ? peakA : peakB} />
        <Fader
          label={`Kanalfader ${label}`}
          value={value}
          defaultValue={1}
          onChange={(v) => backend.mixer({ type: 'channelFader', deck, value: v })}
        />
      </div>
    </div>
  );
}

/** Mixer column between the decks: channel faders, master, crossfader. */
export function MixerPanel() {
  const crossfader = useAppState((s) => s.frame?.crossfader ?? 0.5);
  const curve = useAppState((s) => s.frame?.curve ?? 'smooth');
  const master = useAppState((s) => s.frame?.masterGain ?? 1);

  return (
    <section className={styles.mixer} aria-label="Mixer">
      <span className={styles.label}>Mixer</span>
      <div className={styles.channels}>
        <Channel deck="a" />
        <div className={styles.master}>
          <span className={styles.channelLabel}>M</span>
          <div className={styles.strip}>
            <Meter label="Pegel Master" select={peakMaster} />
            <Fader
              label="Master-Lautstärke"
              value={master}
              defaultValue={1}
              onChange={(v) => backend.mixer({ type: 'masterGain', value: v })}
            />
          </div>
        </div>
        <Channel deck="b" />
      </div>
      <div className={styles.crossfader}>
        <Fader
          label="Crossfader"
          orientation="horizontal"
          bipolar
          value={crossfader}
          defaultValue={0.5}
          onChange={(v) => backend.mixer({ type: 'crossfader', value: v })}
        />
        <div className={styles.curves} role="radiogroup" aria-label="Crossfader-Kurve">
          {CURVES.map((c) => (
            <button
              key={c.value}
              type="button"
              role="radio"
              aria-checked={curve === c.value}
              title={c.hint}
              className={styles.chip}
              onClick={() => backend.mixer({ type: 'curve', curve: c.value })}
            >
              {c.label}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
}
