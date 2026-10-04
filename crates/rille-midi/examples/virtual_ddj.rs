//! Run a virtual DDJ-200 next to the app: `cargo run -p rille-midi --example virtual_ddj`.
//! rille picks it up via hotplug; this prints what rille sends back (LEDs, vinyl mode).
use std::time::Duration;

use rille_midi::virtual_ddj::VirtualDdj;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let seconds: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(4);
    let mut ddj = VirtualDdj::create()?;
    println!("virtual DDJ-200 up, waiting for rille to connect …");
    std::thread::sleep(Duration::from_millis(2500));
    let first = ddj.take_received();
    println!("received {} messages after connect", first.len());
    for m in first.iter().filter(|m| m.get(1) == Some(&0x17)) {
        println!("  vinyl mode: {m:02X?}");
    }
    // Move the crossfader to the left and press shift.
    ddj.send(&[0xB6, 0x1F, 0x00]);
    ddj.send(&[0xB6, 0x3F, 0x00]);
    ddj.send(&[0x90, 0x3F, 0x7F]);
    ddj.send(&[0x90, 0x3F, 0x00]);
    std::thread::sleep(Duration::from_secs(seconds.saturating_sub(2)));
    println!("received {} more messages", ddj.take_received().len());
    Ok(())
}
