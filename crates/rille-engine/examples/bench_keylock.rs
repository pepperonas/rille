//! How expensive is keylock? `cargo run --release -p rille-engine --example bench_keylock`
//!
//! Prints the average load and the slowest single block against its real-time budget for
//! several buffer sizes. Signalsmith does its spectral work in bursts (one analysis per
//! interval), so the worst block matters more than the average for small buffers.
use std::sync::Arc;
use std::time::{Duration, Instant};

use rille_core::{Command, DeckCommand, DeckId, TrackAudio};
use rille_engine::engine_pair;

fn main() {
    let sr = 48_000u32;
    let seconds = 30usize;
    let samples: Vec<f32> = (0..sr as usize * seconds)
        .flat_map(|i| {
            let t = i as f32 / sr as f32;
            let v = 0.3 * (t * 110.0 * std::f32::consts::TAU).sin()
                + 0.2 * (t * 1760.0 * std::f32::consts::TAU).sin();
            [v, v]
        })
        .collect();
    for block in [64usize, 128, 256] {
        for keylock in [false, true] {
            let (mut engine, mut h) = engine_pair(sr, 1024);
            for deck in DeckId::ALL {
                let _ = h.load(deck, Arc::new(TrackAudio::new(1, sr, samples.clone())));
                h.send(Command::Deck(deck, DeckCommand::Keylock(keylock)));
                h.send(Command::Deck(deck, DeckCommand::Tempo(0.5)));
                h.send(Command::Deck(deck, DeckCommand::Play));
            }
            let mut out = vec![0.0f32; block * 2];
            let blocks = 20 * sr as usize / block;
            let mut worst = Duration::ZERO;
            let start = Instant::now();
            for _ in 0..blocks {
                let t = Instant::now();
                engine.process(&mut out, 2);
                worst = worst.max(t.elapsed());
            }
            let took = start.elapsed();
            let budget = Duration::from_secs_f64(block as f64 / f64::from(sr));
            println!(
                "block {block:4}, keylock {keylock:5}: average {:4.1} % of one core, \
                 worst block {:5.0} µs = {:3.0} % of its {:.0} µs budget",
                took.as_secs_f64() / 20.0 * 100.0,
                worst.as_secs_f64() * 1e6,
                worst.as_secs_f64() / budget.as_secs_f64() * 100.0,
                budget.as_secs_f64() * 1e6,
            );
        }
    }
}
