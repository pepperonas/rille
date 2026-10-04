import type { Ref } from 'react';
import { copyrightLine } from './copyright';
import styles from './AboutDialog.module.css';

export function AboutDialog({ ref }: { ref: Ref<HTMLDialogElement> }) {
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="about-title">
      <h2 id="about-title" className={styles.title}>
        rille
      </h2>
      <p className={styles.body}>DJ-App für macOS mit Pioneer DDJ-200.</p>
      <p className={styles.version}>Version {__APP_VERSION__}</p>
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Schließen
        </button>
      </form>
      <footer className={styles.footer}>{copyrightLine()}</footer>
    </dialog>
  );
}
