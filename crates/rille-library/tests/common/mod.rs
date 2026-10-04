//! Fixtures shared by the integration tests.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::f32::consts::TAU;
use std::io::Write;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Minimal 16-bit PCM WAV writer for test fixtures. `declared_frames` lets a test lie in the
/// header to simulate truncated files.
pub fn write_wav(
    path: &Path,
    rate: u32,
    channels: u16,
    samples: &[f32],
    declared_frames: Option<u32>,
) {
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

pub fn sine(rate: u32, freq: f32, seconds: f32, channels: usize) -> Vec<f32> {
    let n = (rate as f32 * seconds) as usize;
    (0..n)
        .flat_map(|i| {
            let v = 0.5 * (i as f32 * freq * TAU / rate as f32).sin();
            std::iter::repeat_n(v, channels)
        })
        .collect()
}

pub fn fixture(dir: &TempDir, name: &str) -> PathBuf {
    dir.path().join(name)
}

/// Estimate frequency from rising zero crossings of the left channel.
/// A WAV file with an ID3v2.3 tag in front carrying title and artist. (symphonia 0.6 reads a
/// leading ID3v2 tag for any format; a RIFF `INFO` chunk it parses but drops.)
pub fn write_wav_tagged(path: &Path, rate: u32, samples: &[f32], title: &str, artist: &str) {
    let mut frames = Vec::new();
    for (id, text) in [(b"TIT2", title), (b"TPE1", artist)] {
        let mut body = vec![0u8]; // ISO-8859-1
        body.extend_from_slice(text.as_bytes());
        frames.extend_from_slice(id);
        frames.extend_from_slice(&(body.len() as u32).to_be_bytes());
        frames.extend_from_slice(&[0, 0]);
        frames.extend_from_slice(&body);
    }
    let size = frames.len() as u32;
    let synchsafe = [
        ((size >> 21) & 0x7f) as u8,
        ((size >> 14) & 0x7f) as u8,
        ((size >> 7) & 0x7f) as u8,
        (size & 0x7f) as u8,
    ];
    let wav = path.with_extension("tmp.wav");
    write_wav(&wav, rate, 2, samples, None);
    let mut bytes = b"ID3\x03\x00\x00".to_vec();
    bytes.extend_from_slice(&synchsafe);
    bytes.extend_from_slice(&frames);
    bytes.extend_from_slice(&std::fs::read(&wav).unwrap());
    std::fs::remove_file(&wav).unwrap();
    std::fs::write(path, bytes).unwrap();
}
