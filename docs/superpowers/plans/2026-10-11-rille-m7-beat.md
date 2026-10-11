# M7 – Beat-Funktionen und Pads

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

## Engine (rille-core / rille-engine)

**Grid im Deck.** `TrackLoad` bringt Grid und Hot Cues mit (beides `Copy`), `SetGrid` setzt es
später (Analyse fertig, Grid korrigiert). Grid in Sekunden der Quelle; die Engine rechnet mit
`sample_rate` in Frames um. Beat-Position: `beat(pos) = (pos/sr − first_beat) · bpm / 60`.

**Sync (einmalig).** Ziel ist das andere Deck (geladen, mit Grid). Effektive BPM =
`bpm · tempo_rate` (ohne Bend/Scratch). Faktor ×½/×1/×2, der am nächsten an 1 liegt (174 ↔ 87).
Tempo-Fader = `(rate − 1) / span`; passt es nicht in den Bereich, wird der kleinste passende
Bereich gewählt, sonst nichts geändert. Spielt das Ziel, wird die Phase ausgerichtet: Sprung um
die Beat-Differenz (auf ±½ Beat gefaltet) mit der üblichen 4-ms-Überblendung.

**Sync-Lock.** Je Block: Tempo folgt dem Ziel; Phasenfehler wird über einen kleinen
Raten-Zuschlag (wie Bend, gedeckelt auf ±2 %) ausgeglichen, nie per Sprung. Während Bend,
Scratch oder Suchlauf ruht die Korrektur. Beide Decks gelockt: das zuletzt gelockte folgt dem
anderen (Leader bleibt frei). Ziel muss spielen.

**Loops.** Loop = `[start, end)` in Frames. Beat-Loop startet auf dem Beat (Raster der
Loop-Länge, höchstens 1 Beat) am oder vor dem Abspielkopf – oder auf dem nächsten, wenn er
weniger als ⅛ Schritt entfernt ist (ohne Grid: kein Beat-Loop). Länge ¼ … 32 Beats; gleiche
Länge noch mal = verlassen, andere Länge = Länge ändern (Start bleibt). Ein Loop, der das
Trackende erreichen würde, wird nicht angelegt bzw. nicht verlängert. Nur das Überschreiten des
Loop-Endes springt zurück; ein Sprung aus dem Loop heraus (Cue, Hot Cue, Seek) verlässt ihn. Beim Rendern wird der Block an der Loop-Grenze geteilt;
der Rücksprung ist ein normaler Sprung mit Überblendung (Keylock: Player blendet selbst).
Beat Jump verschiebt einen aktiven Loop mit.

**Beat Jump.** ±½, ±1, ±4, ±16 Beats; ohne Grid nichts.

**Hot Cues.** 8 je Deck, in Frames. Gedrückt: gesetzt → springen (Wiedergabezustand bleibt),
leer → an der aktuellen Position setzen (mit Grid: auf den nächsten Beat quantisiert, wenn er
näher als ⅛ Beat liegt). Löschen per Shift. Snapshot trägt die Cues; der Bridge-Thread speichert
Änderungen in der Library (Schema v3, Tabelle `hot_cues(track_id, slot, seconds)`).

Snapshot je Deck zusätzlich: `grid`, `hot_cues`, `loop`, `sync_lock`.

## Controller
- Sync kurz = einmalig, lang = Lock an/aus; Sync-LED zeigt Lock.
- Pads je nach Pad-Modus (in der App umschaltbar, je Deck): Hot Cue (Shift = löschen),
  Beat Loop (¼ … 32, Shift = Loop verlassen), Beat Jump (oben zurück ½/1/4/16, unten vor).
- Pad-LEDs: leer aus, belegt an, aktiv blinkt im Beat-Takt (aktiver Loop; Hot Cue nicht).
- Ändert die Engine das Tempo (Sync), bekommt der Tempo-Fader neuen Soft-Takeover-Wert.

## App
- Je Deck: SYNC + LOCK, Pad-Feld 8×1 (passt auch in kurze Fenster) mit Modus-Chips,
  Loop-Anzeige; Hot-Cue-Marken und Loop-Bereich in Übersicht und Detail.
- Grid-Korrektur: Grid ±10 ms verschieben, BPM ×2 / ÷2, BPM ±0,01, Tap (pure Funktion,
  Ausreißer verworfen). Speichert in der Library und schickt `SetGrid` an Decks mit dem Track.

## Tests
- Engine: Sync-Faktor (½/2), Bereichswahl, Phasenausrichtung, Lock hält Phase bei
  unterschiedlichen BPM über 30 s, Loop wiederholt exakt (Länge, kein Klick an der Naht), Loop
  über das Trackende, Beat Jump mit Loop, Hot Cue setzen/springen/löschen/quantisieren, alles
  unter `assert_no_alloc`.
- Controller: Pad-Modi → Aktionen, Pad-LEDs inkl. Blinken, Sync-LED.
- Library: Schema v3, Hot Cues speichern/laden, Grid-Korrektur.
- Frontend: Tap-Tempo, Pad-Belegung je Modus (Vitest); Playwright: Hot Cue setzen/springen,
  Loop an/aus, Beat Jump, Sync, Grid-Korrektur.

## Bekannte Grenzen
- Mit Keylock beginnt jeder Loop-Durchlauf mit der Varispeed-Brücke (wie nach jedem Sprung),
  bis der Stretcher wieder eingeschwungen ist; bei Tempo ≠ 0 ist dieser Anfang kurz
  tonhöhenverschoben. Ein nahtloser Keylock-Loop bräuchte einen Sprung, der den Stretcher
  weiterlaufen lässt – offen für M9.
- Sync im Stand gleicht nur das Tempo an (das Ziel läuft bis zum Play ohnehin weiter);
  die Phase kommt über Sync-Lock, sobald beide spielen.
