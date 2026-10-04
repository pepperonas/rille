//! Data shapes exchanged with the frontend (camelCase JSON). Kept separate from the engine
//! types so the engine never depends on serde or Tauri.

use rille_core::{
    CrossfaderCurve, DeckCommand, DeckId, EqBand, MixerCommand, Snapshot, TransitionKind,
};
use rille_engine::output::DeviceInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Deck {
    A,
    B,
}

impl From<Deck> for DeckId {
    fn from(d: Deck) -> DeckId {
        match d {
            Deck::A => DeckId::A,
            Deck::B => DeckId::B,
        }
    }
}

impl From<DeckId> for Deck {
    fn from(d: DeckId) -> Deck {
        match d {
            DeckId::A => Deck::A,
            DeckId::B => Deck::B,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Curve {
    Smooth,
    Linear,
    Cut,
}

impl From<Curve> for CrossfaderCurve {
    fn from(c: Curve) -> CrossfaderCurve {
        match c {
            Curve::Smooth => CrossfaderCurve::Smooth,
            Curve::Linear => CrossfaderCurve::Linear,
            Curve::Cut => CrossfaderCurve::Cut,
        }
    }
}

impl From<CrossfaderCurve> for Curve {
    fn from(c: CrossfaderCurve) -> Curve {
        match c {
            CrossfaderCurve::Smooth => Curve::Smooth,
            CrossfaderCurve::Linear => Curve::Linear,
            CrossfaderCurve::Cut => Curve::Cut,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Band {
    Low,
    Mid,
    High,
}

impl From<Band> for EqBand {
    fn from(b: Band) -> EqBand {
        match b {
            Band::Low => EqBand::Low,
            Band::Mid => EqBand::Mid,
            Band::High => EqBand::High,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Transition {
    EchoOut,
    FilterOut,
}

impl From<TransitionKind> for Transition {
    fn from(k: TransitionKind) -> Transition {
        match k {
            TransitionKind::EchoOut => Transition::EchoOut,
            TransitionKind::FilterOut => Transition::FilterOut,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DeckAction {
    PlayPause,
    Play,
    Pause,
    CuePress,
    CueRelease,
    JumpToStart,
    JumpToCue,
    Seek { frame: u64 },
    Unload,
}

impl From<DeckAction> for DeckCommand {
    fn from(a: DeckAction) -> DeckCommand {
        match a {
            DeckAction::PlayPause => DeckCommand::PlayPause,
            DeckAction::Play => DeckCommand::Play,
            DeckAction::Pause => DeckCommand::Pause,
            DeckAction::CuePress => DeckCommand::CuePress,
            DeckAction::CueRelease => DeckCommand::CueRelease,
            DeckAction::JumpToStart => DeckCommand::JumpToStart,
            DeckAction::JumpToCue => DeckCommand::JumpToCue,
            DeckAction::Seek { frame } => DeckCommand::Seek { frame },
            DeckAction::Unload => DeckCommand::Unload,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum MixerAction {
    ChannelFader { deck: Deck, value: f32 },
    Trim { deck: Deck, value: f32 },
    Crossfader { value: f32 },
    Curve { curve: Curve },
    MasterGain { value: f32 },
    Eq { deck: Deck, band: Band, value: f32 },
    EqKill { deck: Deck, band: Band, kill: bool },
    Filter { deck: Deck, value: f32 },
    TransitionFx,
    CycleTransitionFx,
}

impl MixerAction {
    /// The controller element this action also exists on, with its value in hardware space.
    pub fn takeover(&self) -> Option<(rille_midi::TakeoverKey, f32)> {
        use rille_midi::ddj200::{Control, Scope};
        match *self {
            MixerAction::ChannelFader { deck, value } => {
                Some(((Scope::Deck(deck.into()), Control::ChannelFader), value))
            }
            MixerAction::Crossfader { value } => {
                Some(((Scope::Global, Control::Crossfader), value))
            }
            MixerAction::Eq { deck, band, value } => {
                let control = match band {
                    Band::Low => Control::EqLow,
                    Band::Mid => Control::EqMid,
                    Band::High => Control::EqHi,
                };
                Some(((Scope::Deck(deck.into()), control), value))
            }
            MixerAction::Filter { deck, value } => {
                Some(((Scope::Deck(deck.into()), Control::ColorFx), value))
            }
            _ => None,
        }
    }
}

impl From<MixerAction> for MixerCommand {
    fn from(a: MixerAction) -> MixerCommand {
        match a {
            MixerAction::ChannelFader { deck, value } => {
                MixerCommand::ChannelFader(deck.into(), value)
            }
            MixerAction::Trim { deck, value } => MixerCommand::Trim(deck.into(), value),
            MixerAction::Crossfader { value } => MixerCommand::Crossfader(value),
            MixerAction::Curve { curve } => MixerCommand::CrossfaderCurve(curve.into()),
            MixerAction::MasterGain { value } => MixerCommand::MasterGain(value),
            MixerAction::Eq { deck, band, value } => {
                MixerCommand::Eq(deck.into(), band.into(), value)
            }
            MixerAction::EqKill { deck, band, kill } => {
                MixerCommand::EqKill(deck.into(), band.into(), kill)
            }
            MixerAction::Filter { deck, value } => MixerCommand::Filter(deck.into(), value),
            MixerAction::TransitionFx => MixerCommand::TransitionFx,
            MixerAction::CycleTransitionFx => MixerCommand::CycleTransitionFx,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckFrame {
    pub track_id: Option<u64>,
    pub position: u64,
    pub frames: u64,
    pub playing: bool,
    pub cue: u64,
    pub previewing: bool,
    pub peak: [f32; 2],
}

/// One state update, sent at most 60 times per second.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StateFrame {
    pub frame_clock: u64,
    pub sample_rate: u32,
    pub decks: [DeckFrame; 2],
    pub channel_fader: [f32; 2],
    pub trim: [f32; 2],
    pub crossfader: f32,
    pub curve: Curve,
    pub master_gain: f32,
    pub master_peak: [f32; 2],
    /// EQ knob positions per deck: low, mid, high.
    pub eq: [[f32; 3]; 2],
    pub eq_kill: [[bool; 3]; 2],
    pub filter: [f32; 2],
    pub transition: TransitionFrame,
    pub xruns: u32,
    pub dropped_commands: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransitionFrame {
    pub kind: Transition,
    pub deck: Option<Deck>,
    pub releasing: bool,
}

impl StateFrame {
    pub fn from_snapshot(s: &Snapshot, dropped_commands: u32) -> StateFrame {
        let deck = |d: &rille_core::DeckSnapshot| DeckFrame {
            track_id: d.track_id,
            position: d.position,
            frames: d.frames,
            playing: d.playing,
            cue: d.cue,
            previewing: d.previewing,
            peak: d.peak,
        };
        StateFrame {
            frame_clock: s.frame_clock,
            sample_rate: s.sample_rate,
            decks: [deck(&s.decks[0]), deck(&s.decks[1])],
            channel_fader: s.channel_fader,
            trim: s.trim,
            crossfader: s.crossfader,
            curve: s.curve.into(),
            master_gain: s.master_gain,
            master_peak: s.master_peak,
            eq: s.eq,
            eq_kill: s.eq_kill,
            filter: s.filter,
            transition: TransitionFrame {
                kind: s.transition.kind.into(),
                deck: s.transition.deck.map(Deck::from),
                releasing: s.transition.releasing,
            },
            xruns: s.xruns,
            dropped_commands,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub max_channels: u16,
    pub is_default: bool,
}

impl From<DeviceInfo> for AudioDevice {
    fn from(d: DeviceInfo) -> AudioDevice {
        AudioDevice {
            id: d.id,
            name: d.name,
            max_channels: d.max_channels,
            is_default: d.is_default,
        }
    }
}

/// Output status shown in the settings and in the title bar.
#[derive(Debug, Clone, PartialEq, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatus {
    pub connected: bool,
    pub device_id: Option<String>,
    pub device_name: Option<String>,
    pub sample_rate: u32,
    pub channels: u16,
    /// Requested buffer size in frames.
    pub requested_buffer: u32,
    /// Buffer size in use; `None` if the device chose its own.
    pub buffer_frames: Option<u32>,
    /// Buffer + device latency in milliseconds.
    pub latency_ms: f32,
    /// Overloads CoreAudio reported for the current stream.
    pub device_xruns: u32,
    /// Last problem, in German, ready to show.
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckLoaded {
    pub deck: Deck,
    pub track_id: u64,
    pub title: String,
    pub duration_secs: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckLoadFailed {
    pub deck: Deck,
    pub track_id: u64,
    pub title: String,
    pub message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deck_actions_parse_from_tagged_json() {
        let a: DeckAction = serde_json::from_str(r#"{"type":"cuePress"}"#).unwrap();
        assert_eq!(DeckCommand::from(a), DeckCommand::CuePress);
        let a: DeckAction = serde_json::from_str(r#"{"type":"seek","frame":42}"#).unwrap();
        assert_eq!(DeckCommand::from(a), DeckCommand::Seek { frame: 42 });
    }

    #[test]
    fn mixer_actions_parse_from_tagged_json() {
        let a: MixerAction =
            serde_json::from_str(r#"{"type":"channelFader","deck":"b","value":0.5}"#).unwrap();
        assert_eq!(
            MixerCommand::from(a),
            MixerCommand::ChannelFader(DeckId::B, 0.5)
        );
        let a: MixerAction =
            serde_json::from_str(r#"{"type":"eqKill","deck":"a","band":"low","kill":true}"#)
                .unwrap();
        assert_eq!(
            MixerCommand::from(a),
            MixerCommand::EqKill(DeckId::A, EqBand::Low, true)
        );
        let a: MixerAction = serde_json::from_str(r#"{"type":"transitionFx"}"#).unwrap();
        assert_eq!(MixerCommand::from(a), MixerCommand::TransitionFx);
        let a: MixerAction = serde_json::from_str(r#"{"type":"curve","curve":"cut"}"#).unwrap();
        assert_eq!(
            MixerCommand::from(a),
            MixerCommand::CrossfaderCurve(CrossfaderCurve::Cut)
        );
    }

    #[test]
    fn app_fader_moves_are_reported_for_takeover() {
        use rille_midi::ddj200::{Control, Scope};
        let a = MixerAction::ChannelFader {
            deck: Deck::B,
            value: 0.3,
        };
        assert_eq!(
            a.takeover(),
            Some(((Scope::Deck(DeckId::B), Control::ChannelFader), 0.3))
        );
        assert_eq!(
            MixerAction::Crossfader { value: 0.1 }.takeover(),
            Some(((Scope::Global, Control::Crossfader), 0.1))
        );
        assert_eq!(
            MixerAction::MasterGain { value: 1.0 }.takeover(),
            None,
            "not on the DDJ-200"
        );
        assert_eq!(
            MixerAction::Eq {
                deck: Deck::A,
                band: Band::High,
                value: 0.7
            }
            .takeover(),
            Some(((Scope::Deck(DeckId::A), Control::EqHi), 0.7))
        );
        assert_eq!(
            MixerAction::Filter {
                deck: Deck::B,
                value: 0.2
            }
            .takeover(),
            Some(((Scope::Deck(DeckId::B), Control::ColorFx), 0.2))
        );
        assert_eq!(
            MixerAction::TransitionFx.takeover(),
            None,
            "a button, not a fader"
        );
    }

    #[test]
    fn state_frame_serialises_camel_case() {
        let frame = StateFrame::from_snapshot(&Snapshot::default(), 3);
        let json = serde_json::to_value(&frame).unwrap();
        for key in [
            "frameClock",
            "sampleRate",
            "decks",
            "channelFader",
            "trim",
            "crossfader",
            "curve",
            "masterGain",
            "masterPeak",
            "eq",
            "eqKill",
            "filter",
            "transition",
            "xruns",
            "droppedCommands",
        ] {
            assert!(json.get(key).is_some(), "missing {key}");
        }
        assert_eq!(json["decks"][0]["trackId"], serde_json::Value::Null);
        assert_eq!(json["curve"], "smooth");
        assert_eq!(json["droppedCommands"], 3);
    }
}
