//! Manual check: how soon can a file play, and how long does the whole decode take?
//! `cargo run --release -p rille-library --example decode -- <file> [rate]`
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: decode <file> [rate]")?;
    let rate: u32 = args
        .next()
        .map(|r| r.parse())
        .transpose()?
        .unwrap_or(48_000);
    let start = Instant::now();
    let stream = rille_library::decode::open_stream(std::path::Path::new(&path), rate, 1)?;
    let opened = start.elapsed();
    let track = stream.track();
    // Time until one second of audio is ready: what playback needs to start without a gap.
    let watcher = {
        let track = track.clone();
        std::thread::spawn(move || {
            let begin = Instant::now();
            while track.ready_frames() < rate as usize && !track.is_complete() {
                std::thread::sleep(Duration::from_micros(200));
            }
            begin.elapsed()
        })
    };
    stream.run(|| true)?;
    let total = start.elapsed();
    let first_second = watcher.join().map_err(|_| "watcher panicked")? + opened;
    let peak = track.to_vec().iter().fold(0f32, |m, s| m.max(s.abs()));
    println!(
        "{:.1} s audio at {rate} Hz, peak {peak:.3}\n  opened (playable) after {opened:?}\n  \
         first second ready after {first_second:?}\n  complete after {total:?}",
        track.duration_secs(),
    );
    Ok(())
}
