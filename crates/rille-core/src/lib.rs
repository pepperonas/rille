//! Shared types for rille. This crate has no dependency on audio, MIDI or Tauri,
//! so the engine, the controller layer and the library can all speak the same language.

mod command;
mod deck;
mod grid;
mod snapshot;
mod track;

pub use command::{
    Command, CrossfaderCurve, DeckCommand, EqBand, MixerCommand, TempoRange, TransitionKind,
};
pub use deck::DeckId;
pub use grid::BeatGrid;
pub use snapshot::{DeckSnapshot, Snapshot, TransitionSnapshot};
pub use track::TrackAudio;
