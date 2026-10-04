//! Print MIDI inputs/outputs every second (diagnostics). `cargo run -p rille-midi --example list_ports`
use midir::{MidiInput, MidiOutput};
fn main() {
    let n: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    for _ in 0..n {
        if let (Ok(i), Ok(o)) = (MidiInput::new("probe"), MidiOutput::new("probe")) {
            let ins: Vec<_> = i
                .ports()
                .iter()
                .filter_map(|p| i.port_name(p).ok())
                .collect();
            let outs: Vec<_> = o
                .ports()
                .iter()
                .filter_map(|p| o.port_name(p).ok())
                .collect();
            println!("in: {ins:?}\nout: {outs:?}");
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
