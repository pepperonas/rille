import styles from './LibraryPanel.module.css';

/** Track library. Until the first import it explains what to do. */
export function LibraryPanel() {
  return (
    <section className={styles.library} aria-label="Library">
      <div className={styles.empty}>
        <p className={styles.emptyTitle}>Noch keine Tracks</p>
        <p className={styles.emptyHint}>
          Importiere einen Ordner mit Musik. rille analysiert BPM und Beatgrid im Hintergrund.
        </p>
      </div>
    </section>
  );
}
