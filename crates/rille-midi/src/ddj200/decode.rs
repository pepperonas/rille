//! Raw MIDI bytes → [`ControlEvent`]: which element moved, to what value, with or without shift.

use std::collections::HashMap;

use rille_core::DeckId;

use super::table::{
    CONTROL_CHANGE, Control, InputKind, InputSpec, NOTE_OFF, NOTE_ON, Scope, inputs,
};

/// Largest 14-bit value.
pub const MAX_14BIT: u16 = 0x3FFF;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlValue {
    Button(bool),
    /// 14-bit absolute position, normalised to `0.0..=1.0`.
    Absolute(f32),
    /// Relative ticks; positive = clockwise.
    Relative(i32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlEvent {
    pub scope: Scope,
    pub control: Control,
    pub shifted: bool,
    pub value: ControlValue,
}

#[derive(Clone, Copy)]
enum Slot {
    Spec(&'static InputSpec),
    Lsb(&'static InputSpec),
}

/// Stateful decoder: assembles 14-bit pairs and tracks the shift buttons.
pub struct Decoder {
    by_address: HashMap<(u8, u8), Slot>,
    /// Last MSB per (status, msb controller).
    msb: HashMap<(u8, u8), u8>,
    shift: [bool; 2],
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Combine MSB and LSB into a 14-bit value.
#[inline]
pub fn combine_14bit(msb: u8, lsb: u8) -> u16 {
    (u16::from(msb & 0x7F) << 7) | u16::from(lsb & 0x7F)
}

/// Relative encoder value (centred on 64) → signed ticks.
#[inline]
pub fn relative_delta(value: u8) -> i32 {
    i32::from(value & 0x7F) - 64
}

impl Decoder {
    pub fn new() -> Decoder {
        let mut by_address = HashMap::new();
        for spec in inputs() {
            let status = spec.message | spec.channel;
            by_address.insert((status, spec.data1), Slot::Spec(spec));
            if let InputKind::Absolute14 { lsb } = spec.kind {
                by_address.insert((status, lsb), Slot::Lsb(spec));
            }
        }
        Decoder {
            by_address,
            msb: HashMap::new(),
            shift: [false; 2],
        }
    }

    pub fn shift(&self, deck: DeckId) -> bool {
        self.shift[deck.index()]
    }

    /// Forget held buttons and half-received values (controller unplugged).
    pub fn reset(&mut self) {
        self.msb.clear();
        self.shift = [false; 2];
    }

    /// Decode one MIDI message. Returns `None` for messages that are not (yet) an event: MSB
    /// halves, unknown messages, running status, system messages.
    pub fn decode(&mut self, bytes: &[u8]) -> Option<ControlEvent> {
        let (&status, rest) = bytes.split_first()?;
        let (&data1, rest) = rest.split_first()?;
        let data2 = rest.first().copied().unwrap_or(0);
        // Note off (8n) and note on with velocity 0 both mean "released".
        let (status, data2) = match status & 0xF0 {
            NOTE_OFF => (NOTE_ON | (status & 0x0F), 0),
            NOTE_ON | CONTROL_CHANGE => (status, data2),
            _ => return None,
        };
        let slot = *self.by_address.get(&(status, data1))?;
        let (spec, value) = match slot {
            Slot::Spec(spec) => match spec.kind {
                InputKind::Button => (spec, ControlValue::Button(data2 > 0)),
                InputKind::Relative => (spec, ControlValue::Relative(relative_delta(data2))),
                InputKind::Absolute14 { .. } => {
                    self.msb.insert((status, data1), data2);
                    return None;
                }
            },
            Slot::Lsb(spec) => {
                let msb = self.msb.get(&(status, spec.data1)).copied().unwrap_or(0);
                let v = combine_14bit(msb, data2);
                (
                    spec,
                    ControlValue::Absolute(f32::from(v) / f32::from(MAX_14BIT)),
                )
            }
        };

        if spec.control == Control::Shift
            && let (Scope::Deck(deck), ControlValue::Button(down)) = (spec.scope, value)
        {
            self.shift[deck.index()] = down;
        }
        let shifted = spec.shift.unwrap_or(match spec.scope {
            Scope::Deck(deck) => self.shift[deck.index()],
            Scope::Global => self.shift.iter().any(|s| *s),
        });
        Some(ControlEvent {
            scope: spec.scope,
            control: spec.control,
            shifted,
            value,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ev(d: &mut Decoder, bytes: &[u8]) -> Option<ControlEvent> {
        d.decode(bytes)
    }

    #[test]
    fn combines_fourteen_bits() {
        assert_eq!(combine_14bit(0, 0), 0);
        assert_eq!(combine_14bit(0x7F, 0x7F), MAX_14BIT);
        assert_eq!(combine_14bit(0x40, 0x00), 0x2000);
        assert_eq!(combine_14bit(0xFF, 0xFF), MAX_14BIT, "high bits ignored");
    }

    #[test]
    fn value_is_taken_when_the_lsb_arrives() {
        let mut d = Decoder::new();
        assert_eq!(
            ev(&mut d, &[0xB0, 0x13, 0x7F]),
            None,
            "MSB alone is not an event"
        );
        let e = ev(&mut d, &[0xB0, 0x33, 0x7F]).unwrap();
        assert_eq!(e.control, Control::ChannelFader);
        assert_eq!(e.scope, Scope::Deck(DeckId::A));
        assert_eq!(e.value, ControlValue::Absolute(1.0));
    }

    #[test]
    fn fine_moves_only_change_the_lsb() {
        let mut d = Decoder::new();
        ev(&mut d, &[0xB1, 0x13, 0x40]);
        let a = ev(&mut d, &[0xB1, 0x33, 0x00]).unwrap();
        let b = ev(&mut d, &[0xB1, 0x33, 0x01]).unwrap();
        let (ControlValue::Absolute(a), ControlValue::Absolute(b)) = (a.value, b.value) else {
            panic!()
        };
        assert!((b - a - 1.0 / f32::from(MAX_14BIT)).abs() < 1e-7);
    }

    #[test]
    fn lsb_without_msb_uses_zero_msb() {
        let mut d = Decoder::new();
        let e = ev(&mut d, &[0xB6, 0x3F, 0x10]).unwrap();
        assert_eq!(e.control, Control::Crossfader);
        assert_eq!(e.value, ControlValue::Absolute(16.0 / 16383.0));
    }

    #[test]
    fn msb_is_kept_per_controller() {
        let mut d = Decoder::new();
        ev(&mut d, &[0xB0, 0x07, 0x7F]); // EQ hi deck A
        ev(&mut d, &[0xB0, 0x0B, 0x00]); // EQ mid deck A
        let hi = ev(&mut d, &[0xB0, 0x27, 0x7F]).unwrap();
        assert_eq!(hi.value, ControlValue::Absolute(1.0));
        let mid = ev(&mut d, &[0xB0, 0x2B, 0x00]).unwrap();
        assert_eq!(mid.value, ControlValue::Absolute(0.0));
    }

    #[test]
    fn jog_is_relative_around_64() {
        let mut d = Decoder::new();
        assert_eq!(
            ev(&mut d, &[0xB0, 0x22, 0x41]).unwrap().value,
            ControlValue::Relative(1)
        );
        assert_eq!(
            ev(&mut d, &[0xB0, 0x22, 0x3F]).unwrap().value,
            ControlValue::Relative(-1)
        );
        assert_eq!(
            ev(&mut d, &[0xB1, 0x21, 0x48]).unwrap().value,
            ControlValue::Relative(8)
        );
        assert_eq!(
            ev(&mut d, &[0xB0, 0x23, 0x40]).unwrap().value,
            ControlValue::Relative(0)
        );
    }

    #[test]
    fn note_off_and_zero_velocity_release() {
        let mut d = Decoder::new();
        assert_eq!(
            ev(&mut d, &[0x90, 0x0B, 0x7F]).unwrap().value,
            ControlValue::Button(true)
        );
        assert_eq!(
            ev(&mut d, &[0x90, 0x0B, 0x00]).unwrap().value,
            ControlValue::Button(false)
        );
        assert_eq!(
            ev(&mut d, &[0x80, 0x0B, 0x40]).unwrap().value,
            ControlValue::Button(false)
        );
    }

    #[test]
    fn shift_is_tracked_for_messages_that_do_not_say_it() {
        let mut d = Decoder::new();
        assert!(!ev(&mut d, &[0xB0, 0x21, 0x41]).unwrap().shifted);
        ev(&mut d, &[0x90, 0x3F, 0x7F]); // shift A down
        assert!(
            ev(&mut d, &[0xB0, 0x21, 0x41]).unwrap().shifted,
            "jog rim A now shifted"
        );
        assert!(
            !ev(&mut d, &[0xB1, 0x21, 0x41]).unwrap().shifted,
            "deck B unaffected"
        );
        ev(&mut d, &[0x90, 0x3F, 0x00]); // shift A up mid-move
        assert!(!ev(&mut d, &[0xB0, 0x21, 0x41]).unwrap().shifted);
    }

    #[test]
    fn explicit_shift_messages_win_over_tracking() {
        let mut d = Decoder::new();
        assert!(
            ev(&mut d, &[0x90, 0x47, 0x7F]).unwrap().shifted,
            "shift+play note"
        );
        ev(&mut d, &[0x90, 0x3F, 0x7F]);
        assert!(
            !ev(&mut d, &[0x90, 0x0B, 0x7F]).unwrap().shifted,
            "plain play note"
        );
    }

    #[test]
    fn global_controls_see_either_shift() {
        let mut d = Decoder::new();
        ev(&mut d, &[0x91, 0x3F, 0x7F]); // shift on deck B
        ev(&mut d, &[0xB6, 0x1F, 0x40]);
        assert!(ev(&mut d, &[0xB6, 0x3F, 0x00]).unwrap().shifted);
    }

    #[test]
    fn pads_decode_with_shift_from_channel() {
        let mut d = Decoder::new();
        let e = ev(&mut d, &[0x9A, 0x05, 0x7F]).unwrap();
        assert_eq!(
            (e.scope, e.control, e.shifted),
            (Scope::Deck(DeckId::B), Control::Pad(5), true)
        );
    }

    #[test]
    fn garbage_is_ignored_without_panicking() {
        let mut d = Decoder::new();
        for bytes in [
            &[][..],
            &[0x90],
            &[0xF8],
            &[0xB0, 0x7E, 1],
            &[0xE0, 0, 0],
            &[0x95, 0x0B, 1],
        ] {
            assert_eq!(ev(&mut d, bytes), None, "{bytes:?}");
        }
    }

    #[test]
    fn reset_releases_shift() {
        let mut d = Decoder::new();
        ev(&mut d, &[0x90, 0x3F, 0x7F]);
        d.reset();
        assert!(!d.shift(DeckId::A));
    }
}
