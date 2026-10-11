import { useRef } from 'react';
import type { Deck } from '../ipc/types';
import { formatRemaining, formatTime, positionAt } from '../state/extrapolate';
import { store, useAppState } from '../state/store';
import { useAnimationFrame } from '../state/useAnimationFrame';
import styles from './TimeReadout.module.css';

/** Seconds before the end at which the remaining time turns into a warning. */
export const WARN_SECONDS = 30;

/** Elapsed / remaining time. Written directly to the DOM at display rate,
 *  so React does not re-render 120 times per second. */
export function TimeReadout({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);
  const elapsed = useRef<HTMLSpanElement>(null);
  const remaining = useRef<HTMLSpanElement>(null);
  const root = useRef<HTMLDivElement>(null);

  useAnimationFrame(playing, (now) => {
    const { frame, receivedAt } = store.get();
    const d = frame?.decks[i];
    if (!frame || !d) return;
    const pos = positionAt(d, frame.sampleRate, receivedAt, now);
    if (elapsed.current) elapsed.current.textContent = formatTime(pos, frame.sampleRate);
    if (remaining.current) {
      remaining.current.textContent = formatRemaining(pos, d.frames, frame.sampleRate);
    }
    // The last 30 seconds: time to have the next track ready.
    const warn = d.frames > 0 && (d.frames - pos) / frame.sampleRate < WARN_SECONDS;
    if (root.current && (root.current.dataset.warn === 'true') !== warn) {
      root.current.dataset.warn = String(warn);
    }
  });

  // While paused, re-render when position or length change (seek, cue); the hook then draws.
  useAppState((s) => {
    const d = s.frame?.decks[i];
    return d && !d.playing ? `${d.position}/${d.frames}` : '';
  });

  return (
    <div ref={root} className={styles.readout} data-warn="false">
      <div className={styles.times}>
        <span ref={elapsed} className={`${styles.elapsed} numeric`} aria-label="Gespielt">
          0:00.0
        </span>
        <span ref={remaining} className={`${styles.remaining} numeric`} aria-label="Restzeit">
          −0:00.0
        </span>
      </div>
    </div>
  );
}
