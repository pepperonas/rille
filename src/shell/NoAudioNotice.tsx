import { useAppState } from '../state/store';
import { openDialog } from './dialogs';
import styles from './NoAudioNotice.module.css';

/** Without an output device nothing is audible; say so above the decks and offer the fix. */
export function NoAudioNotice() {
  const audio = useAppState((s) => s.audio);
  if (audio === null || audio.connected) return null;
  return (
    <div className={styles.notice} role="alert">
      <span className={styles.text}>
        <strong>Kein Audio-Gerät.</strong> {audio.error ?? 'Ohne Ausgabe ist nichts zu hören.'}
      </span>
      <button type="button" className={styles.button} onClick={() => openDialog('settings')}>
        Ausgabe wählen
      </button>
    </div>
  );
}
