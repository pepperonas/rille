# M9 – Feinschliff und Abnahme

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

## Tastatur
Eine **Tabelle** (`src/shell/shortcuts.ts`) ist die einzige Quelle: Taste (`KeyboardEvent.code`,
damit QWERTZ und QWERTY gleich liegen; Y/Z werden gemieden), Shift-Variante, Gruppe,
Beschreibung, Aktion. Daraus entstehen der globale Handler und die `?`-Übersicht; ein Test prüft,
dass keine Taste doppelt belegt ist und die README jede Zeile nennt.

| Deck 1 | Deck 2 | Funktion |
|---|---|---|
| F | J | Play/Pause |
| D | K | Cue (halten = Vorhören) |
| S / ⇧S | L / ⇧L | Sync / Sync-Lock |
| G | H | Kopfhörer-Cue |
| 1 2 3 4 Q W E R | 7 8 9 0 U I O P | Pads 1–8 im aktuellen Modus (⇧ = löschen / Loop verlassen) |
| 5 | 6 | Pad-Modus wechseln |
| X / C | M / , | Pitch-Bend langsamer / schneller (halten) |

Mixer: V / N Crossfader schrittweise nach links / rechts (⇧ = ganz), B = Mitte.
Allgemein: `?` Übersicht, ⌘F Suche, ⌘L Trackliste, ⌘, Einstellungen, ⌘⌥M MIDI-Monitor.

Regeln: kein Kürzel, wenn das Ziel ein Textfeld ist, ein Element die Taste schon verarbeitet hat
(`defaultPrevented`, z. B. `1`/`2` in der Liste) oder ein modaler Dialog offen ist;
Tastenwiederholung löst nichts erneut aus; ⌘/Ctrl/Alt nur bei den Einträgen, die sie verlangen.
Gehaltene Tasten (Cue, Bend) lassen beim Loslassen los – auch wenn das Fenster den Fokus verliert.

## Einstellungen und Theme
- Abschnitt **Darstellung & Wiedergabe**: Theme (Dunkel / Hell / System), Tempo-Bereich beim
  Start (gilt sofort für beide Decks), Crossfader-Kurve (dieselbe wie im Mixer). Gespeichert in
  `localStorage` (`rille.prefs`), Parser verträgt kaputte und alte Daten (pure, getestet).
- Theme setzt `data-theme` auf `<html>` und im Tauri-Fenster `setTheme` (sonst zieht die
  Vibrancy das falsche Grau); „System“ folgt `prefers-color-scheme` live.
- Light Theme: Tokens existieren (Generator mit Kontrasttest); Komponenten auf feste Annahmen
  prüfen, Screenshot in beiden Themes.

## Leerzustände
- Kein Audio-Gerät: Hinweisleiste über den Decks mit „Ausgabe wählen“ (öffnet Einstellungen).
- Kein Controller: Hinweis in der Statuspille und in den Einstellungen (vorhanden), dazu im
  leeren Deck der Tastatur-Hinweis „? zeigt alle Tasten“.
- Keine Tracks: vorhanden.

## Performance
`docs/performance.md`: jedes Budget mit Messweg, Datum, Maschine und Zahl. Neu gemessen:
Frontend-Bildzeit mit zwei spielenden Decks (Playwright, rAF-Intervalle), Ladezeit, Engine-Last,
Last während der Analyse, Speicher je Track-Minute.

## Tests
- Vitest: Tabelle eindeutig, Handler-Filter (Textfeld, Dialog, Wiederholung, Modifier),
  Prefs-Parser, Theme-Auflösung.
- Playwright: Kürzel steuern Play/Cue/Pads/Crossfader, `?`-Übersicht, Theme-Wechsel (hell
  gemessen), Leerzustand ohne Gerät, Einstellungen bleiben nach Reload.

## Bekannte Grenzen (bleiben)
- ~~Nahtlose Keylock-Loops~~ – nachgezogen nach M9: der Stretcher wird mit dem Loop als
  durchgehendem Strom gefüttert, statt an jeder Naht neu anzulaufen.
- ~~Sync im Stillstand nur Tempo~~ – nachgezogen nach M9: ein im Stillstand gesynctes oder
  gelocktes Deck übernimmt die Phase beim Start (`align_on_start`, höchstens ½ Beat, nie aus
  einem Loop heraus); Vorhören am Cue verbraucht den einmaligen Sync nicht.
