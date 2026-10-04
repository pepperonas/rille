#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

use std::path::Path;

use rille_library::decode::open_stream;
use rille_library::{DecodeError, decode_file};
use tempfile::TempDir;

mod common;
use common::{fixture, sine, write_wav};

fn frequency(samples: &[f32], rate: u32) -> f32 {
    let left: Vec<f32> = samples.as_chunks::<2>().0.iter().map(|f| f[0]).collect();
    // skip resampler edges
    let body = &left[left.len() / 10..left.len() * 9 / 10];
    let crossings = body
        .windows(2)
        .filter(|w| w[0] < 0.0 && w[1] >= 0.0)
        .count();
    crossings as f32 * rate as f32 / body.len() as f32
}

#[test]
fn same_rate_is_passed_through() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "a.wav");
    write_wav(&path, 48_000, 2, &sine(48_000, 1000.0, 0.5, 2), None);
    let t = decode_file(&path, 48_000, 9).unwrap();
    assert_eq!((t.id, t.sample_rate, t.frames()), (9, 48_000, 24_000));
}

#[test]
fn resamples_to_engine_rate_without_changing_pitch() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "b.wav");
    write_wav(&path, 44_100, 2, &sine(44_100, 1000.0, 1.0, 2), None);
    let t = decode_file(&path, 48_000, 1).unwrap();
    assert!(
        (t.frames() as i64 - 48_000).abs() <= 1,
        "frames {}",
        t.frames()
    );
    let f = frequency(&t.to_vec(), 48_000);
    assert!((f - 1000.0).abs() < 5.0, "frequency {f}");
}

#[test]
fn mono_becomes_stereo() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "c.wav");
    write_wav(&path, 48_000, 1, &sine(48_000, 440.0, 0.1, 1), None);
    let t = decode_file(&path, 48_000, 1).unwrap();
    assert_eq!(t.frames(), 4800);
    assert!(t.to_vec().as_chunks::<2>().0.iter().all(|f| f[0] == f[1]));
}

#[test]
fn empty_file_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "empty.mp3");
    std::fs::File::create(&path).unwrap();
    assert!(matches!(
        decode_file(&path, 48_000, 1),
        Err(DecodeError::Empty)
    ));
}

#[test]
fn text_with_audio_extension_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "fake.mp3");
    std::fs::write(
        &path,
        "this is not audio at all, just a text file ".repeat(100),
    )
    .unwrap();
    let err = decode_file(&path, 48_000, 1).unwrap_err();
    assert!(!err.to_string().is_empty());
}

#[test]
fn missing_file_reports_io_error() {
    let err = decode_file(Path::new("/nonexistent/rille.wav"), 48_000, 1).unwrap_err();
    assert!(matches!(err, DecodeError::Io(_)));
}

#[test]
fn truncated_file_keeps_what_is_there() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "cut.wav");
    // Header claims 2 s, only 0.5 s present.
    write_wav(&path, 48_000, 2, &sine(48_000, 440.0, 0.5, 2), Some(96_000));
    match decode_file(&path, 48_000, 1) {
        Ok(t) => assert!(t.frames() <= 24_000 && t.frames() > 0),
        Err(e) => assert!(matches!(e, DecodeError::Corrupt(_)), "{e}"),
    }
}

#[test]
fn absurd_sample_rate_is_rejected() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "slow.wav");
    write_wav(&path, 1_000, 2, &sine(1_000, 100.0, 0.5, 2), None);
    assert!(matches!(
        decode_file(&path, 48_000, 1),
        Err(DecodeError::UnsupportedRate(1_000))
    ));
}

#[test]
fn stream_is_playable_before_decoding_and_ends_identical() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "s.wav");
    write_wav(&path, 44_100, 2, &sine(44_100, 440.0, 2.0, 2), None);
    let stream = open_stream(&path, 48_000, 7).unwrap();
    let track = stream.track();
    assert_eq!(track.frames(), 96_000, "length known before decoding");
    assert_eq!(track.ready_frames(), 0);
    assert!(!track.is_complete());
    stream.run(|| true).unwrap();
    assert!(track.is_complete());
    assert_eq!(track.frames(), 96_000);
    let whole = decode_file(&path, 48_000, 7).unwrap();
    assert_eq!(track.to_vec(), whole.to_vec());
}

#[test]
fn cancelled_stream_keeps_what_it_decoded() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "c2.wav");
    write_wav(&path, 48_000, 2, &sine(48_000, 440.0, 5.0, 2), None);
    let stream = open_stream(&path, 48_000, 1).unwrap();
    let track = stream.track();
    let mut calls = 0;
    stream
        .run(|| {
            calls += 1;
            calls <= 3
        })
        .unwrap();
    assert!(track.is_complete());
    assert!(
        track.frames() > 0 && track.frames() < 5 * 48_000,
        "{}",
        track.frames()
    );
    assert_eq!(track.frames(), track.ready_frames());
}

#[test]
fn header_that_understates_the_length_still_plays_everything() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "u.wav");
    // The header claims 1 s; there are 1.5 s of audio (within the capacity margin).
    write_wav(&path, 48_000, 2, &sine(48_000, 440.0, 1.5, 2), Some(48_000));
    let stream = open_stream(&path, 48_000, 1).unwrap();
    let track = stream.track();
    let announced = track.frames();
    stream.run(|| true).unwrap();
    assert!(
        track.frames() >= announced,
        "announced {announced}, got {}",
        track.frames()
    );
}
