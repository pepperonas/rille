# Regeln: Audio-Thread und Engine

Gilt für `crates/rille-engine` und alles, was im Audio-Callback läuft.

## Der Callback ist heilig
- Im Audio-Callback (`Engine::process` und alles, was es aufruft): **keine** Allokation oder
  Freigabe (`Vec::push` über Kapazität, `Box::new`, `String`, `format!`, `Arc` droppen, wenn es
  das letzte ist), **keine** Locks (`Mutex`, `RwLock`, Channels mit Locks), **kein** Logging
  (`tracing`, `println!`), **kein** I/O, **keine** Syscalls, kein `thread::sleep`.
- Debug-Builds verpacken den Callback in `assert_no_alloc`. Jede neue DSP-Funktion bekommt einen
  Test, der sie unter `assert_no_alloc` laufen lässt.
- Puffer werden beim Konfigurieren dimensioniert (größte Blockgröße), nie im Callback.
- Track-Audio kommt als `Arc<TrackAudio>`. Ein ersetzter Buffer geht über die Rückgabe-Queue
  aus dem Callback heraus und wird von der Bridge freigegeben – nie im Callback droppen.

## Kommunikation
- UI → Engine: `rtrb`-Queue (UI-Thread als einziger Produzent).
- MIDI → Engine: eigene `rtrb`-Queue (MIDI-Thread als einziger Produzent).
- Engine → außen: Triple Buffer für den Snapshot (neuester Stand gewinnt), `rtrb` für Ereignisse,
  die nicht verloren gehen dürfen (Xrun, Track-Ende, Fehler).
- `Command` und `Snapshot` sind `Copy` und enthalten keine Heap-Daten.
- Volle Queue: Befehl verwerfen und zählen (Snapshot-Feld), niemals blockieren.

## DSP
- Alle Benutzerparameter (Fader, EQ, Filter, Tempo, Crossfader) laufen durch einen Glätter, sonst
  entstehen Zipper-Geräusche. Glättung zeitbasiert (ms), nicht pro Callback.
- Denormals vermeiden (Flush-to-zero setzen bzw. kleine DC-Offsets in Rückkopplungen).
- Engine-Code ist ohne Gerät testbar: `Engine::process(&mut out, channels)` rendert offline.
- Zielwerte: 256 Frames bei 48 kHz, Controller → hörbar < 10 ms (ohne Gerätelatenz).

## Fehler
- Kein `unwrap()`/`expect()`/`panic!` außerhalb von Tests (Workspace-Lints erzwingen das).
- Gerätefehler (abgezogen, Format geändert) kommen als Ereignis aus dem cpal-Error-Callback; die
  Bridge baut den Stream neu auf, der Engine-Zustand bleibt erhalten.
