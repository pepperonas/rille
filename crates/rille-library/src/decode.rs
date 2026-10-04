//! Decode an audio file into a [`TrackAudio`]: stereo, interleaved `f32`, at the engine rate.

use std::fs::File;
use std::path::Path;

use rille_core::TrackAudio;
use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::resample::resample_stereo;

/// Longest track we accept (1 h at the source rate).
pub const MAX_SECONDS: u64 = 60 * 60;
/// Absolute cap on decoded stereo frames regardless of rate (~1.5 GB as f32). Together with
/// `MAX_SECONDS` this keeps a malicious or broken file from exhausting memory.
pub const MAX_FRAMES: u64 = 200_000_000;
/// Sample rates outside this range are rejected before anything is allocated for them.
pub const MIN_RATE: u32 = 8_000;
pub const MAX_RATE: u32 = 384_000;

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("Datei nicht lesbar: {0}")]
    Io(#[from] std::io::Error),
    #[error("Kein unterstütztes Audioformat")]
    UnsupportedFormat,
    #[error("Die Datei enthält keine Audiospur")]
    NoAudioTrack,
    #[error("Codec wird nicht unterstützt: {0}")]
    UnsupportedCodec(String),
    #[error("Die Datei enthält keine Audiodaten")]
    Empty,
    #[error("Track ist länger als {} Minuten", MAX_SECONDS / 60)]
    TooLong,
    #[error("Abtastrate {0} Hz wird nicht unterstützt")]
    UnsupportedRate(u32),
    #[error("Datei ist beschädigt: {0}")]
    Corrupt(String),
    #[error("Resampling fehlgeschlagen: {0}")]
    Resample(String),
}

/// Decode `path` and resample to `target_rate`. Runs on a worker thread; may take a while.
pub fn decode_file(path: &Path, target_rate: u32, id: u64) -> Result<TrackAudio, DecodeError> {
    let file = File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Err(DecodeError::Empty);
    }
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            mss,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|e| match e {
            SymphoniaError::IoError(io) => DecodeError::Io(io),
            _ => DecodeError::UnsupportedFormat,
        })?;

    let track = format
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::NoAudioTrack)?;
    let track_id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(DecodeError::NoAudioTrack)?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::UnsupportedCodec(e.to_string()))?;

    let mut source_rate = params.sample_rate.unwrap_or(0);
    let mut stereo: Vec<f32> = Vec::new();
    let mut scratch: Vec<f32> = Vec::new();
    let mut decode_errors = 0usize;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            // A truncated file ends with an unexpected EOF: keep what we have.
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(e) => return Err(DecodeError::Corrupt(e.to_string())),
        };
        if packet.track_id != track_id {
            continue;
        }
        let buffer = match decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(SymphoniaError::DecodeError(_)) => {
                // Single damaged frames are skipped, like every player does.
                decode_errors += 1;
                continue;
            }
            Err(e) => return Err(DecodeError::Corrupt(e.to_string())),
        };
        let spec = buffer.spec();
        if source_rate == 0 {
            source_rate = spec.rate();
        }
        let channels = spec.channels().count().max(1);
        scratch.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut scratch);
        append_as_stereo(&mut stereo, &scratch, channels);

        if !(MIN_RATE..=MAX_RATE).contains(&source_rate) {
            return Err(DecodeError::UnsupportedRate(source_rate));
        }
        let max_frames = (MAX_SECONDS * u64::from(source_rate)).min(MAX_FRAMES);
        if (stereo.len() / 2) as u64 > max_frames {
            return Err(DecodeError::TooLong);
        }
    }

    if stereo.is_empty() {
        return Err(if decode_errors > 0 {
            DecodeError::Corrupt(format!("{decode_errors} Frames nicht dekodierbar"))
        } else {
            DecodeError::Empty
        });
    }
    if decode_errors > 0 {
        tracing::warn!(path = %path.display(), decode_errors, "skipped damaged frames");
    }
    if !(MIN_RATE..=MAX_RATE).contains(&source_rate) {
        return Err(DecodeError::UnsupportedRate(source_rate));
    }

    let samples = if source_rate == target_rate {
        stereo
    } else {
        resample_stereo(&stereo, source_rate, target_rate)
            .map_err(|e| DecodeError::Resample(e.to_string()))?
    };
    Ok(TrackAudio::new(id, target_rate, samples))
}

/// Mono is duplicated to both sides; more than two channels keep the first two (front L/R).
fn append_as_stereo(out: &mut Vec<f32>, interleaved: &[f32], channels: usize) {
    match channels {
        1 => out.extend(interleaved.iter().flat_map(|&s| [s, s])),
        2 => out.extend_from_slice(interleaved),
        n => out.extend(interleaved.chunks_exact(n).flat_map(|f| [f[0], f[1]])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mono_is_duplicated() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[0.1, 0.2], 1);
        assert_eq!(out, [0.1, 0.1, 0.2, 0.2]);
    }

    #[test]
    fn surround_keeps_front_pair() {
        let mut out = Vec::new();
        append_as_stereo(&mut out, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 3);
        assert_eq!(out, [1.0, 2.0, 4.0, 5.0]);
    }
}
