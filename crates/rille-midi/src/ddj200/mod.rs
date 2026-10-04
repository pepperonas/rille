//! Pioneer DDJ-200 support.

pub mod decode;
pub mod mapping;
pub mod table;

pub use decode::{ControlEvent, ControlValue, Decoder};
pub use mapping::{ControllerAction, EqBand, Mapper};
pub use table::{Control, GlobalLed, InputKind, InputSpec, Led, Scope};
