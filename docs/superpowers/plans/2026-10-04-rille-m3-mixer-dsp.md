# M3 – Mixer-DSP und Effekte

> Teilplan zu `2026-10-04-rille-masterplan.md`. Ausführung inline, TDD, Review am Ende.

**Ziel:** 3-Band-EQ mit Kill, bipolares Filter pro Kanal und Transition-FX (Echo-Out, Filter-Out)
– hörbar sauber, ohne Klicks, allokationsfrei, am DDJ-200 und in der App bedienbar.

**Signalfluss pro Deck:** Track → Declick → **EQ** → **Filter** → Trim · Kanalfader · Crossfader
→ (Transition-FX: Dry-Ausblendung + Echo-Send) → Master.

## DSP-Entscheidungen
- **EQ = Isolator** mit Linkwitz-Riley-Weichen 4. Ordnung (je zwei kaskadierte
  Butterworth-Biquads) bei **200 Hz** und **2 kHz**. Tiefband bekommt einen Allpass bei 2 kHz,
  damit die drei Bänder bei Mittelstellung zur reinen Allpass-Antwort summieren (flacher
  Frequenzgang). Knopf 0 → Kill (−∞), 0,5 → 0 dB, 1 → +6 dB; Bandgains je Sample geglättet.
  Kill-Taste je Band in der App (der DDJ-200 hat keine).
- **Filter = TPT-State-Variable-Filter** (Zavalishin/Simper), stabil bei schneller
  Modulation. Knopf 0…0,5: Tiefpass 20 kHz → 80 Hz (exponentiell), 0,5…1: Hochpass 20 Hz →
  8 kHz. Mitte ±3 %: Bypass; Übergang Bypass↔Filter über einen geglätteten Mix, damit das
  Einrasten nicht klickt. Resonanz fest (Q 0,9), keine Selbstoszillation.
- **Transition-FX** auf das *aktive* Deck = das Deck mit dem höheren Anteil am Master
  (Kanalfader × Crossfader), bei Gleichstand das zuletzt gestartete. Effekte:
  - **Echo-Out:** Dry blendet in ~150 ms aus, das Signal läuft in ein Delay
    (1/2 Beat; ohne bekanntes Tempo 375 ms) mit Rückkopplung 0,55; nach dem Ausklingen wird
    das Deck pausiert und Dry wiederhergestellt (der Fader kann danach gefahrlos zurück).
  - **Filter-Out:** Hochpass fährt in 2 s von 20 Hz auf 8 kHz, danach Pause + Reset.
  Zweiter Druck während des Effekts bricht ab (Dry zurück). Shift + Transition FX schaltet
  den Effekt durch.
- Alle Puffer (Delay 2 s pro Deck) beim Engine-Start reserviert.

## Tests
- Frequenzgang an Stützstellen (Sinus messen): flach ±0,5 dB bei Mittelstellung; Kill dämpft
  sein Band um ≥ 40 dB in Bandmitte; +6 dB in Bandmitte ±0,5 dB.
- Filter: Mitte bitgenau neutral (nach Einschwingen); LP bei Knopf 0,1 dämpft 5 kHz um
  ≥ 20 dB; HP bei 0,9 dämpft 100 Hz um ≥ 20 dB; schnelles Durchdrehen ohne NaN/Inf, ohne
  Sprung > Schwelle.
- Echo-Out: nach Auslösen fällt Dry in ≤ 200 ms auf Stille, das Echo ist danach noch hörbar
  und klingt ab, Deck pausiert am Ende; Abbruch stellt Dry wieder her.
- Keine Denormals (Stille nach Signal → Werte exakt 0 oder ≥ 1e-30).
- Allokationsfreiheit: bestehender `no_alloc`-Test deckt EQ/Filter/FX mit ab.

## Controller/UI
- DDJ-200: EQ Hi/Mid/Low, CFX → Filter, Transition FX (+ Shift) über `commands_for`;
  Soft-Takeover gilt bereits für EQ und CFX; Transition-FX-LED leuchtet während des Effekts.
- Mixer-UI: drei EQ-Drehregler + Kill pro Kanal, Filter-Drehregler (bipolar), Transition-FX-
  Taste mit Effektname. Neue Komponente `Knob` (Pointer vertikal ziehen, Tastatur, Doppelklick
  = Mitte), Wert als Bogen in Deckfarbe.
