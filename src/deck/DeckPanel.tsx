import { backend } from '../ipc';
import type { Deck } from '../ipc/types';
import { loadIntoDeck, unloadDeck } from '../state/connect';
import { useAppState } from '../state/store';
import { trackById } from '../library/model';
import { Detail } from '../waveform/Detail';
import { Overview } from '../waveform/Overview';
import { usePeaks } from '../waveform/usePeaks';
import { TimeReadout } from './TimeReadout';
import { TempoButtons, TempoFader } from './Tempo';
import { Transport } from './Transport';
import styles from './DeckPanel.module.css';

interface Props {
  deck: Deck;
}

/** One deck: track info, time, transport. Accepts files dropped from the Finder. */
export function DeckPanel({ deck }: Props) {
  const label = deck === 'a' ? 'Deck 1' : 'Deck 2';
  const info = useAppState((s) => s.decks[deck]);
  const libraryId = info.status === 'ready' ? info.libraryId : null;
  // The library has the latest analysis (tempo, grid) – it may finish after loading.
  const libTrack = useAppState((s) => trackById(s.library.tracks, libraryId));
  const peaks = usePeaks(libraryId);
  const bpm = libTrack?.bpm ?? (info.status === 'ready' ? info.bpm : null);

  return (
    <section className={styles.deck} data-deck={deck} aria-label={label}>
      <header className={styles.header}>
        <span className={styles.badge}>{deck === 'a' ? '1' : '2'}</span>
        <span className={styles.label}>{label}</span>
        {info.status === 'ready' && (
          <button
            type="button"
            className={styles.eject}
            aria-label={`${label} auswerfen`}
            onClick={() => unloadDeck(deck)}
          >
            <svg viewBox="0 0 24 24" width="20" height="20" aria-hidden="true">
              <path fill="currentColor" d="M5 17h14v2H5v-2Zm7-12 7 10H5l7-10Z" />
            </svg>
          </button>
        )}
      </header>

      {info.status === 'ready' ? (
        <div className={styles.loaded}>
          <div className={styles.main}>
            <div>
              <h2 className={styles.title} title={info.title}>
                {info.title}
              </h2>
              {(info.artist || bpm !== null) && (
                <p className={styles.meta}>
                  {info.artist && <span>{info.artist}</span>}
                  {bpm !== null && (
                    <span className={`${styles.bpm} numeric`} aria-label="Track-Tempo">
                      {bpm.toFixed(1)} BPM
                    </span>
                  )}
                </p>
              )}
              {info.warning && (
                <p className={styles.warning} role="alert">
                  {info.warning}
                </p>
              )}
            </div>
            <Detail
              deck={deck}
              peaks={peaks}
              bpm={libTrack?.bpm ?? null}
              firstBeat={libTrack?.firstBeat ?? null}
            />
            <TimeReadout deck={deck} />
            <Overview deck={deck} peaks={peaks} />
            <div className={styles.controls}>
              <Transport deck={deck} />
              <TempoButtons deck={deck} />
            </div>
          </div>
          <TempoFader deck={deck} />
        </div>
      ) : (
        <div className={styles.empty} data-status={info.status}>
          {info.status === 'loading' && (
            <>
              <p className={styles.emptyTitle}>Lädt …</p>
              <p className={styles.emptyHint}>{info.title}</p>
            </>
          )}
          {info.status === 'error' && (
            <>
              <p className={styles.emptyTitle}>„{info.title}“ lässt sich nicht laden</p>
              <p className={styles.emptyHint}>{info.message}</p>
            </>
          )}
          {info.status === 'empty' && (
            <>
              <p className={styles.emptyTitle}>Kein Track geladen</p>
              <p className={styles.emptyHint}>
                Audiodatei aus dem Finder hierher ziehen. Unterstützt: MP3, AAC/M4A, ALAC, FLAC,
                WAV, AIFF.
              </p>
            </>
          )}
          {backend.kind === 'mock' && info.status !== 'loading' && (
            <button
              type="button"
              className={styles.demo}
              onClick={() => void loadIntoDeck(deck, `/demo/Demo Track ${label}.wav`)}
            >
              Demo-Track laden
            </button>
          )}
        </div>
      )}
    </section>
  );
}
