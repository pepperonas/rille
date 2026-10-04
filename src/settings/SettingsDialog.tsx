import type { Ref } from 'react';
import { copyrightLine } from '../shell/copyright';
import { AudioSettings } from './AudioSettings';
import styles from './SettingsDialog.module.css';

/** Settings: audio for now; controller and display arrive with later milestones. */
interface Props {
  ref: Ref<HTMLDialogElement>;
  open: boolean;
  onClose: () => void;
}

export function SettingsDialog({ ref, open, onClose }: Props) {
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="settings-title" onClose={onClose}>
      <h2 id="settings-title" className={styles.title}>
        Einstellungen
      </h2>
      <h3 className={styles.section}>Audio</h3>
      <AudioSettings open={open} />
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Fertig
        </button>
      </form>
      <footer className={styles.footer}>{copyrightLine()}</footer>
    </dialog>
  );
}
