import { backend } from '../ipc';
import type { Band, Curve, Deck } from '../ipc/types';
import { Fader } from '../components/Fader';
import { Knob } from '../components/Knob';
import { Meter } from '../components/Meter';
import type { AppState } from '../state/store';
import { useAppState } from '../state/store';
import { useOptimistic } from '../state/useOptimistic';
import { TRANSITION_NAMES, eqText, filterText } from './valueText';
import styles from './MixerPanel.module.css';

const peakA = (s: AppState) => s.frame?.decks[0].peak;
const peakB = (s: AppState) => s.frame?.decks[1].peak;
const peakMaster = (s: AppState) => s.frame?.masterPeak;

const CURVES: { value: Curve; label: string; hint: string }[] = [
  { value: 'smooth', label: 'Blend', hint: 'Konstante Leistung, für Übergänge' },
  { value: 'linear', label: 'Linear', hint: 'Lineare Überblendung' },
  { value: 'cut', label: 'Cut', hint: 'Scharfe Kurve zum Scratchen' },
];

const BANDS: { band: Band; index: number; caption: string; name: string }[] = [
  { band: 'high', index: 2, caption: 'HI', name: 'Höhen' },
  { band: 'mid', index: 1, caption: 'MID', name: 'Mitten' },
  { band: 'low', index: 0, caption: 'LOW', name: 'Bässe' },
];

interface EqKnobProps {
  deck: Deck;
  band: Band;
  caption: string;
  name: string;
  channel: string;
  value: number;
  killed: boolean;
}

/** One EQ band; the caption toggles kill. Optimistic, so a quick double click toggles twice. */
function EqKnob({ deck, band, caption, name, channel, value, killed }: EqKnobProps) {
  const kill = useOptimistic(killed);
  return (
    <Knob
      label={`${name} ${channel}`}
      caption={caption}
      value={value}
      muted={kill.shown}
      valueText={eqText(value, kill.shown)}
      onChange={(v) => backend.mixer({ type: 'eq', deck, band, value: v })}
      captionToggle={{
        label: `${name} ${channel} stummschalten (Kill)`,
        pressed: kill.shown,
        onToggle: () => {
          const next = !kill.current();
          kill.set(next);
          backend.mixer({ type: 'eqKill', deck, band, kill: next });
        },
      }}
    />
  );
}

function Channel({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const fader = useAppState((s) => s.frame?.channelFader[i] ?? 1);
  const eq = useAppState((s) => s.frame?.eq[i]);
  const kill = useAppState((s) => s.frame?.eqKill[i]);
  const filter = useAppState((s) => s.frame?.filter[i] ?? 0.5);
  const label = deck === 'a' ? 'Kanal 1' : 'Kanal 2';
  return (
    <div className={styles.channel} data-deck={deck}>
      <span className={styles.channelLabel}>{deck === 'a' ? '1' : '2'}</span>
      <div className={styles.knobs}>
        {BANDS.map(({ band, index, caption, name }) => (
          <EqKnob
            key={band}
            deck={deck}
            band={band}
            caption={caption}
            name={name}
            channel={label}
            value={eq?.[index] ?? 0.5}
            killed={kill?.[index] ?? false}
          />
        ))}
        <Knob
          label={`Filter ${label}`}
          caption="FILTER"
          value={filter}
          valueText={filterText(filter)}
          onChange={(value) => backend.mixer({ type: 'filter', deck, value })}
        />
      </div>
      <div className={styles.strip}>
        <Meter label={`Pegel ${label}`} select={deck === 'a' ? peakA : peakB} />
        <Fader
          label={`Kanalfader ${label}`}
          value={fader}
          defaultValue={1}
          onChange={(v) => backend.mixer({ type: 'channelFader', deck, value: v })}
        />
      </div>
    </div>
  );
}

function TransitionFx() {
  const t = useAppState((s) => s.frame?.transition);
  const kind = t?.kind ?? 'echoOut';
  const running = !!t?.deck;
  const releasing = !!t?.releasing;
  const deckLabel = t?.deck === 'a' ? 'Deck 1' : t?.deck === 'b' ? 'Deck 2' : null;
  return (
    <div className={styles.transition}>
      <button
        type="button"
        className={styles.fxButton}
        data-running={running || undefined}
        data-releasing={releasing || undefined}
        aria-pressed={running}
        disabled={releasing}
        aria-label={`Transition FX: ${TRANSITION_NAMES[kind]}${deckLabel ? ` auf ${deckLabel}` : ''}`}
        title={
          releasing
            ? 'Abgebrochen – der Nachhall klingt aus'
            : running
              ? 'Nochmal drücken bricht ab'
              : 'Blendet das lautere spielende Deck aus und pausiert es danach'
        }
        onClick={() => backend.mixer({ type: 'transitionFx' })}
      >
        {TRANSITION_NAMES[kind]}
        {deckLabel && (
          <span className={styles.fxDeck}> · {releasing ? 'klingt aus' : deckLabel}</span>
        )}
      </button>
      <button
        type="button"
        className={styles.fxCycle}
        disabled={running}
        aria-label={`Effekt wechseln (jetzt ${TRANSITION_NAMES[kind]})`}
        title="Effekt wechseln"
        onClick={() => backend.mixer({ type: 'cycleTransitionFx' })}
      >
        <svg viewBox="0 0 24 24" width="18" height="18" aria-hidden="true">
          <path fill="currentColor" d="M7 7h10v3l4-4-4-4v3H5v6h2V7Zm10 10H7v-3l-4 4 4 4v-3h12v-6h-2v4Z" />
        </svg>
      </button>
    </div>
  );
}

/** Mixer column between the decks: EQ, filter, faders, crossfader, transition effect. */
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
        <TransitionFx />
      </div>
    </section>
  );
}
