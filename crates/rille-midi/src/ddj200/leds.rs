//! LED feedback: desired lamp state → the minimal set of MIDI messages to get there.

use std::collections::HashMap;

use rille_core::DeckId;

use super::table::{GlobalLed, LED_OFF, LED_ON, Led, PADS, global_led_address, led_address};

/// A lamp is on, off, or blinking (on/off with the shared blink phase).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lamp {
    #[default]
    Off,
    On,
    Blink,
}

impl Lamp {
    pub fn from_bool(on: bool) -> Lamp {
        if on { Lamp::On } else { Lamp::Off }
    }

    fn lit(self, blink_on: bool) -> bool {
        match self {
            Lamp::Off => false,
            Lamp::On => true,
            Lamp::Blink => blink_on,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeckLamps {
    pub play: Lamp,
    pub cue: Lamp,
    pub sync: Lamp,
    pub headphone_cue: Lamp,
    pub pads: [Lamp; PADS as usize],
    pub loaded: Lamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LampState {
    pub decks: [DeckLamps; 2],
    pub master_cue: Lamp,
    pub transition_fx: Lamp,
}

/// Remembers what the controller currently shows and emits only changes.
#[derive(Debug, Default)]
pub struct LedWriter {
    shown: HashMap<(u8, u8), u8>,
}

impl LedWriter {
    pub fn new() -> LedWriter {
        LedWriter::default()
    }

    /// Forget what the controller shows (after connect): the next `update` sends everything.
    pub fn invalidate(&mut self) {
        self.shown.clear();
    }

    /// Append the messages needed to show `state` to `out`. The shift layer mirrors the normal
    /// layer, so the lamps do not change when shift is pressed.
    pub fn update(&mut self, state: &LampState, blink_on: bool, out: &mut Vec<[u8; 3]>) {
        for deck in DeckId::ALL {
            let lamps = &state.decks[deck.index()];
            for shifted in [false, true] {
                self.set(
                    out,
                    led_address(deck, Led::Play { shifted }),
                    lamps.play.lit(blink_on),
                );
                self.set(
                    out,
                    led_address(deck, Led::Cue { shifted }),
                    lamps.cue.lit(blink_on),
                );
                self.set(
                    out,
                    led_address(deck, Led::Sync { shifted }),
                    lamps.sync.lit(blink_on),
                );
                let hp = lamps.headphone_cue.lit(blink_on);
                self.set(out, led_address(deck, Led::HeadphoneCue { shifted }), hp);
                for (index, pad) in lamps.pads.iter().enumerate() {
                    let address = led_address(
                        deck,
                        Led::Pad {
                            index: index as u8,
                            shifted,
                        },
                    );
                    self.set(out, address, pad.lit(blink_on));
                }
            }
            self.set(
                out,
                led_address(deck, Led::Loaded),
                lamps.loaded.lit(blink_on),
            );
        }
        for shifted in [false, true] {
            let mc = state.master_cue.lit(blink_on);
            self.set(
                out,
                global_led_address(GlobalLed::MasterCue { shifted }),
                mc,
            );
            let fx = state.transition_fx.lit(blink_on);
            self.set(
                out,
                global_led_address(GlobalLed::TransitionFx { shifted }),
                fx,
            );
        }
    }

    fn set(&mut self, out: &mut Vec<[u8; 3]>, (status, data1): (u8, u8), on: bool) {
        let value = if on { LED_ON } else { LED_OFF };
        if self.shown.get(&(status, data1)) != Some(&value) {
            self.shown.insert((status, data1), value);
            out.push([status, data1, value]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lamps_on_play_a() -> LampState {
        let mut s = LampState::default();
        s.decks[0].play = Lamp::On;
        s
    }

    #[test]
    fn first_update_sends_the_full_state() {
        let mut w = LedWriter::new();
        let mut out = Vec::new();
        w.update(&LampState::default(), true, &mut out);
        // per deck: 2 layers × (4 buttons + 8 pads) + loaded = 25; global: 2 × 2 = 4
        assert_eq!(out.len(), 2 * 25 + 4);
        assert!(out.iter().all(|m| m[2] == LED_OFF));
    }

    #[test]
    fn later_updates_send_only_changes() {
        let mut w = LedWriter::new();
        let mut out = Vec::new();
        w.update(&LampState::default(), true, &mut out);
        out.clear();
        w.update(&lamps_on_play_a(), true, &mut out);
        assert_eq!(out, [[0x90, 0x0B, 0x7F], [0x90, 0x47, 0x7F]]);
        out.clear();
        w.update(&lamps_on_play_a(), true, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn blinking_follows_the_phase() {
        let mut w = LedWriter::new();
        let mut s = LampState::default();
        s.decks[1].pads[2] = Lamp::Blink;
        let mut out = Vec::new();
        w.update(&s, true, &mut out);
        out.clear();
        w.update(&s, false, &mut out);
        assert_eq!(out, [[0x99, 0x02, 0x00], [0x9A, 0x02, 0x00]]);
    }

    #[test]
    fn invalidate_resends_everything() {
        let mut w = LedWriter::new();
        let mut out = Vec::new();
        w.update(&lamps_on_play_a(), true, &mut out);
        let full = out.len();
        out.clear();
        w.invalidate();
        w.update(&lamps_on_play_a(), true, &mut out);
        assert_eq!(out.len(), full);
    }

    #[test]
    fn loaded_lamp_uses_channel_16() {
        let mut w = LedWriter::new();
        let mut s = LampState::default();
        s.decks[1].loaded = Lamp::On;
        let mut out = Vec::new();
        w.update(&s, true, &mut out);
        assert!(out.contains(&[0x9F, 0x01, 0x7F]));
    }
}
