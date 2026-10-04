# M4 – Tempo, Keylock, Pitch-Bend, Scratch, Reverse

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

**Risiko vorab geklärt:** Signalsmith Stretch alloziert in `process()`, `reset()` und `seek()`
nicht – gemessen über den macOS-`malloc_logger`-Haken, der auch C++-Allokationen sieht
(`assert_no_alloc` sieht sie nicht). Test: `crates/rille-engine/tests/stretch_alloc.rs`.

## Modell
- **Abspielkopf** je Deck als `f64` (`playhead`); `Transport.position` (u64) wird nach jedem
  Block daraus abgeleitet. Sprünge aus dem Transport (Cue, Seek) setzen den Abspielkopf neu.
- **Rate** pro Sample geglättet: normal `basis × (1 + bend)` mit Basis = `1 + tempo × bereich`
  (vorwärts) bzw. negativ bei Reverse; beim Scratchen die Teller-Geschwindigkeit.
- **Varispeed** (Tonhöhe folgt dem Tempo): 4-Punkt-Hermite-Interpolation auf dem Track.
- **Keylock** (Tonhöhe bleibt): Signalsmith Stretch je Deck, gefüttert mit `rate × frames`
  Eingangsframes pro Block; Latenz wird durch Vorlauf der Fütterung ausgeglichen; nach Sprüngen
  `reset` + `seek` (Pre-Roll). Gilt nur vorwärts spielend ohne Scratch.
- **Tempo-Bereiche:** ±6 / ±10 / ±16 % / Wide (±50 %). Fader −1…+1, oben = langsamer.
- **Pitch-Bend:** Jog-Rand-Ticks pro Sekunde → Ratenabweichung (geglättet, klingt ab); Buttons
  in der App halten ±4 %.
- **Scratch** (Vinyl-Modus, Jog berührt): Rate = Teller-Ticks/s ÷ (Ticks pro Umdrehung ×
  33⅓ U/min). Loslassen kehrt zur Spielrate zurück (bzw. Stillstand, wenn pausiert).
  `JOG_TICKS_PER_REV` ist eine Annahme und wird bei der Hardware-Abnahme kalibriert.
- **Suchlauf** (Shift + Teller): springt pro Tick 50 ms.
- **Reverse:** solange gehalten rückwärts mit Spieltempo; am Trackanfang Stopp.

## Tests
Tempo-Mapping inkl. Bereiche und Invertierung; Varispeed verschiebt Tonhöhe (Nulldurchgänge);
Keylock hält Tonhöhe bei ±16 % (±1 %) und Laufzeit stimmt; Reverse läuft rückwärts und stoppt
bei 0; Scratch folgt den Ticks, Halten = Stillstand, Loslassen = zurück zur Rate; Bend klingt ab;
Suchlauf; Sprünge mit Keylock ohne lange Stille; Allokationsfreiheit (Rust + C++).

## Controller/UI
DDJ-200: Tempo-Fader (Soft-Takeover), Jog-Rand = Bend, Teller = Scratch (nur Vinyl-Modus),
Shift+Teller = Suchlauf, Shift+Play = Reverse solange gehalten, Shift+Sync = Bereich wechseln.
Deck-UI: Tempo-Fader, Tempo in %, Bereichs-Chip, KEY-Schalter, Bend-Tasten, REV (halten).
