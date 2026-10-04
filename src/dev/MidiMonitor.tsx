import { useEffect, useRef, useState } from 'react';
import type { Ref } from 'react';
import { backend } from '../ipc';
import type { MonitorLine } from '../ipc/types';
import styles from './MidiMonitor.module.css';

const MAX_LINES = 400;

/** Developer view: every MIDI message in and out, live. Streams only while open. */
export function MidiMonitor({ ref, open, onClose }: { ref: Ref<HTMLDialogElement>; open: boolean; onClose: () => void }) {
  const [lines, setLines] = useState<MonitorLine[]>([]);
  const [paused, setPaused] = useState(false);
  const pausedRef = useRef(paused);
  useEffect(() => {
    pausedRef.current = paused;
  }, [paused]);

  useEffect(() => {
    if (!open) return;
    let off: (() => void) | undefined;
    let cancelled = false;
    // Listen first, then switch the stream on, so the first batch is not lost.
    backend
      .on('midi-monitor', (batch) => {
        if (pausedRef.current) return;
        setLines((prev) => [...prev, ...batch].slice(-MAX_LINES));
      })
      .then((unsubscribe) => {
        if (cancelled) {
          unsubscribe();
          return;
        }
        off = unsubscribe;
        void backend.setMidiMonitor(true);
      });
    return () => {
      cancelled = true;
      off?.();
      void backend.setMidiMonitor(false);
    };
  }, [open]);

  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="midi-title" onClose={onClose}>
      <header className={styles.header}>
        <h2 id="midi-title" className={styles.title}>
          MIDI-Monitor
        </h2>
        <button type="button" className={styles.action} onClick={() => setPaused((p) => !p)}>
          {paused ? 'Fortsetzen' : 'Anhalten'}
        </button>
        <button type="button" className={styles.action} onClick={() => setLines([])}>
          Leeren
        </button>
        <form method="dialog">
          <button type="submit" className={styles.action}>
            Schließen
          </button>
        </form>
      </header>
      <div className={styles.list} role="log" aria-live="off">
        {lines.length === 0 && (
          <p className={styles.empty}>
            Noch keine Nachrichten. Bewege ein Bedienelement am DDJ-200.
          </p>
        )}
        {lines.map((l) => (
          <div key={l.seq} className={styles.line} data-out={l.outgoing || undefined}>
            <span className="numeric">{(l.timeUs / 1000).toFixed(1).padStart(9)} ms</span>
            <span className={styles.dir}>{l.outgoing ? '→' : '←'}</span>
            <span className={`${styles.hex} numeric`}>{l.hex}</span>
            <span className={styles.meaning}>{l.meaning ?? (l.outgoing ? 'LED/Einstellung' : '')}</span>
          </div>
        ))}
      </div>
    </dialog>
  );
}
