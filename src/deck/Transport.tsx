import { motion, useReducedMotion } from 'motion/react';
import { useState } from 'react';
import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { shapes, sizes, springs } from '../design/tokens';
import { useAppState } from '../state/store';
import styles from './Transport.module.css';

/** Cue and Play, as on the controller: Cue left, Play right. */
export function Transport({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const playing = useAppState((s) => s.frame?.decks[i]?.playing ?? false);
  const previewing = useAppState((s) => s.frame?.decks[i]?.previewing ?? false);
  const atCue = useAppState((s) => {
    const d = s.frame?.decks[i];
    return !!d && !d.playing && d.position === d.cue;
  });
  const reduce = useReducedMotion();
  // Shown immediately on press, before the engine confirms (< 1 frame of feedback).
  const [cueHeld, setCueHeld] = useState(false);
  const [optimisticPlay, setOptimisticPlay] = useState<boolean | null>(null);
  const showPlaying = (optimisticPlay ?? playing) && !previewing;

  const transition = reduce ? { duration: 0 } : springs.spatialFast;

  return (
    <div className={styles.transport}>
      <motion.button
        type="button"
        className={styles.cue}
        data-active={cueHeld || previewing || atCue || undefined}
        aria-label="Cue"
        animate={{ borderRadius: cueHeld ? shapes.md : sizes.transportButton / 2 }}
        transition={transition}
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId);
          setCueHeld(true);
          backend.deck(deck, e.shiftKey ? { type: 'jumpToStart' } : { type: 'cuePress' });
        }}
        onPointerUp={(e) => {
          setCueHeld(false);
          if (!e.shiftKey) backend.deck(deck, { type: 'cueRelease' });
        }}
        onPointerCancel={() => {
          setCueHeld(false);
          backend.deck(deck, { type: 'cueRelease' });
        }}
      >
        CUE
      </motion.button>
      <motion.button
        type="button"
        className={styles.play}
        data-playing={showPlaying || undefined}
        aria-label={showPlaying ? 'Pause' : 'Play'}
        aria-pressed={showPlaying}
        animate={{ borderRadius: showPlaying ? shapes.lgInc : sizes.playButton / 2 }}
        transition={transition}
        onPointerDown={() => {
          setOptimisticPlay(!showPlaying);
          backend.deck(deck, { type: 'playPause' });
        }}
        onAnimationComplete={() => setOptimisticPlay(null)}
      >
        <svg viewBox="0 0 24 24" width="32" height="32" aria-hidden="true">
          {showPlaying ? (
            <path fill="currentColor" d="M6 5h4v14H6zM14 5h4v14h-4z" />
          ) : (
            <path fill="currentColor" d="M8 5.5v13a1 1 0 0 0 1.5.86l10.5-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5Z" />
          )}
        </svg>
      </motion.button>
    </div>
  );
}
