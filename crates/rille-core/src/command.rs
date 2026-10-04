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
    fn command_stays_small() {
        assert!(std::mem::size_of::<Command>() <= 24);
    }
}
