# Regeln: Controller-Mapping (DDJ-200)

Einzige Quelle für Messages: `docs/ddj-200_midi_message_list.pdf`. Kein Code aus Mixxx (GPL);
das Mixxx-Mapping darf nur zum Verständnis gelesen werden.

## Struktur
- Mapping als **deklarative Tabelle** (`crates/rille-midi/src/mapping/ddj200.rs`):
  `(status, data1, shift_kontext) → Action`. Keine verstreuten `match`-Arme pro Taste.
- LED-Ausgabe ebenfalls als Tabelle (`Feedback → (status, data1)`), diff-basiert gesendet; beim
  Verbinden wird der komplette LED-Zustand neu geschickt.
- Jede Tabellenzeile hat einen Test (eingehend → Action, Zustand → LED-Message).

## DDJ-200-Besonderheiten (aus dem PDF)
- **14 Bit:** Fader/Regler senden MSB (CC x) und LSB (CC x+0x20) getrennt. Wert =
  `(msb << 7) | lsb`, übernommen beim Eintreffen des LSB. Nie nur MSB auswerten.
- **Tempo-Fader:** „−"-Seite (oben) = Min (0), „+"-Seite (unten) = Max. Oben langsamer, unten schneller.
- **Jog:** relativ, Delta = Wert − 64 (> 64 im Uhrzeigersinn). Teller `Bn 22` (Vinyl an),
  `Bn 23` (Vinyl aus), mit Shift `Bn 29`. Rand `Bn 21` – **mit und ohne Shift identisch**.
- **Shift (`9n 3F`)** selbst tracken: Rand, Tempo-Fader und (vermutlich) Kanalfader senden mit
  Shift dieselbe Message wie ohne.
- **Vinyl-Modus:** Default an, abschalten nur per `[0x9n, 0x17, 0x00]`. Beim Verbinden setzen.
- **LEDs:** dieselbe Message wie der Button, Data 2 `0x7F` an / `0x00` aus. Keine LED für Shift,
  Jog, Regler und für Beat-Sync-lang (`9n 5C`) – Sync-Lock wird über die LED von `9n 58` gezeigt.
  Loaded-LED: `9F 00 7F` (Deck 1), `9F 01 7F` (Deck 2).
- **Pad-LEDs** kennen nur an/aus: leer = aus, belegt = an, aktiv = blinkt im Beat-Takt.
- **Kanäle:** Mixer/Effekt-Elemente (Crossfader, CFX, Master Cue, Transition FX) senden laut
  Message-Zeilen auf Kanal 7 (Status `x6`), obwohl die Kanaltabelle des PDFs „EFFECT = 5"
  nennt. Die Message-Zeilen gelten; am Gerät mit dem MIDI-Monitor bestätigen.
- **Soft-Takeover** für alle absoluten Regler (nach Connect, Deck-Wechsel, Zustandsänderung aus
  der App): Wert erst übernehmen, wenn der Hardware-Regler den Software-Wert erreicht.

## Hardware-Status
Offen am Gerät zu prüfen: Kanal 7 vs. 5, Loaded-LED aus mit `0x00`, sendet der Kanalfader mit
Shift weiterhin `Bn 13/33`, löst der Crossfader Fader Start aus.
