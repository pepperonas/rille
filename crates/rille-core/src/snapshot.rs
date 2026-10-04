use crate::DeckId;
use crate::command::{CrossfaderCurve, TransitionKind};

/// State of one deck as seen at the end of an audio block.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DeckSnapshot {
    /// Id of the loaded track, `None` when the deck is empty.
    pub track_id: Option<u64>,
    /// Playback position in engine frames.
    pub position: u64,
    /// Track length in engine frames.
    pub frames: u64,
    pub playing: bool,
    /// Main cue point in engine frames.
    pub cue: u64,
    /// The cue button is held and the deck plays from the cue point.
    pub previewing: bool,
    /// Post-fader channel peak (linear, decaying), left/right.
    pub peak: [f32; 2],
}

/// State of the transition effect.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TransitionSnapshot {
    pub kind: TransitionKind,
    /// Deck the effect runs on, `None` when idle.
    pub deck: Option<DeckId>,
    /// The effect was cancelled and only its tail is still sounding.
    pub releasing: bool,
}

/// Engine state handed from the audio thread to the rest of the app. `Copy`, no heap data.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Snapshot {
    /// Frames rendered since the engine started; doubles as a monotonic clock.
    pub frame_clock: u64,
    pub sample_rate: u32,
    pub decks: [DeckSnapshot; 2],
    pub channel_fader: [f32; 2],
    pub trim: [f32; 2],
    pub crossfader: f32,
    pub curve: CrossfaderCurve,
    pub master_gain: f32,
    pub master_peak: [f32; 2],
    /// EQ knob positions per deck: low, mid, high.
    pub eq: [[f32; 3]; 2],
    pub eq_kill: [[bool; 3]; 2],
    pub filter: [f32; 2],
    pub transition: TransitionSnapshot,
    /// Output callbacks that arrived late (device under-runs).
    pub xruns: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_copy<T: Copy>() {}

    #[test]
    fn snapshot_is_plain_data() {
        assert_copy::<Snapshot>();
        assert_eq!(Snapshot::default().decks[0].track_id, None);
    }
}
