import type { Ref } from 'react';
import { copyrightLine } from '../shell/copyright';
import { AudioSettings } from './AudioSettings';
import { ControllerSettings } from './ControllerSettings';
import { DisplaySettings } from './DisplaySettings';
import styles from './SettingsDialog.module.css';

/** Settings: audio, controller, display and playback. */
interface Props {
  ref: Ref<HTMLDialogElement>;
  open: boolean;
  onClose: () => void;
  onOpenMonitor: () => void;
}

export function SettingsDialog({ ref, open, onClose, onOpenMonitor }: Props) {
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="settings-title" onClose={onClose}>
      <h2 id="settings-title" className={styles.title}>
        Einstellungen
      </h2>
      <h3 className={styles.section}>Audio</h3>
      <AudioSettings open={open} />
      <h3 className={styles.section}>Controller</h3>
      <ControllerSettings onOpenMonitor={onOpenMonitor} />
      <h3 className={styles.section}>Darstellung &amp; Wiedergabe</h3>
      <DisplaySettings />
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Fertig
        </button>
      </form>
      <footer className={styles.footer}>{copyrightLine()}</footer>
    </dialog>
  );
}
