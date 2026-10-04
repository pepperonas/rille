import type { Ref } from 'react';
import { backend } from '../ipc';
import icon from '../../src-tauri/icons/128x128@2x.png';
import { copyrightLine } from './copyright';
import { LINKS } from './links';
import styles from './AboutDialog.module.css';

function open(url: string) {
  backend.openExternal(url).catch((e: unknown) => console.error('could not open link', e));
}

export function AboutDialog({ ref }: { ref: Ref<HTMLDialogElement> }) {
  return (
    <dialog ref={ref} className={styles.dialog} aria-labelledby="about-title">
      <div className={styles.head}>
        <img className={styles.icon} src={icon} alt="" width={64} height={64} />
        <div>
          <h2 id="about-title" className={styles.title}>
            rille
          </h2>
          <p className={styles.version}>Version {__APP_VERSION__}</p>
        </div>
      </div>
      <p className={styles.body}>DJ-App für macOS mit Pioneer DDJ-200.</p>
      <p className={styles.support}>
        Kostenlos, quelloffen, ohne Tracking – eine Person, viele Abende. Wenn rille deine Sets
        besser macht, freue ich mich über einen Kaffee oder eine Bewertung.
      </p>
      <div className={styles.buttons}>
        <button type="button" className={styles.donate} onClick={() => open(LINKS.donate)}>
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path
              fill="currentColor"
              d="M4 19h14v2H4v-2Zm14-12h1a3 3 0 0 1 0 6h-1.1A6 6 0 0 1 12 17h-2a6 6 0 0 1-6-6V5h14v2Zm0 2v2h1a1 1 0 0 0 0-2h-1Z"
            />
          </svg>
          Spenden via PayPal
        </button>
        <button type="button" className={styles.rate} onClick={() => open(LINKS.rate)}>
          <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
            <path
              fill="currentColor"
              d="m12 17.3 6.2 3.7-1.6-7 5.4-4.7-7.1-.6L12 2 9.1 8.7 2 9.3l5.4 4.7-1.6 7L12 17.3Z"
            />
          </svg>
          celox.io bewerten
        </button>
      </div>
      <p className={styles.links}>
        <button type="button" className={styles.link} onClick={() => open(LINKS.repo)}>
          Quellcode auf GitHub
        </button>
        <span aria-hidden="true">·</span>
        <button type="button" className={styles.link} onClick={() => open(LINKS.celox)}>
          celox.io
        </button>
      </p>
      <form method="dialog" className={styles.actions}>
        <button type="submit" className={styles.textButton}>
          Schließen
        </button>
      </form>
      <footer className={styles.footer}>{copyrightLine()}</footer>
    </dialog>
  );
}
