# M8 – Kopfhörer-Routing

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

Der DDJ-200 hat kein Audio-Interface. Vorhören braucht deshalb entweder ein Gerät mit vier
Ausgängen (1/2 Master, 3/4 Kopfhörer) oder ein zweites Gerät (z. B. USB-Kopfhörer, Kopfhörer-
buchse des Mac neben einem externen Interface).

## Engine
- **Cue-Bus:** je Kanal `HeadphoneCue(deck, on)`; abgegriffen nach EQ/Filter und Trim, vor
  Kanalfader und Crossfader (wie am Pult). `MasterCue(on)` legt den Master zusätzlich auf den
  Bus. Ein- und Ausschalten über Glätter (kein Knacken).
- **Kopfhörer-Signal:** `hp = level · ((1 − mix) · cue + mix · master)`, `CueMix` 0 = nur Cue,
  1 = nur Master; `CueLevel` 0…1 (−∞ … 0 dB, Fader-Kennlinie). Beides geglättet.
- **Routing** (`CueRoute`): `Off`, `Channels34` (gleiches Gerät, Kanäle 3/4) oder `Ring`
  (zweites Gerät). Für `Ring` schreibt die Engine je Block in einen lockfreien Ringpuffer
  (`rtrb`, beim Start alloziert); volle Puffer verwerfen, nie blockieren.
- Snapshot: `cue[2]`, `master_cue`, `cue_mix`, `cue_level`, `cue_peak`.

## Zweites Gerät
- Eigener Stream mit eigenem Takt. Sein Callback liest den Ring über einen **Drift-Ausgleich**:
  Resampler (kubisch) mit Rate `1 + Korrektur`, die ein PI-Regler aus dem Füllstand berechnet
  (Ziel: zwei Blöcke Vorlauf, Korrektur gedeckelt auf ±0,5 %). Leer → Stille und neu
  vorfüllen, bevor wieder gespielt wird (kein Stottern im Wechsel).
- Der Ring-Leser liegt in einem eigenen Slot (wie `EngineSlot`), damit er Stream-Wechsel
  überlebt. Fällt das Cue-Gerät aus, läuft der Master unberührt weiter; der Cue-Stream wird
  wie der Master neu aufgebaut (oder bleibt aus, wenn das Gerät weg ist).
- Gleiche Rate wie die Engine; sonst klare Fehlermeldung.

## Controller
- Kopfhörer-Cue-Tasten (ohne Shift) schalten den Cue des Kanals, Master Cue den Master auf den
  Bus. LEDs zeigen den Zustand.

## App
- Mixer: Kopfhörer-Taste je Kanal, Master-Cue, Regler „Cue/Master“ und „Kopfhörer“.
- Einstellungen: Kopfhörer-Ausgang = Aus / Kanäle 3–4 des Ausgabegeräts (wenn vorhanden) /
  anderes Gerät (+ Puffer). Status zeigt Gerät, Latenz und Aussetzer.

## Tests
- Engine: Cue-Bus vor dem Fader (Fader zu → Cue hörbar), Master-Cue, Mix-Endstellungen, Level,
  Kanäle 3/4 bei 4-Kanal-Ausgabe, Ring wird nur bei `Ring` gefüllt, voller Ring blockiert nicht,
  alles unter `assert_no_alloc`.
- Drift: simulierter Erzeuger/Verbraucher mit ±200 ppm über Minuten: Füllstand stabil, keine
  Aussetzer nach dem Einschwingen, Korrektur konvergiert auf die Drift.
- Ausfall: Fehler des Cue-Streams berührt den Master nicht (Service-Test mit Fake-Fehler).
- Controller: Cue-Tasten → Befehle, LEDs.
- Frontend: Mixer-Bedienung, Einstellungen (Playwright gegen Mock).
