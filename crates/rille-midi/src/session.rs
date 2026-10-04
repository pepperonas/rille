//! One connected DDJ-200: decoder, function mapping, soft takeover and LED state together.
//! No ports here, so the whole controller behaviour is testable with plain byte slices.

use std::collections::HashMap;

use rille_core::DeckId;

use crate::ddj200::table::vinyl_mode_message;
use crate::ddj200::{
    Control, ControlEvent, ControlValue, ControllerAction, Decoder, LampState, LedWriter, Mapper,
    Scope,
};
use crate::takeover::SoftTakeover;

/// An absolute control subject to soft takeover.
pub type TakeoverKey = (Scope, Control);

/// The absolute controls of the DDJ-200 (all normalised to 0..1 in hardware space).
pub fn absolute_controls() -> Vec<TakeoverKey> {
    let mut keys = vec![(Scope::Global, Control::Crossfader)];
    for deck in DeckId::ALL {
        let s = Scope::Deck(deck);
        keys.extend([
            (s, Control::ChannelFader),
            (s, Control::TempoFader),
            (s, Control::EqHi),
            (s, Control::EqMid),
            (s, Control::EqLow),
            (s, Control::ColorFx),
        ]);
    }
    keys
}

pub struct ControllerSession {
    decoder: Decoder,
    mapper: Mapper,
    takeover: HashMap<TakeoverKey, SoftTakeover>,
    leds: LedWriter,
    vinyl_mode: bool,
}

impl Default for ControllerSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ControllerSession {
    pub fn new() -> ControllerSession {
        let takeover = absolute_controls()
            .into_iter()
            .map(|k| (k, SoftTakeover::new(0.0)))
            .collect();
        ControllerSession {
            decoder: Decoder::new(),
            mapper: Mapper::new(),
            takeover,
            leds: LedWriter::new(),
            vinyl_mode: true,
        }
    }

    pub fn vinyl_mode(&self) -> bool {
        self.vinyl_mode
    }

    /// Change vinyl mode and return the messages that tell the controller.
    pub fn set_vinyl_mode(&mut self, on: bool, out: &mut Vec<[u8; 3]>) {
        self.vinyl_mode = on;
        out.extend(DeckId::ALL.map(|d| vinyl_mode_message(d, on)));
    }

    /// The controller (re)connected. `adopt_positions`: take the physical positions as they are
    /// (nothing audible yet). Otherwise every absolute control must be picked up first, so a
    /// reconnect in the middle of a set never jumps a fader.
    pub fn on_connect(&mut self, adopt_positions: bool, out: &mut Vec<[u8; 3]>) {
        self.decoder.reset();
        self.mapper.reset();
        for t in self.takeover.values_mut() {
            t.reset();
            if adopt_positions {
                t.engage();
            }
        }
        self.leds.invalidate();
        let vinyl = self.vinyl_mode;
        self.set_vinyl_mode(vinyl, out);
    }

    /// The app or engine reports the current software value of an absolute control
    /// (0..1, hardware orientation).
    pub fn sync_software(&mut self, key: TakeoverKey, value: f32) {
        if let Some(t) = self.takeover.get_mut(&key) {
            t.sync_software(value);
        }
    }

    /// Physical position of a control that has not been picked up yet (for a UI hint).
    pub fn pending_pickup(&self, key: TakeoverKey) -> Option<f32> {
        self.takeover
            .get(&key)
            .filter(|t| !t.engaged())
            .and_then(|t| t.hardware())
    }

    /// Decode, apply soft takeover, map. Also returns the decoded event for the MIDI monitor.
    pub fn on_input(&mut self, bytes: &[u8]) -> (Option<ControlEvent>, Option<ControllerAction>) {
        let Some(mut event) = self.decoder.decode(bytes) else {
            return (None, None);
        };
        if let ControlValue::Absolute(v) = event.value
            && let Some(t) = self.takeover.get_mut(&(event.scope, event.control))
        {
            match t.on_hardware(v) {
                Some(taken) => event.value = ControlValue::Absolute(taken),
                None => return (Some(event), None),
            }
        }
        (Some(event), self.mapper.map(event))
    }

    pub fn update_leds(&mut self, state: &LampState, blink_on: bool, out: &mut Vec<[u8; 3]>) {
        self.leds.update(state, blink_on, out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ddj200::Lamp;

    fn fader(session: &mut ControllerSession, deck: u8, msb: u8) -> Option<ControllerAction> {
        session.on_input(&[0xB0 | deck, 0x13, msb]);
        session.on_input(&[0xB0 | deck, 0x33, 0]).1
    }

    #[test]
    fn connect_sends_vinyl_mode_for_both_decks() {
        let mut s = ControllerSession::new();
        let mut out = Vec::new();
        s.on_connect(true, &mut out);
        assert_eq!(out, [[0x90, 0x17, 0x7F], [0x91, 0x17, 0x7F]]);
        out.clear();
        s.set_vinyl_mode(false, &mut out);
        assert_eq!(out, [[0x90, 0x17, 0x00], [0x91, 0x17, 0x00]]);
        out.clear();
        s.on_connect(true, &mut out);
        assert_eq!(out[0], [0x90, 0x17, 0x00], "setting survives reconnect");
    }

    #[test]
    fn first_connect_adopts_physical_positions() {
        let mut s = ControllerSession::new();
        s.on_connect(true, &mut Vec::new());
        assert!(matches!(
            fader(&mut s, 0, 0x40),
            Some(ControllerAction::ChannelFader { .. })
        ));
    }

    #[test]
    fn reconnect_while_playing_requires_pickup() {
        let mut s = ControllerSession::new();
        s.on_connect(true, &mut Vec::new());
        fader(&mut s, 0, 0x7F); // fader up, music playing
        s.sync_software((Scope::Deck(DeckId::A), Control::ChannelFader), 1.0);
        s.on_connect(false, &mut Vec::new()); // unplugged and back
        assert_eq!(fader(&mut s, 0, 0x00), None, "no sudden mute");
        assert_eq!(
            s.pending_pickup((Scope::Deck(DeckId::A), Control::ChannelFader)),
            Some(0.0),
            "UI can show where the hardware is"
        );
        assert!(
            fader(&mut s, 0, 0x7F).is_some(),
            "picked up at the playing level"
        );
    }

    #[test]
    fn app_change_needs_pickup_on_the_controller() {
        let mut s = ControllerSession::new();
        s.on_connect(true, &mut Vec::new());
        s.on_input(&[0xB6, 0x1F, 0x00]);
        assert!(s.on_input(&[0xB6, 0x3F, 0x00]).1.is_some());
        s.sync_software((Scope::Global, Control::Crossfader), 1.0); // dragged in the app
        s.on_input(&[0xB6, 0x1F, 0x10]);
        assert_eq!(s.on_input(&[0xB6, 0x3F, 0x00]).1, None);
    }

    #[test]
    fn unmapped_or_garbage_messages_are_harmless() {
        let mut s = ControllerSession::new();
        assert_eq!(s.on_input(&[0xF8]), (None, None));
        let (event, action) = s.on_input(&[0x96, 0x78, 0x7F]); // shift + master cue: unassigned
        assert!(event.is_some());
        assert_eq!(action, None);
    }

    #[test]
    fn leds_are_resent_after_reconnect() {
        let mut s = ControllerSession::new();
        let mut state = LampState::default();
        state.decks[0].play = Lamp::On;
        let mut out = Vec::new();
        s.update_leds(&state, true, &mut out);
        out.clear();
        s.on_connect(false, &mut Vec::new());
        s.update_leds(&state, true, &mut out);
        assert!(out.contains(&[0x90, 0x0B, 0x7F]));
    }
}
