import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { useAppState } from '../state/store';
import { useOptimistic } from '../state/useOptimistic';
import styles from './Tempo.module.css';

/** SYNC matches tempo and phase to the other deck once; LOCK keeps them matched. */
export function SyncButtons({ deck }: { deck: Deck }) {
  const i = deck === 'a' ? 0 : 1;
  const locked = useAppState((s) => s.frame?.decks[i]?.syncLock ?? false);
  const canSync = useAppState((s) => !!s.frame?.decks[i]?.grid && !!s.frame.decks[1 - i]?.grid);
  const lock = useOptimistic(locked);
  return (
    <span className={styles.buttons} role="group" aria-label="Sync">
      <button
        type="button"
        className={styles.toggle}
        disabled={!canSync}
        title="Tempo und Phase an das andere Deck angleichen"
        onClick={() => backend.deck(deck, { type: 'sync' })}
      >
        SYNC
      </button>
      <button
        type="button"
        className={styles.toggle}
        aria-label="Sync Lock"
        aria-pressed={lock.shown}
        disabled={!canSync && !locked}
        title="Sync Lock: folgt dem anderen Deck dauerhaft"
        onClick={() => {
          const on = !lock.current();
          lock.set(on);
          backend.deck(deck, { type: 'syncLock', on });
        }}
      >
        LOCK
      </button>
    </span>
  );
}
