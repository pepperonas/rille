# Regeln: Frontend und Material 3 Expressive

Selbstpflegend: neue Muster, die sich bewähren, hier ergänzen.

## Tokens
- `src/design/tokens.css` ist die einzige Quelle für Shape, Spacing, Typo, Motion.
  Farben kommen aus `src/design/color.generated.css`, erzeugt von `scripts/gen-tokens.mjs`
  (`pnpm tokens`) – **nie von Hand editieren**; ein Test prüft den Gleichstand.
- Springs für `motion` stehen in `src/design/tokens.ts` (`springs.*`). Keine eigenen
  stiffness/damping-Werte in Komponenten.
- Komponenten-CSS (CSS Modules) nutzt nur `var(--…)`. Keine Hex-Werte, keine px-Abstände außer
  Hairlines (1px) und Icon-Größen.

## Farbe
- Dark ist Default (`:root`), Light über `data-theme="light"` auf `<html>`.
- Jedes Deck setzt `--accent`, `--on-accent`, `--accent-container`, `--on-accent-container` auf
  seiner Wurzel (`[data-deck='a'|'b']`); Kinder verwenden nur diese Aliase. So zieht sich die
  Deckfarbe automatisch durch Waveform, Pads, Fader und Pegel.
- Text auf Akzentflächen immer mit dem passenden `on-*`-Token. Die Generator-Tests sichern
  ≥ 4,5:1 für Text und ≥ 3:1 für Akzente auf der Fläche.

## Typo
- Roboto Flex Variable, lokal gebündelt (`@fontsource-variable/roboto-flex`, OFL), kein CDN.
- Live-Zahlen (BPM, Zeit, Pitch) bekommen `.numeric` (tabellarische Ziffern) und
  `font-stretch: var(--font-stretch-emphasized)`.

## Motion
- Nur Springs bzw. M3-Emphasized-Kurven, keine linearen Easings.
- Räumliche Bewegung: `springs.spatial*` (darf überschwingen). Farbe/Opazität: `springs.effects*`
  (überschwingt nie – Test erzwingt das).
- `prefers-reduced-motion`: CSS-Dauern fallen auf 0 (tokens.css); JS-Animationen prüfen
  `useReducedMotion()` und springen direkt zum Ziel.
- Feedback auf Eingaben innerhalb eines Frames (< 16 ms): Zustand optimistisch anzeigen.

## Fenster
- Titlebar Overlay: oberste Zeile ist `data-tauri-drag-region`, links `--titlebar-inset-left`
  frei für die nativen Ampeln. Interaktive Elemente in der Leiste dürfen kein Drag-Attribut tragen.
- Im Tauri-Fenster ist `body` transparent (Klasse `native` auf `<html>`), damit Vibrancy greift;
  Panels malen ihre eigene Fläche.

## Canvas
- Waveforms auf `<canvas>`/WebGL2, nie als DOM-Elemente. Rendering pausiert, wenn nichts spielt
  und sich nichts ändert.
