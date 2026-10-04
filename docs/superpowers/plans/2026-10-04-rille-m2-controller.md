# M2 – Controller DDJ-200

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD je Task, Review am Ende,
> danach **Hardware-Abnahme 1** mit dem Nutzer am echten Gerät.

**Ziel:** Der DDJ-200 steuert alles, was die Engine heute kann (Play/Pause, Cue, Shift+Cue,
Kanalfader, Crossfader, Fader Start), sendet LED-Rückmeldung, übersteht An- und Abstecken, und
alle übrigen Bedienelemente kommen bereits sauber dekodiert als Aktionen an, damit spätere
Meilensteine nur noch die Wirkung anschließen. Ohne Hardware entwickelbar über einen virtuellen
DDJ-200; ein MIDI-Monitor zeigt alles live.

**Architektur:** Drei Schichten, jede für sich testbar.
1. `decode`: Rohbytes → `ControlEvent` (welches Element, Wert, Shift-Kontext). Zustandsbehaftet
   nur für 14-Bit-Paare und Shift. Kennt nur die **Tabelle** aus dem PDF.
2. `mapping`: `ControlEvent` → `ControllerAction` (semantisch: „Deck A Cue gedrückt“,
   „Library eins runter“). Funktionsbelegung als Tabelle, später austauschbar.
3. `leds`: gewünschter LED-Zustand (aus Engine-Snapshot) → Diff → MIDI-Out.
`src-tauri` übersetzt `ControllerAction` in Engine-Commands (über die MIDI-Queue der Engine,
eigener Produzent) bzw. UI-Events.

## Review Focus (aus dem Masterplan, M2 zuständig)
- Controller wird mitten in einer Bewegung abgezogen und wieder angesteckt: kein Pegelsprung
  (Soft-Takeover nach Reconnect), LEDs komplett neu gesendet. Tests in `takeover` und
  `ControllerSession::on_connect`.

## Dateien
```
crates/rille-midi/src/
  lib.rs
  message.rs        // Rohbytes ↔ MidiMessage
  ddj200/mod.rs
  ddj200/table.rs   // INPUTS + OUTPUTS: die Tabelle aus dem PDF, einzige Quelle
  ddj200/decode.rs  // 14 Bit, Jog, Shift, Tempo-Invertierung → ControlEvent
  ddj200/mapping.rs // ControlEvent → ControllerAction (Default-Funktionsbelegung)
  ddj200/leds.rs    // LedState → Messages (Diff, Vollabgleich, Blinken)
  takeover.rs       // Soft-Takeover
  monitor.rs        // Ringpuffer für den MIDI-Monitor
  port.rs           // midir, Hotplug-Polling, Vinyl-Modus beim Connect
  virtual_ddj.rs    // virtueller DDJ-200 (CoreMIDI-Virtual-Ports) für Tests/Entwicklung
crates/rille-midi/tests/{table_vs_pdf.rs, virtual_roundtrip.rs}
src-tauri/src/controller_service.rs
src/dev/MidiMonitor.tsx, src/shell/ControllerChip.tsx, Settings „Controller“
```

## Kerntypen
```rust
pub enum Control { Play, Cue, Shift, Sync, SyncLong, TempoFader, JogTouch, JogRim, JogPlatter,
                   EqHi, EqMid, EqLow, ChannelFader, FaderStartPlay, FaderStartCue,
                   HeadphoneCue, Cfx, Pad(u8), // je Deck
                   Crossfader, MasterCue, TransitionFx }        // global
pub struct ControlEvent { pub deck: Option<DeckId>, pub control: Control, pub shifted: bool,
                          pub value: ControlValue }
pub enum ControlValue { Button(bool), Absolute(f32 /*0..1, 14 Bit*/), Relative(i32), }
pub enum ControllerAction { /* PlayPause(deck), Cue{deck,pressed}, JumpToStart(deck), Reverse{..},
   ChannelFader(deck,f32), Crossfader(f32), Tempo(deck,f32), PitchBend(deck,i32),
   Scratch(deck,i32), ScratchTouch{deck,bool}, Search(deck,i32), LibraryScroll(i32),
   LoadSelected(deck), HeadphoneCue(deck), Eq{deck,band,f32}, Filter(deck,f32), Sync(deck),
   SyncLock(deck), CycleTempoRange(deck), Pad{deck,index,pressed,shifted}, MasterCue,
   TransitionFx, CycleTransitionFx, FaderStart{deck,play} */ }
```

## Tasks
1. **Tabelle** (`table.rs`): alle Zeilen des PDFs als Konstanten, inkl. Kanal 7 (`x6`) für
   Mixer/Effekt und 16 (`F`) für Loaded-LED. Test `table_vs_pdf.rs`: jede PDF-Zeile (von Hand
   abgeschrieben im Test, unabhängig von der Tabelle) wird gefunden, keine Doppelbelegung von
   (Status, Data1) außer den dokumentierten (Jog-Rand mit/ohne Shift).
2. **Decoder**: 14 Bit (MSB puffern, beim LSB übernehmen; LSB ohne vorheriges MSB nutzt das
   zuletzt bekannte MSB; Note-Off als Status `8n` und als `9n` mit Velocity 0), Jog-Delta
   `Wert − 64`, Tempo-Invertierung (oben langsamer), Shift-Tracking je Deck plus „irgendein
   Shift“ für globale Elemente. Tests: MSB/LSB-Reihenfolgen, Grenzwerte 0/16383, Jog ±, Shift
   gedrückt während Fader-Bewegung, Shift-Release mitten im Jog.
3. **Funktionsbelegung** (`mapping.rs`) laut Prompt; Tests je Zeile der Belegungstabelle.
4. **Soft-Takeover**: Wert übernehmen, sobald Hardware den Software-Wert kreuzt oder auf
   ±2 % herankommt; Reset bei Connect, Deck-/Zustandsänderung aus der App. Tests inkl.
   Reconnect-Szenario aus dem Review Focus.
5. **LEDs**: `LedState` (Play, Cue, Sync, Kopfhörer-Cue, Master Cue, Transition FX, Pads,
   Loaded, Vinyl) → Diff-Messages, Vollabgleich, Blinktakt aus einer Phase. Tests.
6. **Port/Hotplug/Virtuell**: Polling 1 s, Port-Name enthält „DDJ-200“; Vinyl-Modus-Message
   beim Connect; virtueller DDJ-200 für einen Roundtrip-Integrationstest (`virtual_roundtrip.rs`:
   Note an virtuellen Port → Aktion kommt an; LED-Ausgabe kommt am virtuellen Eingang an).
7. **Monitor**: Ringpuffer 512 Messages (Zeit, Richtung, Bytes, dekodierte Bedeutung), nur bei
   offener Ansicht an das Frontend gestreamt.
8. **App**: `ControllerService` (eigener Thread, MIDI-Queue der Engine), Status-Chip in der
   Titelleiste, Einstellungen „Controller“ (Vinyl-Modus), Dev-Ansicht MIDI-Monitor
   (Tastenkürzel `⌘⌥M`), README-Belegungstabelle.
9. **Abschluss** + Hardware-Abnahme 1 (Kanal 7 statt 5, Loaded-LED aus, Kanalfader-CC mit
   Shift, Fader Start am Crossfader).
