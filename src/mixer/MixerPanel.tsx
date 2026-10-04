import styles from './MixerPanel.module.css';

/** Mixer column between the decks. Channel strips arrive with the engine in M1. */
export function MixerPanel() {
  return (
    <section className={styles.mixer} aria-label="Mixer">
      <span className={styles.label}>Mixer</span>
    </section>
  );
}
