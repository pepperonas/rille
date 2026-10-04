//! Ring buffer of recent MIDI traffic for the developer view.

use std::collections::VecDeque;

use crate::ddj200::{Control, ControlEvent, ControlValue, Scope};

pub const CAPACITY: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonitorEntry {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Microseconds since the monitor started.
    pub time_us: u64,
    pub direction: Direction,
    pub bytes: [u8; 3],
    pub len: u8,
    /// Human-readable meaning, e.g. "Deck 1 · Play ↓".
    pub meaning: Option<String>,
}

#[derive(Debug, Default)]
pub struct Monitor {
    entries: VecDeque<MonitorEntry>,
    /// Entries pushed since the last `drain_new`.
    fresh: usize,
    next_seq: u64,
}

impl Monitor {
    pub fn new() -> Monitor {
        Monitor {
            entries: VecDeque::with_capacity(CAPACITY),
            fresh: 0,
            next_seq: 0,
        }
    }

    pub fn push(
        &mut self,
        time_us: u64,
        direction: Direction,
        bytes: &[u8],
        meaning: Option<String>,
    ) {
        let mut b = [0u8; 3];
        let len = bytes.len().min(3);
        b[..len].copy_from_slice(&bytes[..len]);
        if self.entries.len() == CAPACITY {
            self.entries.pop_front();
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        self.entries.push_back(MonitorEntry {
            seq,
            time_us,
            direction,
            bytes: b,
            len: len as u8,
            meaning,
        });
        self.fresh = (self.fresh + 1).min(CAPACITY);
    }

    pub fn entries(&self) -> impl Iterator<Item = &MonitorEntry> {
        self.entries.iter()
    }

    /// Entries added since the previous call (for streaming to the UI).
    pub fn drain_new(&mut self) -> Vec<MonitorEntry> {
        let skip = self.entries.len() - self.fresh;
        self.fresh = 0;
        self.entries.iter().skip(skip).cloned().collect()
    }
}

/// Short German description of a decoded event.
pub fn describe(event: &ControlEvent) -> String {
    let place = match event.scope {
        Scope::Deck(d) => format!("Deck {}", d.index() + 1),
        Scope::Global => "Mixer".to_string(),
    };
    let name = match event.control {
        Control::Play => "Play/Pause".to_string(),
        Control::Cue => "Cue".into(),
        Control::Shift => "Shift".into(),
        Control::Sync => "Beat Sync".into(),
        Control::SyncLong => "Beat Sync (lang)".into(),
        Control::TempoFader => "Tempo".into(),
        Control::JogTouch => "Jog berührt".into(),
        Control::JogRim => "Jog-Rand".into(),
        Control::JogPlatterVinyl => "Jog-Teller (Vinyl)".into(),
        Control::JogPlatter => "Jog-Teller".into(),
        Control::JogPlatterShift => "Jog-Teller (Shift)".into(),
        Control::EqHi => "EQ Hi".into(),
        Control::EqMid => "EQ Mid".into(),
        Control::EqLow => "EQ Low".into(),
        Control::ChannelFader => "Kanalfader".into(),
        Control::FaderStartPlay => "Fader Start: Play".into(),
        Control::FaderStartCue => "Fader Start: Cue".into(),
        Control::HeadphoneCue => "Kopfhörer-Cue".into(),
        Control::ColorFx => "Color FX".into(),
        Control::Pad(i) => format!("Pad {}", i + 1),
        Control::Crossfader => "Crossfader".into(),
        Control::MasterCue => "Master Cue".into(),
        Control::TransitionFx => "Transition FX".into(),
    };
    let shift = if event.shifted { " + Shift" } else { "" };
    let value = match event.value {
        ControlValue::Button(true) => " ↓".to_string(),
        ControlValue::Button(false) => " ↑".to_string(),
        ControlValue::Absolute(v) => format!(" {:.1} %", v * 100.0),
        ControlValue::Relative(t) => format!(" {t:+}"),
    };
    format!("{place} · {name}{shift}{value}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ddj200::Decoder;

    #[test]
    fn keeps_only_the_newest_entries() {
        let mut m = Monitor::new();
        for i in 0..(CAPACITY as u64 + 10) {
            m.push(i, Direction::In, &[0x90, 0x0B, 0x7F], None);
        }
        assert_eq!(m.entries().count(), CAPACITY);
        assert_eq!(m.entries().next().map(|e| e.time_us), Some(10));
    }

    #[test]
    fn drain_returns_only_new_entries() {
        let mut m = Monitor::new();
        m.push(1, Direction::In, &[0x90, 0x0B, 0x7F], None);
        m.push(2, Direction::Out, &[0x90, 0x0B, 0x7F], None);
        assert_eq!(m.drain_new().len(), 2);
        assert!(m.drain_new().is_empty());
        m.push(3, Direction::In, &[0xF8], None);
        let new = m.drain_new();
        assert_eq!((new.len(), new[0].len), (1, 1));
    }

    #[test]
    fn describes_events_in_german() {
        let mut d = Decoder::new();
        let e = d.decode(&[0x91, 0x47, 0x7F]).unwrap();
        assert_eq!(describe(&e), "Deck 2 · Play/Pause + Shift ↓");
        let e = d.decode(&[0xB0, 0x21, 0x3E]).unwrap();
        assert_eq!(describe(&e), "Deck 1 · Jog-Rand -2");
        d.decode(&[0xB6, 0x1F, 0x7F]);
        let e = d.decode(&[0xB6, 0x3F, 0x7F]).unwrap();
        assert_eq!(describe(&e), "Mixer · Crossfader 100.0 %");
    }
}
