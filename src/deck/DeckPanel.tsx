import styles from './DeckPanel.module.css';

interface Props {
  deck: 'a' | 'b';
}

/** Deck surface. Until a track is loaded it explains how to get one onto the deck. */
export function DeckPanel({ deck }: Props) {
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  return (
    <section className={styles.deck} data-deck={deck} aria-label={label}>
      <header className={styles.header}>
        <span className={styles.badge}>{deck === 'a' ? '1' : '2'}</span>
        <span className={styles.label}>{label}</span>
      </header>
      <div className={styles.empty}>
        <p className={styles.emptyTitle}>Kein Track geladen</p>
        <p className={styles.emptyHint}>
          Track aus der Library hierher ziehen oder am DDJ-200 Shift + Kopfhörer-Cue drücken.
        </p>
      </div>
    </section>
  );
}
