//! Realtime audio engine. The audio callback in this crate never allocates, locks, logs or does
//! I/O; see `.claude/rules/audio-realtime.md`.

pub mod curves;
pub mod dsp;
pub mod engine;
pub mod meter;
pub mod output;
pub mod slot;
pub mod smooth;
pub mod transport;

pub use engine::{CommandSender, Engine, EngineEvent, EngineHandle, TrackLoad, engine_pair};
pub use rille_core::DeckId;
pub use slot::EngineSlot;
