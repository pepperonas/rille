//! A virtual DDJ-200 made of CoreMIDI virtual ports. Lets the app and the tests talk to "the
//! controller" without hardware: bytes sent with [`VirtualDdj::send`] arrive at rille as if the
//! device sent them, and everything rille sends back (LEDs) is collected.

use std::sync::{Arc, Mutex};

use midir::os::unix::{VirtualInput, VirtualOutput};
use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

use crate::port::MidiError;

/// Part of every virtual controller name; real devices never contain it.
pub const VIRTUAL_MARKER: &str = "(rille virtuell)";
pub const VIRTUAL_NAME: &str = "DDJ-200 (rille virtuell)";

pub struct VirtualDdj {
    to_app: MidiOutputConnection,
    _from_app: MidiInputConnection<()>,
    received: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl VirtualDdj {
    pub fn create() -> Result<VirtualDdj, MidiError> {
        Self::create_named(VIRTUAL_NAME)
    }

    pub fn create_named(name: &str) -> Result<VirtualDdj, MidiError> {
        let out = MidiOutput::new("rille-virtual-ddj")
            .map_err(|e| MidiError::Unavailable(e.to_string()))?;
        let to_app = out
            .create_virtual(name)
            .map_err(|e| MidiError::Connect(e.to_string()))?;
        let received = Arc::new(Mutex::new(Vec::new()));
        let sink = received.clone();
        let input = MidiInput::new("rille-virtual-ddj")
            .map_err(|e| MidiError::Unavailable(e.to_string()))?;
        let from_app = input
            .create_virtual(
                name,
                move |_, bytes, _| {
                    if let Ok(mut v) = sink.lock() {
                        v.push(bytes.to_vec());
                    }
                },
                (),
            )
            .map_err(|e| MidiError::Connect(e.to_string()))?;
        Ok(VirtualDdj {
            to_app,
            _from_app: from_app,
            received,
        })
    }

    /// Send bytes as if the controller produced them.
    pub fn send(&mut self, bytes: &[u8]) -> bool {
        self.to_app.send(bytes).is_ok()
    }

    /// Everything the app sent to the controller so far (and clear the list).
    pub fn take_received(&self) -> Vec<Vec<u8>> {
        self.received
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }
}
