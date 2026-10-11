import { useEffect, useRef, useState } from 'react';
import { backend } from '../ipc';
import type { Deck, Grid } from '../ipc/types';
import { positionAt } from '../state/extrapolate';
import { store } from '../state/store';
import { gridFromTap, scaleGrid, shiftGrid, tapTempo } from './beat';
import styles from './GridEditor.module.css';

/** Seconds the grid moves per arrow press. */
const SHIFT_STEP = 0.01;

interface Props {
  deck: Deck;
  libraryId: number;
  grid: Grid | null;
}

/** Correct the beatgrid: move it, halve or double the tempo, fine-tune it, or tap it in. Saved
 *  in the library; decks playing the track get it at once. */
export function GridEditor({ deck, libraryId, grid }: Props) {
  const [open, setOpen] = useState(false);
  const [taps, setTaps] = useState<number[]>([]);
  const root = useRef<HTMLDivElement>(null);
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener('pointerdown', close);
    return () => window.removeEventListener('pointerdown', close);
  }, [open]);

  const save = (g: Grid) =>
    backend.setGrid(libraryId, g.bpm, g.firstBeat).catch((e: unknown) => console.error('grid', e));

  const tap = () => {
    const now = performance.now();
    const next = [...taps, now].slice(-16);
    setTaps(next);
    const bpm = tapTempo(next);
    if (bpm === null) return;
    // The tap was on a beat: anchor the grid at where the deck is now.
    const { frame, receivedAt } = store.get();
    const d = frame?.decks[deck === 'a' ? 0 : 1];
    if (!frame || !d) return;
    void save(gridFromTap(bpm, positionAt(d, frame.sampleRate, receivedAt, now), frame.sampleRate));
  };

  const tapped = tapTempo(taps);

  return (
    <div ref={root} className={styles.root}>
      <button
        type="button"
        className={styles.trigger}
        aria-expanded={open}
        aria-label={`Beatgrid ${label} korrigieren`}
        onClick={() => setOpen((o) => !o)}
      >
        GRID
      </button>
      {open && (
        <div
          className={styles.panel}
          role="dialog"
          aria-label={`Beatgrid ${label}`}
          onKeyDown={(e) => e.key === 'Escape' && setOpen(false)}
        >
          {grid ? (
            <div className={styles.row}>
              <button
                type="button"
                onClick={() => void save(shiftGrid(grid, -SHIFT_STEP))}
                aria-label="Grid früher"
              >
                ◀
              </button>
              <button
                type="button"
                onClick={() => void save(shiftGrid(grid, SHIFT_STEP))}
                aria-label="Grid später"
              >
                ▶
              </button>
              <button
                type="button"
                onClick={() => void save(scaleGrid(grid, 0.5))}
                aria-label="Tempo halbieren"
              >
                ÷2
              </button>
              <button
                type="button"
                onClick={() => void save(scaleGrid(grid, 2))}
                aria-label="Tempo verdoppeln"
              >
                ×2
              </button>
              <button
                type="button"
                onClick={() =>
                  void save({ ...grid, bpm: Math.round((grid.bpm - 0.01) * 100) / 100 })
                }
                aria-label="Tempo −0,01 BPM"
              >
                −.01
              </button>
              <button
                type="button"
                onClick={() =>
                  void save({ ...grid, bpm: Math.round((grid.bpm + 0.01) * 100) / 100 })
                }
                aria-label="Tempo +0,01 BPM"
              >
                +.01
              </button>
            </div>
          ) : (
            <p className={styles.hint}>Kein Beatgrid. Zum Setzen im Takt tippen.</p>
          )}
          <div className={styles.row}>
            <button type="button" className={styles.tap} onClick={tap} aria-label="Tap Tempo">
              TAP
            </button>
            <span className={`${styles.readout} numeric`} aria-live="polite">
              {tapped !== null
                ? `${tapped.toFixed(2)} BPM`
                : taps.length > 0
                  ? `${Math.min(taps.length, 4)} / 4`
                  : 'im Takt tippen'}
            </span>
          </div>
        </div>
      )}
    </div>
  );
}
