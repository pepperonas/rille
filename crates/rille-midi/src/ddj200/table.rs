//! The DDJ-200 MIDI message table, transcribed from Pioneer's "DDJ-200 List of MIDI messages".
//! This file is the only place where raw status/data bytes of the controller appear.
//!
//! Channels (0-based in the status nibble):
//! - deck 1 / deck 2 controls: 0 / 1
//! - mixer and effect controls (crossfader, colour FX, master cue, transition FX): 6. The PDF's
//!   channel table says "EFFECT = 5", but every message row uses channel 7 (`x6`); the rows win.
//! - performance pads: 7 (deck 1), 8 (deck 1 + shift), 9 (deck 2), 10 (deck 2 + shift)
//! - "loaded" LEDs: 15

use std::sync::LazyLock;

use rille_core::DeckId;

pub const NOTE_ON: u8 = 0x90;
pub const NOTE_OFF: u8 = 0x80;
pub const CONTROL_CHANGE: u8 = 0xB0;

pub const CH_DECK_1: u8 = 0;
pub const CH_DECK_2: u8 = 1;
pub const CH_MIXER: u8 = 6;
pub const CH_LOADED: u8 = 15;

pub const fn deck_channel(deck: DeckId) -> u8 {
    match deck {
        DeckId::A => CH_DECK_1,
        DeckId::B => CH_DECK_2,
    }
}

pub const fn pad_channel(deck: DeckId, shifted: bool) -> u8 {
    match (deck, shifted) {
        (DeckId::A, false) => 7,
        (DeckId::A, true) => 8,
        (DeckId::B, false) => 9,
        (DeckId::B, true) => 10,
    }
}

/// A physical element of the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Control {
    // per deck
    Play,
    Cue,
    Shift,
    /// BEAT SYNC short press (and shift + press, which sends its own note).
    Sync,
    /// BEAT SYNC held: the controller sends a separate note.
    SyncLong,
    TempoFader,
    JogTouch,
    /// Outer ring of the jog wheel (pitch bend). Same message with and without shift.
    JogRim,
    /// Jog platter while vinyl mode is on.
    JogPlatterVinyl,
    /// Jog platter while vinyl mode is off.
    JogPlatter,
    /// Jog platter with shift (fast search).
    JogPlatterShift,
    EqHi,
    EqMid,
    EqLow,
    ChannelFader,
    /// Shift + fader moved from zero to not-zero.
    FaderStartPlay,
    /// Shift + fader moved back to zero.
    FaderStartCue,
    HeadphoneCue,
    /// Colour FX knob of the channel (lives on the mixer channel, one per deck).
    ColorFx,
    Pad(u8),
    // global
    Crossfader,
    MasterCue,
    TransitionFx,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputKind {
    /// Note on/off: pressed / released.
    Button,
    /// 14-bit absolute value; this entry is the MSB, `lsb` the matching LSB controller.
    Absolute14 { lsb: u8 },
    /// Relative, centred on 64.
    Relative,
}

/// Which deck a control belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Deck(DeckId),
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputSpec {
    /// `NOTE_ON` or `CONTROL_CHANGE` (upper nibble only).
    pub message: u8,
    pub channel: u8,
    pub data1: u8,
    pub kind: InputKind,
    pub scope: Scope,
    pub control: Control,
    /// `Some(s)`: the message itself says whether shift was held. `None`: the controller sends
    /// the same message with and without shift; use the tracked shift state.
    pub shift: Option<bool>,
}

/// An LED the app can drive. LEDs answer to the same message their button sends.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Led {
    Play { shifted: bool },
    Cue { shifted: bool },
    Sync { shifted: bool },
    HeadphoneCue { shifted: bool },
    Pad { index: u8, shifted: bool },
    Loaded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GlobalLed {
    MasterCue { shifted: bool },
    TransitionFx { shifted: bool },
}

/// Deck row: (message, data1, kind, control, shift).
type DeckRow = (u8, u8, InputKind, Control, Option<bool>);
/// Mixer row: (message, data1, kind, scope, control, shift).
type MixerRow = (u8, u8, InputKind, Scope, Control, Option<bool>);

const DECK_ROWS: &[DeckRow] = &[
    (
        CONTROL_CHANGE,
        0x22,
        InputKind::Relative,
        Control::JogPlatterVinyl,
        Some(false),
    ),
    (
        CONTROL_CHANGE,
        0x23,
        InputKind::Relative,
        Control::JogPlatter,
        Some(false),
    ),
    (
        CONTROL_CHANGE,
        0x29,
        InputKind::Relative,
        Control::JogPlatterShift,
        Some(true),
    ),
    (
        NOTE_ON,
        0x36,
        InputKind::Button,
        Control::JogTouch,
        Some(false),
    ),
    (
        NOTE_ON,
        0x67,
        InputKind::Button,
        Control::JogTouch,
        Some(true),
    ),
    (
        CONTROL_CHANGE,
        0x21,
        InputKind::Relative,
        Control::JogRim,
        None,
    ),
    (NOTE_ON, 0x58, InputKind::Button, Control::Sync, Some(false)),
    (
        NOTE_ON,
        0x5C,
        InputKind::Button,
        Control::SyncLong,
        Some(false),
    ),
    (NOTE_ON, 0x60, InputKind::Button, Control::Sync, Some(true)),
    (
        CONTROL_CHANGE,
        0x00,
        InputKind::Absolute14 { lsb: 0x20 },
        Control::TempoFader,
        None,
    ),
    (NOTE_ON, 0x0B, InputKind::Button, Control::Play, Some(false)),
    (NOTE_ON, 0x47, InputKind::Button, Control::Play, Some(true)),
    (NOTE_ON, 0x0C, InputKind::Button, Control::Cue, Some(false)),
    (NOTE_ON, 0x48, InputKind::Button, Control::Cue, Some(true)),
    (
        NOTE_ON,
        0x3F,
        InputKind::Button,
        Control::Shift,
        Some(false),
    ),
    (
        CONTROL_CHANGE,
        0x07,
        InputKind::Absolute14 { lsb: 0x27 },
        Control::EqHi,
        None,
    ),
    (
        CONTROL_CHANGE,
        0x0B,
        InputKind::Absolute14 { lsb: 0x2B },
        Control::EqMid,
        None,
    ),
    (
        CONTROL_CHANGE,
        0x0F,
        InputKind::Absolute14 { lsb: 0x2F },
        Control::EqLow,
        None,
    ),
    (
        NOTE_ON,
        0x54,
        InputKind::Button,
        Control::HeadphoneCue,
        Some(false),
    ),
    (
        NOTE_ON,
        0x68,
        InputKind::Button,
        Control::HeadphoneCue,
        Some(true),
    ),
    (
        CONTROL_CHANGE,
        0x13,
        InputKind::Absolute14 { lsb: 0x33 },
        Control::ChannelFader,
        None,
    ),
    (
        NOTE_ON,
        0x66,
        InputKind::Button,
        Control::FaderStartPlay,
        Some(true),
    ),
    (
        NOTE_ON,
        0x52,
        InputKind::Button,
        Control::FaderStartCue,
        Some(true),
    ),
];

/// Mixer/effect rows on channel 7.
const MIXER_ROWS: &[MixerRow] = &[
    (
        CONTROL_CHANGE,
        0x17,
        InputKind::Absolute14 { lsb: 0x37 },
        Scope::Deck(DeckId::A),
        Control::ColorFx,
        None,
    ),
    (
        CONTROL_CHANGE,
        0x18,
        InputKind::Absolute14 { lsb: 0x38 },
        Scope::Deck(DeckId::B),
        Control::ColorFx,
        None,
    ),
    (
        CONTROL_CHANGE,
        0x1F,
        InputKind::Absolute14 { lsb: 0x3F },
        Scope::Global,
        Control::Crossfader,
        None,
    ),
    (
        NOTE_ON,
        0x63,
        InputKind::Button,
        Scope::Global,
        Control::MasterCue,
        Some(false),
    ),
    (
        NOTE_ON,
        0x78,
        InputKind::Button,
        Scope::Global,
        Control::MasterCue,
        Some(true),
    ),
    (
        NOTE_ON,
        0x59,
        InputKind::Button,
        Scope::Global,
        Control::TransitionFx,
        Some(false),
    ),
    (
        NOTE_ON,
        0x5A,
        InputKind::Button,
        Scope::Global,
        Control::TransitionFx,
        Some(true),
    ),
];

pub const PADS: u8 = 8;

static INPUTS: LazyLock<Vec<InputSpec>> = LazyLock::new(|| {
    let mut v = Vec::new();
    for deck in DeckId::ALL {
        for &(message, data1, kind, control, shift) in DECK_ROWS {
            v.push(InputSpec {
                message,
                channel: deck_channel(deck),
                data1,
                kind,
                scope: Scope::Deck(deck),
                control,
                shift,
            });
        }
        for shifted in [false, true] {
            for pad in 0..PADS {
                v.push(InputSpec {
                    message: NOTE_ON,
                    channel: pad_channel(deck, shifted),
                    data1: pad,
                    kind: InputKind::Button,
                    scope: Scope::Deck(deck),
                    control: Control::Pad(pad),
                    shift: Some(shifted),
                });
            }
        }
    }
    for &(message, data1, kind, scope, control, shift) in MIXER_ROWS {
        v.push(InputSpec {
            message,
            channel: CH_MIXER,
            data1,
            kind,
            scope,
            control,
            shift,
        });
    }
    v
});

/// Every input message the DDJ-200 can send (14-bit controls are listed once, by their MSB).
pub fn inputs() -> &'static [InputSpec] {
    &INPUTS
}

/// Status byte + data1 of a deck LED.
pub fn led_address(deck: DeckId, led: Led) -> (u8, u8) {
    let ch = deck_channel(deck);
    match led {
        Led::Play { shifted } => (NOTE_ON | ch, if shifted { 0x47 } else { 0x0B }),
        Led::Cue { shifted } => (NOTE_ON | ch, if shifted { 0x48 } else { 0x0C }),
        Led::Sync { shifted } => (NOTE_ON | ch, if shifted { 0x60 } else { 0x58 }),
        Led::HeadphoneCue { shifted } => (NOTE_ON | ch, if shifted { 0x68 } else { 0x54 }),
        Led::Pad { index, shifted } => (NOTE_ON | pad_channel(deck, shifted), index),
        Led::Loaded => (NOTE_ON | CH_LOADED, deck.index() as u8),
    }
}

pub fn global_led_address(led: GlobalLed) -> (u8, u8) {
    match led {
        GlobalLed::MasterCue { shifted } => (NOTE_ON | CH_MIXER, if shifted { 0x78 } else { 0x63 }),
        GlobalLed::TransitionFx { shifted } => {
            (NOTE_ON | CH_MIXER, if shifted { 0x5A } else { 0x59 })
        }
    }
}

/// Message that switches vinyl mode (jog platter scratches when on). Default on the device: on.
pub fn vinyl_mode_message(deck: DeckId, on: bool) -> [u8; 3] {
    [
        NOTE_ON | deck_channel(deck),
        0x17,
        if on { 0x7F } else { 0x00 },
    ]
}

pub const LED_ON: u8 = 0x7F;
pub const LED_OFF: u8 = 0x00;
