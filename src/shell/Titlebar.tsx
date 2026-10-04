import { useRef } from 'react';
import { AboutDialog } from './AboutDialog';
import styles from './Titlebar.module.css';

/** Title bar overlay: the native traffic lights sit on top-left, the rest is a drag region. */
export function Titlebar() {
  const about = useRef<HTMLDialogElement>(null);
  return (
    <header className={styles.bar} data-tauri-drag-region>
      <span className={styles.wordmark} data-tauri-drag-region>
        rille
      </span>
      <button
        type="button"
        className={styles.iconButton}
        aria-label="Über rille"
        onClick={() => about.current?.showModal()}
      >
        <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
          <path
            fill="currentColor"
            d="M11 17h2v-6h-2v6Zm1-8a1 1 0 1 0 0-2 1 1 0 0 0 0 2Zm0 13a10 10 0 1 1 0-20 10 10 0 0 1 0 20Z"
          />
        </svg>
      </button>
      <AboutDialog ref={about} />
    </header>
  );
}
