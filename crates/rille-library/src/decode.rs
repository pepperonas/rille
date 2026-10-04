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
    #[error("Abgebrochen")]
    Cancelled,
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

/// Receives (source rate, interleaved stereo block).
type BlockSink<'a> = dyn FnMut(u32, &[f32]) -> Result<(), DecodeError> + 'a;

/// An opened audio file that yields interleaved stereo blocks at its own rate.
struct Source {
    format: Box<dyn FormatReader>,
    decoder: Box<dyn AudioDecoder>,
    track_id: u32,
    rate: u32,
    /// Frames the header announces, if it does.
    announced: Option<u64>,
    /// Source frames read so far (for the length limits).
    frames: u64,
    max_frames: u64,
    decode_errors: usize,
    path: PathBuf,
    /// Reused per packet.
    scratch: Vec<f32>,
}

impl Source {
    /// Probe the header. Fails fast on unreadable, empty, unsupported or oversized files.
    fn open(path: &Path) -> Result<Source, DecodeError> {
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
        let announced = track.num_frames.filter(|&n| n > 0);
        let params = track
            .codec_params
            .as_ref()
            .and_then(|p| p.audio())
            .ok_or(DecodeError::NoAudioTrack)?
            .clone();
        let decoder = symphonia::default::get_codecs()
            .make_audio_decoder(&params, &AudioDecoderOptions::default())
            .map_err(|e| DecodeError::UnsupportedCodec(e.to_string()))?;
        let rate = params.sample_rate.unwrap_or(0);
        let mut source = Source {
            format,
            decoder,
            track_id,
            rate,
            announced,
            frames: 0,
            max_frames: 0,
            decode_errors: 0,
            path: path.to_path_buf(),
            scratch: Vec::new(),
        };
        if (MIN_RATE..=MAX_RATE).contains(&rate) {
            source.max_frames = max_frames(rate);
            if announced.is_some_and(|n| n > source.max_frames) {
                return Err(DecodeError::TooLong);
            }
        }
        Ok(source)
    }

    /// Decode the next packet of our track into `out` as interleaved stereo. `false` at the end.
    fn next_stereo(&mut self, out: &mut Vec<f32>) -> Result<bool, DecodeError> {
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
                self.decode_errors += 1;
                return Ok(true);
            }
            Err(e) => return Err(DecodeError::Corrupt(e.to_string())),
        };
        let spec = buffer.spec();
        if self.rate == 0 {
            self.rate = spec.rate();
        }
        if !(MIN_RATE..=MAX_RATE).contains(&self.rate) {
            return Err(DecodeError::UnsupportedRate(self.rate));
        }
        if self.max_frames == 0 {
            self.max_frames = max_frames(self.rate);
        }
        let channels = spec.channels().count().max(1);
        self.scratch.resize(buffer.samples_interleaved(), 0.0);
        buffer.copy_to_slice_interleaved(&mut self.scratch);
        append_as_stereo(out, &self.scratch, channels);
        // The header may understate the length; the limit holds regardless.
        self.frames += (self.scratch.len() / channels) as u64;
        if self.frames > self.max_frames {
            return Err(DecodeError::TooLong);
        }
        Ok(true)
    }

    /// Feed every block (with the source rate) to `sink` until the end or until `keep_going`
    /// says stop. Fails if not a single frame could be decoded.
    fn pump(
        &mut self,
        keep_going: &mut dyn FnMut() -> bool,
        sink: &mut BlockSink<'_>,
    ) -> Result<(), DecodeError> {
        let mut stereo = Vec::new();
        let mut produced = false;
        while keep_going() {
            stereo.clear();
            if !self.next_stereo(&mut stereo)? {
                break;
            }
            if !stereo.is_empty() {
                produced = true;
                sink(self.rate, &stereo)?;
            }
        }
        if self.decode_errors > 0 {
            tracing::warn!(path = %self.path.display(), errors = self.decode_errors, "skipped damaged frames");
        }
        if !produced && self.decode_errors > 0 {
            return Err(DecodeError::Corrupt(format!(
                "{} Frames nicht dekodierbar",
                self.decode_errors
            )));
        }
        Ok(())
    }
}

/// Stream a whole file at its own sample rate into `sink` (rate, interleaved stereo block)
/// without keeping it in memory: what the analysis uses. Returns the frames read.
pub fn read_blocks(
    path: &Path,
    mut keep_going: impl FnMut() -> bool,
    mut sink: impl FnMut(u32, &[f32]),
) -> Result<u64, DecodeError> {
    let mut source = Source::open(path)?;
    source.pump(&mut keep_going, &mut |rate, stereo| {
        sink(rate, stereo);
        Ok(())
    })?;
    if source.frames == 0 {
        return Err(DecodeError::Empty);
    }
    Ok(source.frames)
}

/// A file opened for decoding into a [`TrackAudio`] that can already be played. Opening only
/// probes the header (milliseconds); [`DecodeStream::run`] then fills the track front to back.
pub struct DecodeStream {
    source: Source,
    track: Arc<TrackAudio>,
    resampler: Option<StreamResampler>,
    written: usize,
}

/// Open `path` for streaming decode at `target_rate`. Fails fast on unreadable, empty,
/// unsupported or oversized files. A file whose length the header does not state is decoded
/// completely here, so the returned track is complete.
pub fn open_stream(path: &Path, target_rate: u32, id: u64) -> Result<DecodeStream, DecodeError> {
    let source = Source::open(path)?;
    let mut stream = DecodeStream {
        track: Arc::new(TrackAudio::new(id, target_rate, Vec::new())),
        resampler: None,
        written: 0,
        source,
    };
    let rate = stream.source.rate;
    match stream.source.announced {
        Some(frames) if (MIN_RATE..=MAX_RATE).contains(&rate) => {
            let expected = converted_len(frames as usize, rate, target_rate);
            let margin = (expected as f64 * CAPACITY_MARGIN) as usize
                + (CAPACITY_MARGIN_SECONDS * u64::from(target_rate)) as usize;
            let capacity = (expected + margin).min(max_frames(target_rate) as usize);
            stream.track = Arc::new(TrackAudio::streaming(id, target_rate, expected, capacity));
            if rate != target_rate {
                stream.resampler = Some(
                    StreamResampler::new(rate, target_rate)
                        .map_err(|e| DecodeError::Resample(e.to_string()))?,
                );
            }
            Ok(stream)
        }
        // Length or rate unknown: nothing to size the buffer by, decode it all now.
        _ => {
            let mut stereo = Vec::new();
            stream.source.pump(&mut || true, &mut |_, block| {
                stereo.extend_from_slice(block);
                Ok(())
            })?;
            if stereo.is_empty() {
                return Err(DecodeError::Empty);
            }
            let rate = stream.source.rate;
            if !(MIN_RATE..=MAX_RATE).contains(&rate) {
                return Err(DecodeError::UnsupportedRate(rate));
            }
            let samples = if rate == target_rate {
                stereo
            } else {
                resample_stereo(&stereo, rate, target_rate)
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
        let (track, written, resampler) = (&self.track, &mut self.written, &mut self.resampler);
        let result = self
            .source
            .pump(&mut keep_going, &mut |_, stereo| match resampler.as_mut() {
                Some(r) => r
                    .push(stereo, &mut |out| *written += track.write(*written, out))
                    .map_err(|e| DecodeError::Resample(e.to_string())),
                None => {
                    *written += track.write(*written, stereo);
                    Ok(())
                }
            });
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
