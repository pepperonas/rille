import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { useAppState } from '../state/store';
import { setPadMode } from './padMode';
import { beatFrames, beatsLabel, JUMP_BEATS, LOOP_BEATS, PAD_MODES } from './beat';
import styles from './Pads.module.css';

/** Eight performance pads in three modes, as on the DDJ-200. Shift + pad: delete the hot cue,
 *  or leave the loop. */
export function Pads({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  const mode = useAppState((s) => s.padMode[deck]);
  const cues = useAppState((s) => s.frame?.decks[i]?.hotCues.join(',') ?? '');
  const grid = useAppState((s) => s.frame?.decks[i]?.grid ?? null);
  const loopBeats = useAppState((s) => {
    const d = s.frame?.decks[i];
    if (!d?.loopRange || !d.grid || !s.frame) return null;
    return (d.loopRange[1] - d.loopRange[0]) / beatFrames(d.grid, s.frame.sampleRate);
  });
  const set = cues.split(',').map((c) => c !== '');

  const press = (index: number, shift: boolean) => {
    if (mode === 'hotCue') {
      backend.deck(
        deck,
        shift ? { type: 'hotCueClear', slot: index } : { type: 'hotCue', slot: index },
      );
    } else if (mode === 'beatLoop') {
      const beats = LOOP_BEATS[index] ?? 1;
      backend.deck(deck, shift ? { type: 'loopExit' } : { type: 'beatLoop', beats });
    } else {
      backend.deck(deck, { type: 'beatJump', beats: JUMP_BEATS[index] ?? 1 });
    }
  };

  const pad = (index: number) => {
    if (mode === 'hotCue') {
      return {
        text: String(index + 1),
        name: set[index] ? `Hot Cue ${index + 1} anspringen` : `Hot Cue ${index + 1} setzen`,
        lit: set[index] ?? false,
        disabled: false,
      };
    }
    if (mode === 'beatLoop') {
      const beats = LOOP_BEATS[index] ?? 1;
      return {
        text: beatsLabel(beats),
        name: `Loop ${beatsLabel(beats)} Beats`,
        lit: loopBeats !== null && Math.abs(loopBeats - beats) < 0.01,
        disabled: !grid,
      };
    }
    const beats = JUMP_BEATS[index] ?? 1;
    return {
      text: beatsLabel(beats),
      name: `${beats < 0 ? 'Zurück' : 'Vor'} ${beatsLabel(Math.abs(beats))} Beats`,
      lit: false,
      disabled: !grid,
    };
  };

  return (
    <div className={styles.pads}>
      <div className={styles.modes} role="radiogroup" aria-label={`Pad-Modus ${label}`}>
        {PAD_MODES.map((m) => (
          <button
            key={m.mode}
            type="button"
            role="radio"
            aria-checked={mode === m.mode}
            className={styles.mode}
            onClick={() => setPadMode(deck, m.mode)}
          >
            {m.label}
          </button>
        ))}
      </div>
      <div className={styles.grid} role="group" aria-label={`Pads ${label}`}>
        {Array.from({ length: 8 }, (_, index) => {
          const p = pad(index);
          return (
            <button
              key={index}
              type="button"
              className={`${styles.pad} numeric`}
              data-lit={p.lit || undefined}
              aria-label={p.name}
              aria-pressed={mode === 'beatJump' ? undefined : p.lit}
              disabled={p.disabled}
              title={
                mode === 'hotCue'
                  ? 'Klick: setzen oder anspringen · Shift+Klick: löschen'
                  : mode === 'beatLoop'
                    ? 'Klick: Loop an/aus · Shift+Klick: Loop verlassen'
                    : undefined
              }
              onClick={(e) => press(index, e.shiftKey)}
            >
              {p.text}
            </button>
          );
        })}
      </div>
    </div>
  );
}
