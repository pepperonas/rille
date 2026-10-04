import { useRef, useState } from 'react';
import { Fader } from '../components/Fader';
import { backend } from '../ipc';
import type { Deck, TempoRange } from '../ipc/types';
import {
  NEXT_RANGE,
  faderToTempo,
  formatRange,
  formatTempo,
  tempoToFader,
} from '../state/extrapolate';
import { useAppState } from '../state/store';
import { useOptimistic } from '../state/useOptimistic';
import { type BendDirection, bendOf, holdBend } from './bend';
import { HoldButton } from './HoldButton';
import styles from './Tempo.module.css';

const index = (deck: Deck) => (deck === 'a' ? 0 : 1);
const RANGES: readonly TempoRange[] = ['six', 'ten', 'sixteen', 'wide'];

/** Range chip, tempo readout and the tempo fader (top = slower, as on the controller). */
export function TempoFader({ deck }: { deck: Deck }) {
  const i = index(deck);
  const tempo = useAppState((s) => s.frame?.decks[i]?.tempo ?? 0);
  const range = useAppState((s) => s.frame?.decks[i]?.tempoRange ?? 'ten');
  // Index into RANGES, so quick double clicks step from the last range sent.
  const shownRange = useOptimistic(RANGES.indexOf(range));
  const currentRange = RANGES[shownRange.shown] ?? 'ten';

  return (
    <div className={styles.column}>
      <button
        type="button"
        className={styles.range}
        aria-label={`Tempobereich ${formatRange(currentRange)}, wechseln`}
        onClick={() => {
          const next = NEXT_RANGE[RANGES[shownRange.current()] ?? 'ten'];
          shownRange.set(RANGES.indexOf(next));
          backend.deck(deck, { type: 'tempoRange', range: next });
        }}
      >
        {formatRange(currentRange)}
      </button>
      <span className={`${styles.readout} numeric`} data-centred={tempo === 0 || undefined}>
        {formatTempo(tempo, currentRange)}
      </span>
      <div className={styles.fader}>
        <Fader
          label="Tempo"
          value={tempoToFader(tempo)}
          bipolar
          defaultValue={0.5}
          valueText={formatTempo(tempo, currentRange)}
          onChange={(v) => backend.deck(deck, { type: 'tempo', value: faderToTempo(v) })}
        />
      </div>
    </div>
  );
}

/** Keylock, reverse and pitch bend for one deck. */
export function TempoButtons({ deck }: { deck: Deck }) {
  const i = index(deck);
  const keylock = useAppState((s) => s.frame?.decks[i]?.keylock ?? false);
  const reverse = useAppState((s) => s.frame?.decks[i]?.reverse ?? false);
  const key = useOptimistic(keylock);
  const [held, setHeld] = useState<BendDirection[]>([]);
  const heldRef = useRef<BendDirection[]>([]);
  const bend = bendOf(held);

  const hold = (direction: BendDirection, down: boolean) => {
    const next = holdBend(heldRef.current, direction, down);
    const changed = bendOf(next) !== bendOf(heldRef.current);
    heldRef.current = next;
    setHeld(next);
    if (changed) backend.deck(deck, { type: 'bend', direction: bendOf(next) });
  };

  return (
    <div className={styles.buttons}>
      <button
        type="button"
        className={styles.toggle}
        aria-label="Keylock"
        aria-pressed={key.shown}
        title="Keylock: Tonhöhe bleibt bei Tempoänderung"
        onClick={() => {
          const on = !key.current();
          key.set(on);
          backend.deck(deck, { type: 'keylock', on });
        }}
      >
        KEY
      </button>
      <HoldButton
        className={styles.toggle}
        label="Rückwärts (halten)"
        active={reverse}
        onHold={(on) => backend.deck(deck, { type: 'reverse', on })}
      >
        REV
      </HoldButton>
      <span className={styles.bend} role="group" aria-label="Pitch Bend">
        <HoldButton
          className={styles.bendButton}
          label="Langsamer (halten)"
          active={bend === -1}
          onHold={(down) => hold(-1, down)}
        >
          −
        </HoldButton>
        <HoldButton
          className={styles.bendButton}
          label="Schneller (halten)"
          active={bend === 1}
          onHold={(down) => hold(1, down)}
        >
          +
        </HoldButton>
      </span>
    </div>
  );
}
