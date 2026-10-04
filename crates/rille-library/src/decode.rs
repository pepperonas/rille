//! Decode an audio file into a [`TrackAudio`]: stereo, interleaved `f32`, at the engine rate.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rille_core::TrackAudio;
use symphonia::core::codecs::audio::{AudioDecoder, AudioDecoderOptions};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, FormatReader, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

use crate::resample::{StreamResampler, converted_len, resample_stereo};

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

/// Extra room over the length the header announces: MP3 lengths are estimates.
const CAPACITY_MARGIN: f64 = 0.02;
const CAPACITY_MARGIN_SECONDS: u64 = 2;

/// Decode `path` completely and resample to `target_rate`. For playback prefer
/// [`open_stream`], which hands the track over before decoding has finished.
pub fn decode_file(path: &Path, target_rate: u32, id: u64) -> Result<Arc<TrackAudio>, DecodeError> {
    let stream = open_stream(path, target_rate, id)?;
    let track = stream.track();
    stream.run(|| true)?;
    Ok(track)
}

/// A file opened for decoding into a [`TrackAudio`] that can already be played. Opening only
/// probes the header (milliseconds); [`DecodeStream::run`] then fills the track front to back.
pub struct DecodeStream {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    source_rate: u32,
    track: Arc<TrackAudio>,
    resampler: Option<StreamResampler>,
    /// Source frames read so far (for the length limits).
    source_frames: u64,
    max_source_frames: u64,
    written: usize,
    path: PathBuf,
    /// Reused per packet.
    scratch: Vec<f32>,
}

/// Open `path` for streaming decode at `target_rate`. Fails fast on unreadable, empty,
/// unsupported or oversized files. A file whose length the header does not state is decoded
/// completely here, so the returned track is complete.
pub fn open_stream(path: &Path, target_rate: u32, id: u64) -> Result<DecodeStream, DecodeError> {
    let file = File::open(path)?;
    if file.metadata()?.len() == 0 {
        return Err(DecodeError::Empty);
    }
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let format = symphonia::default::get_probe()
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
    let announced = track.num_frames;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(DecodeError::NoAudioTrack)?
        .clone();
    let decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default())
        .map_err(|e| DecodeError::UnsupportedCodec(e.to_string()))?;

    let source_rate = params.sample_rate.unwrap_or(0);
    let mut stream = DecodeStream {
        format,
        decoder,
        track_id,
        source_rate,
        track: Arc::new(TrackAudio::new(id, target_rate, Vec::new())),
        resampler: None,
        source_frames: 0,
        max_source_frames: 0,
        written: 0,
        path: path.to_path_buf(),
        scratch: Vec::new(),
    };
    match announced {
        Some(frames) if (MIN_RATE..=MAX_RATE).contains(&source_rate) && frames > 0 => {
            stream.max_source_frames = max_frames(source_rate);
            if frames > stream.max_source_frames {
                return Err(DecodeError::TooLong);
            }
            let expected = converted_len(frames as usize, source_rate, target_rate);
            let margin = (expected as f64 * CAPACITY_MARGIN) as usize
                + (CAPACITY_MARGIN_SECONDS * u64::from(target_rate)) as usize;
            let capacity = (expected + margin).min(max_frames(target_rate) as usize);
            stream.track = Arc::new(TrackAudio::streaming(id, target_rate, expected, capacity));
            if source_rate != target_rate {
                stream.resampler = Some(
                    StreamResampler::new(source_rate, target_rate)
                        .map_err(|e| DecodeError::Resample(e.to_string()))?,
                );
            }
            Ok(stream)
        }
        // Length or rate unknown: nothing to size the buffer by, decode it all now.
        _ => {
            let stereo = stream.decode_all()?;
            let samples = if stream.source_rate == target_rate {
                stereo
            } else {
                resample_stereo(&stereo, stream.source_rate, target_rate)
                    .map_err(|e| DecodeError::Resample(e.to_string()))?
            };
            stream.track = Arc::new(TrackAudio::new(id, target_rate, samples));
            Ok(stream)
        }
    }
}

/// Most source frames accepted at `rate`.
fn max_frames(rate: u32) -> u64 {
    (MAX_SECONDS * u64::from(rate)).min(MAX_FRAMES)
}

impl DecodeStream {
    /// The track being filled; hand it to the engine before calling [`DecodeStream::run`].
    pub fn track(&self) -> Arc<TrackAudio> {
        self.track.clone()
    }

    /// Decode to the end, writing into the track as it goes. Stops early (keeping what was
    /// decoded) when `keep_going` returns false. The track is marked complete in every case,
    /// also on errors, with whatever was decoded up to then.
    pub fn run(mut self, mut keep_going: impl FnMut() -> bool) -> Result<(), DecodeError> {
        if self.track.is_complete() {
            return Ok(()); // decoded completely in `open_stream`
        }
        let result = self.fill(&mut keep_going);
        if let Some(resampler) = self.resampler.take() {
            let (track, written) = (&self.track, &mut self.written);
            let _ = resampler.finish(&mut |out| *written += track.write(*written, out));
        }
        self.track.finish(self.written);
        if self.written == 0 && result.is_ok() {
            return Err(DecodeError::Empty);
        }
        result
    }

    fn fill(&mut self, keep_going: &mut dyn FnMut() -> bool) -> Result<(), DecodeError> {
        let mut stereo = Vec::new();
        let mut decode_errors = 0usize;
        while keep_going() {
            stereo.clear();
            match self.next_stereo(&mut stereo, &mut decode_errors)? {
                false => break,
                true if stereo.is_empty() => continue,
                true => {}
            }
            let (track, written) = (&self.track, &mut self.written);
            match self.resampler.as_mut() {
                Some(r) => r
                    .push(&stereo, &mut |out| *written += track.write(*written, out))
                    .map_err(|e| DecodeError::Resample(e.to_string()))?,
                None => *written += track.write(*written, &stereo),
            }
        }
        if decode_errors > 0 {
            tracing::warn!(path = %self.path.display(), decode_errors, "skipped damaged frames");
        }
        if self.written == 0 && decode_errors > 0 {
            return Err(DecodeError::Corrupt(format!(
                "{decode_errors} Frames nicht dekodierbar"
            )));
        }
        Ok(())
    }

    /// Decode the next packet of our track into `out` as interleaved stereo. `false` at the end.
    fn next_stereo(
        &mut self,
        out: &mut Vec<f32>,
        decode_errors: &mut usize,
    ) -> Result<bool, DecodeError> {
        let packet = match self.format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => return Ok(false),
            // A truncated file ends with an unexpected EOF: keep what we have.
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                return Ok(false);
            }
            Err(SymphoniaError::ResetRequired) => return Ok(false),
            Err(e) => return Err(DecodeError::Corrupt(e.to_string())),
        };
        if packet.track_id != self.track_id {
            return Ok(true);
        }
        let buffer = match self.decoder.decode(&packet) {
            Ok(buffer) => buffer,
            Err(SymphoniaError::DecodeError(_)) => {
                // Single damaged frames are skipped, like every player does.
                *decode_errors += 1;
                return Ok(true);
            }
            Err(e) => return Err(DecodeError::Corrupt(e.to_string())),
        };
        let spec = buffer.spec();
        if self.source_rate == 0 {
            self.source_rate = spec.rate();
        }
        if !(MIN_RATE..=MAX_RATE).contains(&self.source_rate) {
            return Err(DecodeError::UnsupportedRate(self.source_rate));
        }
        if self.max_source_frames == 0 {
            self.max_source_frames = max_frames(self.source_rate);
        }
        let channels = spec.channels().count().max(1);
        self.scratch.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut self.scratch);
        append_as_stereo(out, &self.scratch, channels);
        // The header may understate the length; the limit holds regardless.
        self.source_frames += (self.scratch.len() / channels) as u64;
        if self.source_frames > self.max_source_frames {
            return Err(DecodeError::TooLong);
        }
        Ok(true)
    }

    /// Whole file as interleaved stereo at the source rate (length unknown up front).
    fn decode_all(&mut self) -> Result<Vec<f32>, DecodeError> {
        let mut stereo = Vec::new();
        let mut decode_errors = 0usize;
        while self.next_stereo(&mut stereo, &mut decode_errors)? {}
        if stereo.is_empty() {
            return Err(if decode_errors > 0 {
                DecodeError::Corrupt(format!("{decode_errors} Frames nicht dekodierbar"))
            } else {
                DecodeError::Empty
            });
        }
        if !(MIN_RATE..=MAX_RATE).contains(&self.source_rate) {
            return Err(DecodeError::UnsupportedRate(self.source_rate));
        }
        Ok(stereo)
    }
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
