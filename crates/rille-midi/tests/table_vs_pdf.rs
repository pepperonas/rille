#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code
//! Every row of "DDJ-200 List of MIDI messages", typed in independently of `table.rs`.
//! If the table drifts from the PDF, this test fails.

use std::collections::HashSet;

use rille_midi::DeckId;
use rille_midi::ddj200::GlobalLed;
use rille_midi::ddj200::table::{
    Control, InputKind, Led, Scope, global_led_address, inputs, led_address, vinyl_mode_message,
};

/// (status byte, data1, control, shift from message, scope)
fn pdf_rows() -> Vec<(u8, u8, Control, Option<bool>, Scope)> {
    let mut rows = Vec::new();
    for (n, deck) in [(0u8, DeckId::A), (1u8, DeckId::B)] {
        let d = Scope::Deck(deck);
        let b = 0xB0 | n;
        let note = 0x90 | n;
        rows.extend([
            (b, 0x22, Control::JogPlatterVinyl, Some(false), d), // JOG platter, vinyl on
            (b, 0x23, Control::JogPlatter, Some(false), d),      // JOG platter, vinyl off
            (b, 0x29, Control::JogPlatterShift, Some(true), d),  // JOG platter + SHIFT
            (note, 0x36, Control::JogTouch, Some(false), d),     // JOG touch
            (note, 0x67, Control::JogTouch, Some(true), d),      // JOG touch + SHIFT
            (b, 0x21, Control::JogRim, None, d),                 // JOG wheel side (± SHIFT)
            (note, 0x58, Control::Sync, Some(false), d),         // BEAT SYNC press
            (note, 0x5C, Control::SyncLong, Some(false), d),     // BEAT SYNC long press
            (note, 0x60, Control::Sync, Some(true), d),          // BEAT SYNC + SHIFT
            (b, 0x00, Control::TempoFader, None, d),             // Tempo MSB (± SHIFT)
            (note, 0x0B, Control::Play, Some(false), d),         // PLAY/PAUSE
            (note, 0x47, Control::Play, Some(true), d),          // PLAY/PAUSE + SHIFT
            (note, 0x0C, Control::Cue, Some(false), d),          // CUE
            (note, 0x48, Control::Cue, Some(true), d),           // CUE + SHIFT
            (note, 0x3F, Control::Shift, Some(false), d),        // SHIFT
            (b, 0x07, Control::EqHi, None, d),                   // EQ HI MSB
            (b, 0x0B, Control::EqMid, None, d),                  // EQ MID MSB
            (b, 0x0F, Control::EqLow, None, d),                  // EQ LOW MSB
            (note, 0x54, Control::HeadphoneCue, Some(false), d), // CUE (headphones)
            (note, 0x68, Control::HeadphoneCue, Some(true), d),  // CUE (headphones) + SHIFT
            (b, 0x13, Control::ChannelFader, None, d),           // CH FADER MSB
            (note, 0x66, Control::FaderStartPlay, Some(true), d), // fader start: zero -> not zero
            (note, 0x52, Control::FaderStartCue, Some(true), d), // fader start: not zero -> zero
        ]);
        let (pad, pad_shift) = if n == 0 { (0x97, 0x98) } else { (0x99, 0x9A) };
        for i in 0..8 {
            rows.push((pad, i, Control::Pad(i), Some(false), d));
            rows.push((pad_shift, i, Control::Pad(i), Some(true), d));
        }
    }
    rows.extend([
        (0xB6, 0x17, Control::ColorFx, None, Scope::Deck(DeckId::A)), // COLOR FX CH1 MSB
        (0xB6, 0x18, Control::ColorFx, None, Scope::Deck(DeckId::B)), // COLOR FX CH2 MSB
        (0xB6, 0x1F, Control::Crossfader, None, Scope::Global),       // CROSSFADER MSB
        (0x96, 0x63, Control::MasterCue, Some(false), Scope::Global), // MASTER CUE
        (0x96, 0x78, Control::MasterCue, Some(true), Scope::Global),  // MASTER CUE + SHIFT
        (
            0x96,
            0x59,
            Control::TransitionFx,
            Some(false),
            Scope::Global,
        ), // TRANSITION FX
        (0x96, 0x5A, Control::TransitionFx, Some(true), Scope::Global), // TRANSITION FX + SHIFT
    ]);
    rows
}

fn status(spec: &rille_midi::ddj200::InputSpec) -> u8 {
    spec.message | spec.channel
}

#[test]
fn every_pdf_row_is_in_the_table() {
    for (st, d1, control, shift, scope) in pdf_rows() {
        let found = inputs()
            .iter()
            .find(|s| status(s) == st && s.data1 == d1)
            .unwrap_or_else(|| panic!("missing {st:02X} {d1:02X} ({control:?})"));
        assert_eq!(
            (found.control, found.shift, found.scope),
            (control, shift, scope),
            "{st:02X} {d1:02X}"
        );
    }
}

#[test]
fn the_table_has_nothing_the_pdf_lacks() {
    assert_eq!(inputs().len(), pdf_rows().len());
}

#[test]
fn fourteen_bit_pairs_match_the_pdf() {
    let lsb = |st: u8, msb: u8| {
        inputs()
            .iter()
            .find(|s| status(s) == st && s.data1 == msb)
            .map(|s| s.kind)
            .unwrap()
    };
    for n in 0..2u8 {
        let b = 0xB0 | n;
        assert_eq!(lsb(b, 0x00), InputKind::Absolute14 { lsb: 0x20 }); // tempo
        assert_eq!(lsb(b, 0x07), InputKind::Absolute14 { lsb: 0x27 }); // EQ hi
        assert_eq!(lsb(b, 0x0B), InputKind::Absolute14 { lsb: 0x2B }); // EQ mid
        assert_eq!(lsb(b, 0x0F), InputKind::Absolute14 { lsb: 0x2F }); // EQ low
        assert_eq!(lsb(b, 0x13), InputKind::Absolute14 { lsb: 0x33 }); // channel fader
    }
    assert_eq!(lsb(0xB6, 0x17), InputKind::Absolute14 { lsb: 0x37 });
    assert_eq!(lsb(0xB6, 0x18), InputKind::Absolute14 { lsb: 0x38 });
    assert_eq!(lsb(0xB6, 0x1F), InputKind::Absolute14 { lsb: 0x3F });
}

#[test]
fn no_message_is_used_twice_including_lsbs() {
    let mut seen = HashSet::new();
    for s in inputs() {
        assert!(
            seen.insert((status(s), s.data1)),
            "duplicate {:02X} {:02X}",
            status(s),
            s.data1
        );
        if let InputKind::Absolute14 { lsb } = s.kind {
            assert!(
                seen.insert((status(s), lsb)),
                "LSB clash {:02X} {lsb:02X}",
                status(s)
            );
        }
    }
}

#[test]
fn leds_use_the_button_messages() {
    assert_eq!(
        led_address(DeckId::A, Led::Play { shifted: false }),
        (0x90, 0x0B)
    );
    assert_eq!(
        led_address(DeckId::B, Led::Cue { shifted: true }),
        (0x91, 0x48)
    );
    assert_eq!(
        led_address(DeckId::A, Led::Sync { shifted: false }),
        (0x90, 0x58)
    );
    assert_eq!(
        led_address(DeckId::B, Led::HeadphoneCue { shifted: false }),
        (0x91, 0x54)
    );
    assert_eq!(
        led_address(
            DeckId::A,
            Led::Pad {
                index: 3,
                shifted: true
            }
        ),
        (0x98, 0x03)
    );
    assert_eq!(
        led_address(
            DeckId::B,
            Led::Pad {
                index: 7,
                shifted: false
            }
        ),
        (0x99, 0x07)
    );
    assert_eq!(led_address(DeckId::A, Led::Loaded), (0x9F, 0x00));
    assert_eq!(led_address(DeckId::B, Led::Loaded), (0x9F, 0x01));
    assert_eq!(
        global_led_address(GlobalLed::MasterCue { shifted: false }),
        (0x96, 0x63)
    );
    assert_eq!(
        global_led_address(GlobalLed::TransitionFx { shifted: true }),
        (0x96, 0x5A)
    );
}

#[test]
fn vinyl_mode_message_matches_pdf() {
    assert_eq!(vinyl_mode_message(DeckId::A, false), [0x90, 0x17, 0x00]);
    assert_eq!(vinyl_mode_message(DeckId::B, true), [0x91, 0x17, 0x7F]);
}
