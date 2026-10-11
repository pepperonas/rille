//! How long does a controller message take to become an action?
//! `cargo run --release -p rille-midi --example midi_latency`
//!
//! A virtual DDJ-200 sends 1000 Play presses through CoreMIDI; each is timed from `send` to the
//! decoded action on the input thread. Add one audio block for "controller → audible".
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::mpsc;
use std::time::{Duration, Instant};

use rille_midi::ddj200::ControllerAction;
use rille_midi::port::Connection;
use rille_midi::virtual_ddj::VirtualDdj;
use rille_midi::{ControllerSession, DeckId};

const COUNT: usize = 1000;

fn main() {
    rille_midi::port::init_on_main_thread();
    let name = format!("DDJ-200 latency {}", std::process::id());
    let mut ddj = VirtualDdj::create_named(&name).expect("virtual DDJ");
    std::thread::sleep(Duration::from_millis(300));

    let (tx, rx) = mpsc::channel::<(Instant, bool)>();
    let mut session = ControllerSession::new();
    session.on_connect(true, &mut Vec::new());
    let _conn = Connection::open(&name, move |_, bytes| {
        let is_play = session.on_input(bytes).1 == Some(ControllerAction::PlayPause(DeckId::A));
        let _ = tx.send((Instant::now(), is_play));
    })
    .expect("connect");

    let mut samples = Vec::with_capacity(COUNT);
    for i in 0..COUNT {
        // Alternate press and release so the session sees real button traffic.
        let sent = Instant::now();
        assert!(ddj.send(&[0x90, 0x0B, 0x7F]));
        let (at, ok) = rx.recv_timeout(Duration::from_secs(2)).expect("arrives");
        assert!(ok, "message {i} decoded as Play");
        samples.push(at.duration_since(sent));
        assert!(ddj.send(&[0x90, 0x0B, 0x00]));
        let _ = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("release arrives");
        std::thread::sleep(Duration::from_millis(2));
    }
    samples.sort();
    let pick = |q: f64| samples[((samples.len() - 1) as f64 * q) as usize];
    println!(
        "{COUNT} presses: median {:?}, p99 {:?}, worst {:?}",
        pick(0.5),
        pick(0.99),
        samples[samples.len() - 1]
    );
}
