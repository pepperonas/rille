# M5 – Library und Analyse

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

## Messung vorab (entscheidet die Ladestrategie)

Fleetwood Mac – Dreams, MP3 4:18, 44,1 kHz, Release-Build:

| Schritt | Zeit |
|---|---|
| Dekodieren (44,1 kHz, ohne Resampling) | 208 ms |
| Dekodieren + Resampling auf 48 kHz | 450 ms |

Ein PCM-Cache auf Engine-Rate wäre ~100 MB pro Track – für eine Library unbrauchbar. Stattdessen
**progressives Laden**: Die Engine bekommt den Track, während er noch dekodiert wird.

- `TrackAudio` hält einen vorab allozierten Puffer (`AtomicU32` mit f32-Bits) und einen atomaren
  Zähler `ready` (Release beim Schreiben, Acquire beim Lesen). Der Loader schreibt nur hinter
  `ready`, die Engine liest nur davor; dahinter liegt Stille. Keine Locks, keine Allokation im
  Callback, auf arm64 kosten relaxte Atomic-Loads nichts.
- Länge vorab aus den Codec-Parametern (exakt bei WAV/AIFF/FLAC/ALAC/AAC, geschätzt bei MP3) plus
  Reserve; am Ende wird die tatsächliche Länge gesetzt. Unbekannte Länge → wie bisher erst
  komplett dekodieren.
- Dekodieren und Resampling laufen blockweise (rubato `FftFixedIn` pro Chunk).
- Ziel: **erste hörbare Frames < 50 ms**, gemessen; „Laden < 300 ms" damit für jeden Track erfüllt,
  ob analysiert oder nicht.

## Bausteine

| # | Baustein | Ort |
|---|---|---|
| 1 | Progressives `TrackAudio` + Streaming-Decode | `rille-core`, `rille-library/decode.rs` |
| 2 | Scan (rekursiv, Formatfilter) + Tags (Titel/Artist/Album/Dauer ohne Volldekodierung) | `rille-library/scan.rs`, `tags.rs` |
| 3 | BPM + konstantes Raster mit Phase | `rille-library/analysis/bpm.rs` |
| 4 | 3-Band-Peaks in Stufen, Binärformat | `rille-library/analysis/peaks.rs` |
| 5 | SQLite (rusqlite, bundled), Migrationen über `user_version` | `rille-library/db.rs` |
| 6 | Library-Dienst: Import, Analyse-Warteschlange (N = Kerne − 2, QoS utility), Fortschritt, Laden per Track-ID | `src-tauri/src/library_service.rs` |
| 7 | Frontend: virtualisierte Liste, Suche, Sortierung, Fortschritt, Ziehen aufs Deck, Tastatur, Controller | `src/library/` |

### BPM
Mono, Onset-Hüllkurve per spektralem Fluss (log-komprimiert, Fenster 1024, Hop 256), Tempogramm
per Autokorrelation 60–200 BPM mit Oktav-Prior (log-normal um 120, damit 174 nicht zu 87 und 90
nicht zu 180 wird), Verfeinerung per Kammfilter über den ganzen Track (0,01 BPM), Phase mit
Sub-Hop-Interpolation. Ergebnis `None` („BPM unbekannt") bei Stille, Rauschen, < 4 Beats oder
geringer Konfidenz – nie ein Fantasiewert.

### Peaks (Format v1)
```
"RLPK" · u16 version=1 · u8 bands=3 · u8 levels · u32 sample_rate · u32 frames_per_bin (Stufe 0)
je Stufe: u32 bins · bins × [low, mid, high] als u8 (Spitzenwert, Wurzel-skaliert)
```
Stufe k fasst `frames_per_bin × 4^k` Frames zusammen. Bänder per Butterworth bei 200 Hz / 2 kHz.
Datei pro Track unter `~/Library/Caches/io.celox.rille/peaks/<track-id>.bin` (M6 zeichnet daraus).

### DB-Schema v1
`tracks(id, path UNIQUE, size, mtime, title, artist, album, duration, bpm, beat_offset, analysis
['pending'|'done'|'failed'], error, added_at)`, `roots(path)`. Erneuter Import: Upsert über `path`;
geänderte Größe/mtime → Analyse wieder `pending`. WAL-Modus.

### Controller
Shift+Jog-Rand → `library-scroll` (Ticks im Dienst zu Zeilen gebündelt; Teiler ist eine Annahme bis
zur Hardware-Abnahme), Shift+Kopfhörer-Cue → `library-load` für das Deck. Auswahl lebt im Frontend.

### Ziehen aufs Deck
Zeigerbasiert (Pointer Events), nicht HTML5-DnD: funktioniert neben dem nativen Finder-Drop und
ist mit Playwright testbar.

## Tests
- Progressives Laden: Engine liest während des Schreibens nur Veröffentlichtes; Länge wird
  korrigiert; Zeit bis zum ersten Frame gemessen.
- BPM auf synthetischen Tracks: Klicks bei 90/120/128/174 BPM, mit Swing (geshuffelte Hi-Hats), mit
  Pause (8 Takte Stille), mit Rauschen; ±0,05 BPM, Phase ±5 ms. Stille/Rauschen/zu kurz → `None`.
- Peaks: Format-Roundtrip, Band-Trennung (tiefer Ton nur im Low-Band), abgeschnittene Datei abgelehnt.
- DB: frisch anlegen, Migration von v0, zweites Öffnen idempotent, unbekannte zukünftige Version
  → klare Meldung, Upsert setzt Analyse bei geänderter Datei zurück.
- Kaputte/leere Dateien: Analyse `failed` mit Meldung, Worker läuft weiter.
- Lasttest: Analyse auf allen Workern parallel zum Rendern zweier Decks, kein Block über Budget.
- Frontend: Suche/Sortierung (Vitest), Liste, Laden, Ziehen, Tastatur, Fortschritt (Playwright).
- Plausibilitätslauf auf echten Tracks (`examples/analyze`), Ergebnisse im README.
