import type { Ref } from 'react';
import { copyrightLine } from '../shell/copyright';
import { AudioSettings } from './AudioSettings';
import { ControllerSettings } from './ControllerSettings';
import styles from './SettingsDialog.module.css';

/** Settings: audio for now; controller and display arrive with later milestones. */
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
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Fertig
        </button>
      </form>
      <footer className={styles.footer}>{copyrightLine()}</footer>
    </dialog>
  );
}
