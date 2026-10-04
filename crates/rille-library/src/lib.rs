//! Track library: import, decoding, BPM/beatgrid analysis, waveform peaks and the SQLite cache.

pub mod analysis;
pub mod decode;
pub mod resample;
pub mod scan;
pub mod tags;

pub use decode::{DecodeError, decode_file};
pub use rille_core::DeckId;
