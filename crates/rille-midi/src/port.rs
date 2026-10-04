//! MIDI ports via midir/CoreMIDI: finding the controller, connecting, sending.

use std::time::Instant;

use midir::{MidiInput, MidiInputConnection, MidiOutput, MidiOutputConnection};

/// Port names of the DDJ-200 contain this (case-insensitive).
pub const DDJ200_PORT: &str = "DDJ-200";
const CLIENT: &str = "rille";

#[derive(Debug, thiserror::Error)]
pub enum MidiError {
    #[error("MIDI ist nicht verfügbar: {0}")]
    Unavailable(String),
    #[error("Controller nicht gefunden")]
    NotFound,
    #[error("Verbindung zum Controller fehlgeschlagen: {0}")]
    Connect(String),
}

/// Create rille's first CoreMIDI client on the calling thread. CoreMIDI delivers device-list
/// changes through the run loop of the thread that created a process's first client; created on
/// a worker thread without a run loop, newly plugged controllers are never seen. Call once from
/// the main thread at startup.
pub fn init_on_main_thread() {
    if let Err(e) = MidiInput::new(CLIENT) {
        tracing::warn!(%e, "CoreMIDI client could not be created");
    }
}

/// True if a port name belongs to the controller we look for.
pub fn matches(port_name: &str, pattern: &str) -> bool {
    port_name.to_lowercase().contains(&pattern.to_lowercase())
}

/// Names of MIDI inputs matching `pattern` (cheap; used for hotplug polling).
pub fn find_input(pattern: &str) -> Option<String> {
    let input = MidiInput::new(CLIENT).ok()?;
    input
        .ports()
        .iter()
        .filter_map(|p| input.port_name(p).ok())
        .find(|name| matches(name, pattern))
}

/// An open connection to the controller, input and output.
pub struct Connection {
    _input: MidiInputConnection<()>,
    output: MidiOutputConnection,
    pub name: String,
}

impl Connection {
    /// Connect to the first input/output pair matching `pattern`. `on_message` gets the time
    /// since connecting (µs) and the raw bytes; it runs on the CoreMIDI thread.
    pub fn open(
        pattern: &str,
        mut on_message: impl FnMut(u64, &[u8]) + Send + 'static,
    ) -> Result<Connection, MidiError> {
        let mut input =
            MidiInput::new(CLIENT).map_err(|e| MidiError::Unavailable(e.to_string()))?;
        input.ignore(midir::Ignore::All);
        let output = MidiOutput::new(CLIENT).map_err(|e| MidiError::Unavailable(e.to_string()))?;
        let in_port = input
            .ports()
            .into_iter()
            .find(|p| input.port_name(p).is_ok_and(|n| matches(&n, pattern)))
            .ok_or(MidiError::NotFound)?;
        let out_port = output
            .ports()
            .into_iter()
            .find(|p| output.port_name(p).is_ok_and(|n| matches(&n, pattern)))
            .ok_or(MidiError::NotFound)?;
        let name = input
            .port_name(&in_port)
            .unwrap_or_else(|_| pattern.to_string());
        let start = Instant::now();
        let input = input
            .connect(
                &in_port,
                "rille-in",
                move |_stamp, bytes, _| on_message(start.elapsed().as_micros() as u64, bytes),
                (),
            )
            .map_err(|e| MidiError::Connect(e.to_string()))?;
        let output = output
            .connect(&out_port, "rille-out")
            .map_err(|e| MidiError::Connect(e.to_string()))?;
        Ok(Connection {
            _input: input,
            output,
            name,
        })
    }

    pub fn send(&mut self, bytes: &[u8]) -> bool {
        self.output.send(bytes).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_is_case_insensitive_substring() {
        assert!(matches("DDJ-200", DDJ200_PORT));
        assert!(matches("Pioneer DDJ-200 MIDI 1", DDJ200_PORT));
        assert!(matches("ddj-200 virtual", DDJ200_PORT));
        assert!(!matches("DDJ-400", DDJ200_PORT));
    }
}
