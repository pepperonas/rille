# M6 – Waveforms und Deck-Oberfläche

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

## Datenfluss

1. **Jeder geladene Track bekommt Peaks.** Ein aus dem Finder geladener Track wird dabei in die
   Library übernommen (`LibraryService::ensure`): Zeile anlegen, Analyse **vorn** in die
   Warteschlange (das Deck wartet darauf). Ein Library-Track, dessen Analyse noch aussteht,
   rückt ebenfalls nach vorn.
2. `DeckLoaded` trägt `libraryId`. Das Frontend holt die Peaks per `library_peaks(id)` als
   Binärdaten (`tauri::ipc::Response`), sobald die Library den Track als `done` meldet.
3. `LibraryTrack` trägt zusätzlich `firstBeat` – zusammen mit `bpm` das Beatgrid.
4. Zeitachse: Deck-Position in Engine-Frames → Sekunden → Peak-Bins (`peakRate / framesPerBin`).
   Peaks liegen in der Quell-Rate, die Engine spielt resampelt; über Sekunden passt beides.

## Darstellung

- **Übersicht** (Canvas2D) ersetzt die Fortschrittslinie: ganzer Track, drei Bänder in
  Deckfarben-Abstufung, gespielter Teil abgedunkelt, Cue-Marke, Klick = Seek. Das Bild wird nur
  bei neuen Peaks oder Größenänderung gezeichnet; die Abspielposition ist ein CSS-Overlay.
- **Detail** (WebGL2, ein Draw-Call): Fullscreen-Quad, der Fragment-Shader liest je Pixel die
  Bins aus einer Textur der passenden Zoomstufe (max über ≤ 8 Bins), Beatgrid-Linien
  (jeder 4. Schlag kräftiger), Abspielkopf mittig als CSS-Linie. Zoom 2/4/8/16/32 s sichtbar
  (Mausrad, Tasten), gemeinsam für beide Decks.
- **Farben** aus Tokens (`--wave-low/-mid/-high` als `color-mix` der Deckfarbe); im JS über einen
  1×1-Canvas aufgelöst (Farbsyntax nie per Regex zerlegen).
- **Rendering nur bei Bedarf:** rAF-Schleife nur solange gespielt wird; im Stand ein Bild nach
  jeder Änderung (Seek, Zoom, Peaks, Größe). Zähler `window.__rilleWaveDraws` (nur Dev) für den
  Test „kein Rendern im Leerlauf“.
- Ohne WebGL2: Detail fällt auf eine Meldung zurück, Übersicht bleibt.
- Ohne Peaks (Analyse läuft): ruhiger Platzhalter „Waveform wird berechnet …“.

## Deck-UI
- Restzeit-Warnung: letzte 30 s färbt sich die Restzeit in `--md-error` und blinkt langsam
  (nicht bei Reduced Motion), die Übersicht bekommt einen Rand.
- BPM im Deck folgt der Library (wird nachgetragen, wenn die Analyse fertig ist).

## Tests
- Vitest: Peaks-Parser (Roundtrip mit Encoder, abgeschnittene Datei, falsche Magic),
  Zeit↔Bin-Mathematik, Zoomstufenwahl, Beat-Positionen im Fenster, Farbauflösung.
- Rust: `ensure` legt an und priorisiert, `library_peaks` liefert die Datei, Deck lädt mit
  `libraryId`.
- Playwright (Mock liefert synthetische Peaks): Übersicht und Detail erscheinen, Klick in die
  Übersicht seekt, Mausrad zoomt, kein Rendern im Leerlauf (Zähler steht still), Reduced Motion,
  Restzeit-Warnung.
