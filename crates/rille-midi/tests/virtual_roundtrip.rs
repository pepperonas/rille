#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code
//! End to end through CoreMIDI: virtual controller → port → session → action, and LEDs back.

use std::sync::mpsc;
use std::time::Duration;

use rille_midi::ddj200::{ControllerAction, Lamp, LampState};
use rille_midi::port::Connection;
use rille_midi::virtual_ddj::VirtualDdj;
use rille_midi::{ControllerSession, DeckId};

#[test]
fn bytes_travel_both_ways() {
    // Unique name per run so parallel test runs do not see each other's ports.
    let name = format!("DDJ-200 test {}", std::process::id());
    let mut ddj = match VirtualDdj::create_named(&name) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipping: CoreMIDI unavailable ({e})");
            return;
        }
    };
    std::thread::sleep(Duration::from_millis(200)); // let CoreMIDI publish the ports

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let mut conn = Connection::open(&name, move |_, bytes| {
        let _ = tx.send(bytes.to_vec());
    })
    .expect("connect to virtual DDJ");

    // Controller → app
    assert!(ddj.send(&[0x90, 0x0B, 0x7F]));
    let bytes = rx
        .recv_timeout(Duration::from_secs(2))
        .expect("message arrives");
    let mut session = ControllerSession::new();
    session.on_connect(true, &mut Vec::new());
    assert_eq!(
        session.on_input(&bytes).1,
        Some(ControllerAction::PlayPause(DeckId::A))
    );

    // App → controller (LEDs)
    let mut state = LampState::default();
    state.decks[0].play = Lamp::On;
    let mut out = Vec::new();
    session.update_leds(&state, true, &mut out);
    for msg in &out {
        assert!(conn.send(msg));
    }
    std::thread::sleep(Duration::from_millis(200));
    let received = ddj.take_received();
    assert!(
        received.contains(&vec![0x90, 0x0B, 0x7F]),
        "LED message received"
    );
}
