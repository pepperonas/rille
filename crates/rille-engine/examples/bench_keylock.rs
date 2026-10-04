//! How expensive is keylock? `cargo run --release -p rille-engine --example bench_keylock`
use std::sync::Arc;
use std::time::Instant;

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
    for keylock in [false, true] {
        let (mut engine, mut h) = engine_pair(sr, 1024);
        for deck in DeckId::ALL {
            let _ = h.load(deck, Arc::new(TrackAudio::new(1, sr, samples.clone())));
            h.send(Command::Deck(deck, DeckCommand::Keylock(keylock)));
            h.send(Command::Deck(deck, DeckCommand::Tempo(0.5)));
            h.send(Command::Deck(deck, DeckCommand::Play));
        }
        let mut out = vec![0.0f32; 256 * 2];
        let blocks = 20 * sr as usize / 256;
        let start = Instant::now();
        for _ in 0..blocks {
            engine.process(&mut out, 2);
        }
        let took = start.elapsed();
        let budget = 20.0;
        println!(
            "keylock {keylock:5}: 20 s of two decks in {:.0} ms → {:.1} % of one core",
            took.as_secs_f64() * 1000.0,
            took.as_secs_f64() / budget * 100.0
        );
    }
}
