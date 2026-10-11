import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import type { Deck, LibraryTrack } from '../ipc/types';
import { sizes } from '../design/tokens';
import { useAppState } from '../state/store';
import {
  chooseFolder,
  loadSelected,
  loadTrack,
  moveSelected,
  selectTrack,
  setQuery,
  sortBy,
} from './actions';
import { formatBpm, formatDuration, progressText, visibleTracks, type SortKey } from './model';
import { useTrackDrag } from './useTrackDrag';
import styles from './LibraryPanel.module.css';

/** Rows rendered beyond the visible ones, so fast scrolling shows no gaps. */
const OVERSCAN = 8;

const COLUMNS: Array<{ key: SortKey; label: string; className: string }> = [
  { key: 'title', label: 'Titel', className: styles.colTitle ?? '' },
  { key: 'artist', label: 'Artist', className: styles.colArtist ?? '' },
  { key: 'bpm', label: 'BPM', className: styles.colBpm ?? '' },
  { key: 'duration', label: 'Dauer', className: styles.colTime ?? '' },
];

/** Track library: search, sort, a virtualised list, loading by keyboard, double-click,
 *  dragging onto a deck and the controller. */
export function LibraryPanel() {
  const library = useAppState((s) => s.library);
  const rows = visibleTracks(library);
  const status = progressText(library.progress);
  const { drag, start } = useTrackDrag((deck, track) => {
    selectTrack(track.id);
    void loadTrack(deck, track);
  });

  return (
    <section className={styles.library} aria-label="Library" data-library="">
      <header className={styles.toolbar}>
        <h2 className={styles.heading}>Library</h2>
        {library.tracks.length > 0 && (
          <span className={`${styles.count} numeric`}>
            {rows.length === library.tracks.length
              ? `${library.tracks.length} Tracks`
              : `${rows.length} von ${library.tracks.length}`}
          </span>
        )}
        {status && (
          <span className={styles.progress} role="status">
            <span className={styles.spinner} aria-hidden="true" />
            <span className="numeric">{status}</span>
          </span>
        )}
        <span className={styles.spacer} />
        <input
          className={styles.search}
          type="search"
          placeholder="Suchen: Titel, Artist, Album, BPM"
          aria-label="Library durchsuchen"
          value={library.query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'ArrowDown' || e.key === 'Enter') {
              e.preventDefault();
              document.getElementById('library-list')?.focus();
              if (e.key === 'ArrowDown') moveSelected(1);
              else loadSelected();
            }
          }}
        />
        <button type="button" className={styles.import} onClick={chooseFolder}>
          Ordner importieren
        </button>
      </header>

      {library.tracks.length === 0 ? (
        <div className={styles.empty}>
          <p className={styles.emptyTitle}>
            {library.ready ? 'Noch keine Tracks' : 'Library wird geladen …'}
          </p>
          <p className={styles.emptyHint}>
            Importiere einen Ordner mit Musik oder zieh ihn aus dem Finder hierher. rille analysiert
            BPM und Beatgrid im Hintergrund.
          </p>
        </div>
      ) : (
        <>
          <div className={styles.columns} role="presentation">
            <span className={styles.colStatus} />
            {COLUMNS.map((c) => (
              <button
                key={c.key}
                type="button"
                className={`${styles.sort} ${c.className}`}
                aria-sort={
                  library.sort.key === c.key
                    ? library.sort.dir === 'asc'
                      ? 'ascending'
                      : 'descending'
                    : undefined
                }
                aria-label={`Nach ${c.label} sortieren`}
                onClick={() => sortBy(c.key)}
              >
                {c.label}
                {library.sort.key === c.key && (
                  <span aria-hidden="true">{library.sort.dir === 'asc' ? ' ↑' : ' ↓'}</span>
                )}
              </button>
            ))}
          </div>
          {rows.length === 0 ? (
            <p className={styles.noMatch}>Keine Treffer für „{library.query}“</p>
          ) : (
            <TrackList rows={rows} selected={library.selected} onDragStart={start} />
          )}
        </>
      )}

      {drag &&
        createPortal(
          <div
            className={styles.ghost}
            style={{ transform: `translate(${drag.x}px, ${drag.y}px)` }}
            data-over={drag.over ?? undefined}
            aria-hidden="true"
          >
            {drag.track.title}
            {drag.over && <span> → Deck {drag.over === 'a' ? 1 : 2}</span>}
          </div>,
          document.body,
        )}
    </section>
  );
}

interface ListProps {
  rows: LibraryTrack[];
  selected: number | null;
  onDragStart: (e: React.PointerEvent, track: LibraryTrack) => void;
}

function TrackList({ rows, selected, onDragStart }: ListProps) {
  const ref = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [height, setHeight] = useState(0);
  const rowHeight = sizes.libraryRow;

  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const observer = new ResizeObserver(() => setHeight(el.clientHeight));
    observer.observe(el);
    setHeight(el.clientHeight);
    return () => observer.disconnect();
  }, []);

  // Keep the selected row in view (keyboard, controller, search).
  const selectedIndex = rows.findIndex((t) => t.id === selected);
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el || selectedIndex < 0) return;
    const top = selectedIndex * rowHeight;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (top + rowHeight > el.scrollTop + el.clientHeight) {
      el.scrollTop = top + rowHeight - el.clientHeight;
    }
  }, [selectedIndex, rowHeight]);

  const page = Math.max(1, Math.floor(height / rowHeight) - 1);
  const first = Math.max(0, Math.floor(scrollTop / rowHeight) - OVERSCAN);
  const last = Math.min(rows.length, Math.ceil((scrollTop + height) / rowHeight) + OVERSCAN);

  const onKeyDown = (e: React.KeyboardEvent) => {
    const keys: Record<string, () => void> = {
      ArrowDown: () => moveSelected(1),
      ArrowUp: () => moveSelected(-1),
      PageDown: () => moveSelected(page),
      PageUp: () => moveSelected(-page),
      Home: () => moveSelected(-rows.length),
      End: () => moveSelected(rows.length),
      Enter: () => loadSelected(),
      '1': () => loadSelected('a' as Deck),
      '2': () => loadSelected('b' as Deck),
    };
    const action = keys[e.key];
    if (!action || e.metaKey || e.ctrlKey || e.altKey) return;
    e.preventDefault();
    action();
  };

  return (
    <div
      ref={ref}
      id="library-list"
      className={styles.list}
      role="listbox"
      aria-label="Tracks"
      tabIndex={0}
      aria-activedescendant={selectedIndex >= 0 ? `track-${selected}` : undefined}
      onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
      onKeyDown={onKeyDown}
    >
      <div className={styles.canvas} style={{ height: rows.length * rowHeight }}>
        {rows.slice(first, last).map((t, i) => (
          <Row
            key={t.id}
            track={t}
            top={(first + i) * rowHeight}
            selected={t.id === selected}
            onDragStart={onDragStart}
          />
        ))}
      </div>
    </div>
  );
}

interface RowProps {
  track: LibraryTrack;
  top: number;
  selected: boolean;
  onDragStart: (e: React.PointerEvent, track: LibraryTrack) => void;
}

function Row({ track, top, selected, onDragStart }: RowProps) {
  return (
    <div
      id={`track-${track.id}`}
      className={styles.row}
      role="option"
      aria-selected={selected}
      style={{ transform: `translateY(${top}px)` }}
      title={track.error ?? track.path}
      onPointerDown={(e) => {
        selectTrack(track.id);
        onDragStart(e, track);
      }}
      onDoubleClick={() => {
        selectTrack(track.id);
        loadSelected();
      }}
    >
      <span className={styles.colStatus}>
        {track.analysis === 'pending' && (
          <span className={styles.pending} aria-label="Wird analysiert" role="img" />
        )}
        {track.analysis === 'failed' && (
          <span className={styles.failed} aria-label="Analyse fehlgeschlagen" role="img">
            !
          </span>
        )}
      </span>
      <span className={styles.colTitle}>{track.title}</span>
      <span className={styles.colArtist}>{track.artist ?? ''}</span>
      <span className={`${styles.colBpm} numeric`}>
        {track.analysis === 'pending' ? '' : formatBpm(track.bpm)}
      </span>
      <span className={`${styles.colTime} numeric`}>{formatDuration(track.duration)}</span>
    </div>
  );
}
