//! Manual check: plays a quiet 440 Hz tone for half a second on the default output, with the
//! allocation guard active in the callback. `cargo run -p rille-engine --example tone`
use std::sync::Arc;
use std::time::Duration;

use assert_no_alloc::AllocDisabler;
use rille_core::{Command, DeckCommand, DeckId, MixerCommand, TrackAudio};
use rille_engine::output::{self, OutputRequest};
use rille_engine::{EngineSlot, engine_pair};

#[global_allocator]
static ALLOC: AllocDisabler = AllocDisabler;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    for d in output::list_devices() {
        println!(
            "{} {:>2}ch {} [{}]",
            if d.is_default { "*" } else { " " },
            d.max_channels,
            d.name,
            d.id
        );
    }
    let rate = 48_000;
    let samples: Vec<f32> = (0..rate)
        .flat_map(|i| {
            let v = 0.05 * (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin();
            [v, v]
        })
        .collect();
    let (engine, mut handle) = engine_pair(rate, 256);
    let slot = EngineSlot::new(engine);
    let stream = output::open(
        &slot,
        &OutputRequest {
            device_id: None,
            buffer_frames: 256,
            sample_rate: rate,
        },
        |failure| eprintln!("stream failure: {failure:?}"),
    )?;
    println!("{:?}", stream.info);
    handle
        .load(DeckId::A, Arc::new(TrackAudio::new(1, rate, samples)))
        .map_err(|_| "queue")?;
    handle.send(Command::Mixer(MixerCommand::Crossfader(0.0)));
    handle.send(Command::Deck(DeckId::A, DeckCommand::Play));
    std::thread::sleep(Duration::from_millis(500));
    let s = handle.snapshot();
    println!(
        "position {} frames, peak {:.3}, xruns {}, device latency {} µs",
        s.decks[0].position,
        s.master_peak[0],
        s.xruns,
        stream
            .stats
            .device_latency_us
            .load(std::sync::atomic::Ordering::Relaxed)
    );
    Ok(())
}
