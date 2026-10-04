#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test code

use std::f32::consts::TAU;
use std::io::Write;
use std::path::{Path, PathBuf};

use rille_library::{DecodeError, decode_file};
use tempfile::TempDir;

/// Minimal 16-bit PCM WAV writer for test fixtures. `declared_frames` lets a test lie in the
/// header to simulate truncated files.
fn write_wav(path: &Path, rate: u32, channels: u16, samples: &[f32], declared_frames: Option<u32>) {
    let frames = (samples.len() / channels as usize) as u32;
    let data_len = declared_frames.unwrap_or(frames) * u32::from(channels) * 2;
    let mut f = std::fs::File::create(path).unwrap();
    let block_align = channels * 2;
    f.write_all(b"RIFF").unwrap();
    f.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    f.write_all(b"WAVEfmt ").unwrap();
    f.write_all(&16u32.to_le_bytes()).unwrap();
    f.write_all(&1u16.to_le_bytes()).unwrap();
    f.write_all(&channels.to_le_bytes()).unwrap();
    f.write_all(&rate.to_le_bytes()).unwrap();
    f.write_all(&(rate * u32::from(block_align)).to_le_bytes())
        .unwrap();
    f.write_all(&block_align.to_le_bytes()).unwrap();
    f.write_all(&16u16.to_le_bytes()).unwrap();
    f.write_all(b"data").unwrap();
    f.write_all(&data_len.to_le_bytes()).unwrap();
    for s in samples {
        let v = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        f.write_all(&v.to_le_bytes()).unwrap();
    }
}

fn sine(rate: u32, freq: f32, seconds: f32, channels: usize) -> Vec<f32> {
    let n = (rate as f32 * seconds) as usize;
    (0..n)
        .flat_map(|i| {
            let v = 0.5 * (i as f32 * freq * TAU / rate as f32).sin();
            std::iter::repeat_n(v, channels)
        })
        .collect()
}

fn fixture(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

/// Estimate frequency from rising zero crossings of the left channel.
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
    let f = frequency(&t.samples, 48_000);
    assert!((f - 1000.0).abs() < 5.0, "frequency {f}");
}

#[test]
fn mono_becomes_stereo() {
    let dir = TempDir::new().unwrap();
    let path = fixture(&dir, "c.wav");
    write_wav(&path, 48_000, 1, &sine(48_000, 440.0, 0.1, 1), None);
    let t = decode_file(&path, 48_000, 1).unwrap();
    assert_eq!(t.frames(), 4800);
    assert!(t.samples.as_chunks::<2>().0.iter().all(|f| f[0] == f[1]));
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
    assert!(matches!(decode_file(&path, 48_000, 1), Err(DecodeError::UnsupportedRate(1_000))));
}
