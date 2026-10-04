//! Shared types for rille. This crate has no dependency on audio, MIDI or Tauri,
//! so the engine, the controller layer and the library can all speak the same language.

mod command;
mod deck;
mod snapshot;
mod track;

pub use command::{Command, CrossfaderCurve, DeckCommand, MixerCommand};
pub use deck::DeckId;
pub use snapshot::{DeckSnapshot, Snapshot};
pub use track::TrackAudio;
