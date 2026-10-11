//! Does background analysis disturb playback?
//! `cargo run --release -p rille-library --example load_test -- <folder or files>`
//!
//! Renders two keylocked decks block by block at real-time pace (256 frames at 48 kHz, as the
//! audio thread would) while `cores − 2` threads analyse the given files over and over at QoS
//! "utility", like the app's workers. Reports the slowest blocks against their budget.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use rille_core::{Command, DeckCommand, DeckId, TrackAudio};
use rille_engine::engine_pair;
use rille_library::analysis::analyze_file;
use rille_library::scan::{is_audio_file, scan};

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const SECONDS: u64 = 30;

fn main() {
    let files: Vec<PathBuf> = std::env::args()
        .skip(1)
        .map(PathBuf::from)
        .flat_map(|p| if p.is_dir() { scan(&p) } else { vec![p] })
        .filter(|p| is_audio_file(p))
        .collect();
    assert!(
        !files.is_empty(),
        "usage: load_test <folder or audio files>"
    );
    let cores = thread::available_parallelism().map_or(4, |n| n.get());
    let workers = cores.saturating_sub(2).max(1);

    for with_analysis in [false, true] {
        let stop = Arc::new(AtomicBool::new(false));
        let analysed = Arc::new(AtomicUsize::new(0));
        let handles: Vec<_> = (0..if with_analysis { workers } else { 0 })
            .map(|w| {
                let (files, stop, analysed) = (files.clone(), stop.clone(), analysed.clone());
                thread::spawn(move || {
                    #[cfg(target_os = "macos")]
                    // SAFETY: changes only this thread's scheduling class.
                    unsafe {
                        libc::pthread_set_qos_class_self_np(
                            libc::qos_class_t::QOS_CLASS_UTILITY,
                            0,
                        );
                    }
                    let mut i = w;
                    while !stop.load(Ordering::Relaxed) {
                        let _ =
                            analyze_file(&files[i % files.len()], || !stop.load(Ordering::Relaxed));
                        analysed.fetch_add(1, Ordering::Relaxed);
                        i += workers;
                    }
                })
            })
            .collect();

        let (worst, over, blocks) = thread::spawn(play).join().unwrap();
        stop.store(true, Ordering::Relaxed);
        for h in handles {
            h.join().unwrap();
        }
        let budget = BLOCK as f64 / f64::from(RATE) * 1e6;
        println!(
            "{:<28} {blocks} blocks, worst {:5.0} µs = {:3.0} % of {budget:.0} µs, \
             {over} over budget, {} tracks analysed",
            if with_analysis {
                format!("analysis on {workers} workers:")
            } else {
                "playback alone:".to_string()
            },
            worst.as_secs_f64() * 1e6,
            worst.as_secs_f64() * 1e6 / budget * 100.0,
            analysed.load(Ordering::Relaxed),
        );
    }
}

/// Give this thread the scheduling CoreAudio gives its I/O thread: Mach time-constraint
/// (real-time) with the block period. Without it the measurement would be about the machine's
/// other load, not about the analysis.
#[cfg(target_os = "macos")]
#[allow(deprecated)] // libc's Mach bindings point to the mach2 crate; not worth a dependency here
fn make_realtime() {
    let mut tb = libc::mach_timebase_info { numer: 0, denom: 0 };
    // SAFETY: plain Mach calls on the current thread with valid, initialised arguments.
    unsafe {
        libc::mach_timebase_info(&mut tb);
        let to_abs = |ns: f64| (ns * f64::from(tb.denom) / f64::from(tb.numer)) as u32;
        let period_ns = BLOCK as f64 / f64::from(RATE) * 1e9;
        let mut policy = libc::thread_time_constraint_policy {
            period: to_abs(period_ns),
            computation: to_abs(period_ns * 0.5),
            constraint: to_abs(period_ns),
            preemptible: 1,
        };
        let thread = libc::mach_thread_self();
        let rc = libc::thread_policy_set(
            thread,
            libc::THREAD_TIME_CONSTRAINT_POLICY as libc::thread_policy_flavor_t,
            (&mut policy as *mut libc::thread_time_constraint_policy).cast(),
            libc::THREAD_TIME_CONSTRAINT_POLICY_COUNT,
        );
        assert_eq!(rc, 0, "could not make the render thread real-time");
    }
}

/// Two decks with keylock and shifted tempo, rendered at real-time pace.
fn play() -> (Duration, usize, usize) {
    #[cfg(target_os = "macos")]
    make_realtime();
    let samples: Vec<f32> = (0..RATE as usize * 60)
        .flat_map(|i| {
            let t = i as f32 / RATE as f32;
            let v = 0.3 * (t * 110.0 * std::f32::consts::TAU).sin()
                + 0.2 * (t * 1760.0 * std::f32::consts::TAU).sin();
            [v, v]
        })
        .collect();
    let (mut engine, mut h) = engine_pair(RATE, 1024);
    for deck in DeckId::ALL {
        let _ = h.load(deck, Arc::new(TrackAudio::new(1, RATE, samples.clone())));
        h.send(Command::Deck(deck, DeckCommand::Keylock(true)));
        h.send(Command::Deck(deck, DeckCommand::Tempo(0.3)));
        h.send(Command::Deck(deck, DeckCommand::Play));
    }
    let mut out = vec![0.0f32; BLOCK * 2];
    let period = Duration::from_secs_f64(BLOCK as f64 / f64::from(RATE));
    let blocks = (SECONDS * u64::from(RATE)) as usize / BLOCK;
    let (mut worst, mut over) = (Duration::ZERO, 0);
    let mut next = Instant::now();
    for _ in 0..blocks {
        let t = Instant::now();
        engine.process(&mut out, 2);
        let took = t.elapsed();
        worst = worst.max(took);
        if took > period {
            over += 1;
        }
        next += period;
        thread::sleep(next.saturating_duration_since(Instant::now()));
    }
    (worst, over, blocks)
}
