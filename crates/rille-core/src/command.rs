use crate::DeckId;

/// Crossfader response curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrossfaderCurve {
    /// Constant power: both sides at -3 dB in the middle. Default for blending.
    #[default]
    Smooth,
    /// Linear amplitude, both sides at -6 dB in the middle.
    Linear,
    /// Scratch curve: both sides at full level almost everywhere, cut only at the edges.
    Cut,
}

/// EQ band of a mixer channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EqBand {
    Low = 0,
    Mid = 1,
    High = 2,
}

impl EqBand {
    pub const ALL: [EqBand; 3] = [EqBand::Low, EqBand::Mid, EqBand::High];

    pub const fn index(self) -> usize {
        self as usize
    }
}

/// The transition effect that the Transition FX button fires on the active deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionKind {
    /// The deck fades out while its last moment keeps echoing; the deck pauses afterwards.
    #[default]
    EchoOut,
    /// A high-pass sweeps up and takes the deck out; the deck pauses afterwards.
    FilterOut,
}

impl TransitionKind {
    pub const fn next(self) -> TransitionKind {
        match self {
            TransitionKind::EchoOut => TransitionKind::FilterOut,
            TransitionKind::FilterOut => TransitionKind::EchoOut,
        }
    }
}

/// Transport and cue actions of one deck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckCommand {
    PlayPause,
    Play,
    Pause,
    /// Cue button went down.
    CuePress,
    /// Cue button went up.
    CueRelease,
    /// Shift + Cue: back to the very start of the track.
    JumpToStart,
    /// Back to the cue point and pause (fader start "back cue").
    JumpToCue,
    Seek {
        frame: u64,
    },
    Unload,
}

/// Mixer parameters. Levels are normalised to `0.0..=1.0`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MixerCommand {
    ChannelFader(DeckId, f32),
    /// 0.0 = fully left (deck A), 1.0 = fully right (deck B).
    Crossfader(f32),
    CrossfaderCurve(CrossfaderCurve),
    /// Master level, 0.0..=1.0 maps to silence..unity.
    MasterGain(f32),
    /// Channel trim, 0.0..=1.0 maps to -12..+12 dB with 0.5 = unity.
    Trim(DeckId, f32),
    /// EQ knob, 0.0 = kill, 0.5 = unity, 1.0 = +6 dB.
    Eq(DeckId, EqBand, f32),
    EqKill(DeckId, EqBand, bool),
    /// Bipolar filter: 0.0 low-pass closed, 0.5 off, 1.0 high-pass open.
    Filter(DeckId, f32),
    /// Start the transition effect on the active deck, or cancel it if it is running.
    TransitionFx,
    /// Select the next transition effect.
    CycleTransitionFx,
}

/// Everything the engine accepts through its command queues. Small and `Copy` so it can travel
/// through a lock-free queue without allocating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Command {
    Deck(DeckId, DeckCommand),
    Mixer(MixerCommand),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transition_kinds_cycle() {
        assert_eq!(
            TransitionKind::EchoOut.next().next(),
            TransitionKind::EchoOut
        );
        assert_ne!(TransitionKind::EchoOut.next(), TransitionKind::EchoOut);
    }

    #[test]
    fn command_stays_small() {
        assert!(std::mem::size_of::<Command>() <= 24);
    }
}
