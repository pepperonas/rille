//! Soft takeover for absolute controls.
//!
//! When the software value and the physical position disagree (after reconnecting the
//! controller, or after the value was changed in the app), a hardware move must not jump the
//! value. The control is "picked up" only once the hardware crosses the software value or comes
//! within [`PICKUP_WINDOW`] of it.

/// Distance at which a hardware position counts as matching the software value.
pub const PICKUP_WINDOW: f32 = 0.02;
/// Changes smaller than this from the software side are treated as echoes of our own moves.
const ECHO_TOLERANCE: f32 = 0.005;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoftTakeover {
    software: f32,
    hardware: Option<f32>,
    engaged: bool,
}

impl SoftTakeover {
    /// Starts not engaged: the first hardware move has to pick the value up.
    pub fn new(software: f32) -> SoftTakeover {
        SoftTakeover {
            software,
            hardware: None,
            engaged: false,
        }
    }

    pub fn engaged(&self) -> bool {
        self.engaged
    }

    /// Last known physical position (for drawing a "pick up here" ghost in the UI).
    pub fn hardware(&self) -> Option<f32> {
        self.hardware
    }

    /// Adopt the next hardware value directly (first connect while nothing is audible).
    pub fn engage(&mut self) {
        self.engaged = true;
    }

    /// Controller (re)connected: positions are unknown again.
    pub fn reset(&mut self) {
        self.hardware = None;
        self.engaged = false;
    }

    /// The value changed on the software side (UI, engine). Echoes of our own hardware moves
    /// are ignored; a real change disengages the control.
    pub fn sync_software(&mut self, value: f32) {
        if (value - self.software).abs() > ECHO_TOLERANCE {
            self.software = value;
            self.engaged = false;
        }
    }

    /// A hardware move. Returns the value to apply, or `None` while not picked up yet.
    pub fn on_hardware(&mut self, value: f32) -> Option<f32> {
        let previous = self.hardware.replace(value);
        if !self.engaged {
            let close = (value - self.software).abs() <= PICKUP_WINDOW;
            let crossed = previous
                .is_some_and(|p| (p - self.software).signum() != (value - self.software).signum());
            if !(close || crossed) {
                return None;
            }
            self.engaged = true;
        }
        self.software = value;
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_hardware_far_from_software_value() {
        let mut t = SoftTakeover::new(0.8);
        assert_eq!(t.on_hardware(0.1), None);
        assert_eq!(t.on_hardware(0.2), None);
    }

    #[test]
    fn picks_up_when_crossing() {
        let mut t = SoftTakeover::new(0.5);
        assert_eq!(t.on_hardware(0.3), None);
        assert_eq!(
            t.on_hardware(0.6),
            Some(0.6),
            "jumped over 0.5 in one message"
        );
        assert_eq!(t.on_hardware(0.1), Some(0.1), "engaged now");
    }

    #[test]
    fn picks_up_when_close() {
        let mut t = SoftTakeover::new(0.5);
        assert_eq!(t.on_hardware(0.51), Some(0.51));
    }

    #[test]
    fn software_change_disengages() {
        let mut t = SoftTakeover::new(0.5);
        t.engage();
        assert_eq!(t.on_hardware(0.9), Some(0.9));
        t.sync_software(0.2); // moved in the app
        assert_eq!(t.on_hardware(0.95), None);
        assert!(t.on_hardware(0.21).is_some());
    }

    #[test]
    fn own_echo_does_not_disengage() {
        let mut t = SoftTakeover::new(0.5);
        t.engage();
        t.on_hardware(0.7);
        t.sync_software(0.7001);
        assert_eq!(t.on_hardware(0.72), Some(0.72));
    }

    #[test]
    fn reconnect_mid_set_does_not_jump() {
        // Channel fader at 1.0 while the music plays; controller unplugged, fader knocked down,
        // plugged in again. The first message must not mute the channel.
        let mut t = SoftTakeover::new(0.0);
        t.engage();
        t.on_hardware(1.0);
        t.reset();
        assert_eq!(t.on_hardware(0.0), None);
        assert_eq!(t.on_hardware(0.4), None);
        assert_eq!(
            t.on_hardware(0.99),
            Some(0.99),
            "picked up near the playing level"
        );
    }
}
