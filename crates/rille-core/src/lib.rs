//! Shared types for rille. This crate has no dependency on audio, MIDI or Tauri,
//! so the engine, the controller layer and the library can all speak the same language.

mod deck;

pub use deck::DeckId;
