//! Manual check: decode a file and report time.
//! `cargo run --release -p rille-library --example decode -- <file>`
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args().nth(1).ok_or("usage: decode <file>")?;
    let start = Instant::now();
    let track = rille_library::decode_file(std::path::Path::new(&path), 48_000, 1)?;
    let peak = track.samples.iter().fold(0f32, |m, s| m.max(s.abs()));
    println!(
        "{:.1} s audio, {} frames, peak {:.3}, decoded+resampled in {:?}",
        track.duration_secs(),
        track.frames(),
        peak,
        start.elapsed()
    );
    Ok(())
}
