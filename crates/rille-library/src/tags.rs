//! Title, artist, album and duration without decoding the audio.

use std::fs::File;
use std::path::Path;

use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTag};

use crate::decode::DecodeError;

#[derive(Debug, Clone, PartialEq)]
pub struct TrackInfo {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    /// From the header; `None` if the format does not state it (analysis fills it in).
    pub duration_secs: Option<f64>,
}

/// Read tags and length. Missing tags fall back to the file name, read as `Artist - Title`
/// when it has that shape (how most DJ downloads are named).
///
/// Note: symphonia 0.6 reads ID3v2/ID3v1, Vorbis comments and MP4 tags, but its WAV reader
/// drops a RIFF `INFO` chunk; WAV files without ID3 fall back to the file name.
pub fn read_info(path: &Path) -> Result<TrackInfo, DecodeError> {
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
            symphonia::core::errors::Error::IoError(io) => DecodeError::Io(io),
            _ => DecodeError::UnsupportedFormat,
        })?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(DecodeError::NoAudioTrack)?;
    let rate = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .and_then(|a| a.sample_rate);
    let duration_secs = match (track.num_frames, rate) {
        (Some(frames), Some(rate)) if rate > 0 => Some(frames as f64 / f64::from(rate)),
        _ => None,
    };

    let mut tags = Tags::default();
    let mut metadata = format.metadata();
    if let Some(revision) = metadata.skip_to_latest() {
        tags.take(revision);
    }
    let (name_artist, name_title) = from_file_name(path);
    Ok(TrackInfo {
        title: tags.title.unwrap_or(name_title),
        artist: tags.artist.or(name_artist),
        album: tags.album,
        duration_secs,
    })
}

#[derive(Default)]
struct Tags {
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
}

impl Tags {
    fn take(&mut self, revision: &MetadataRevision) {
        let per_track = revision
            .per_track
            .iter()
            .flat_map(|t| t.metadata.tags.iter());
        for tag in revision.media.tags.iter().chain(per_track) {
            let (slot, value) = match &tag.std {
                Some(StandardTag::TrackTitle(v)) => (&mut self.title, v),
                Some(StandardTag::Artist(v)) => (&mut self.artist, v),
                Some(StandardTag::Album(v)) => (&mut self.album, v),
                _ => continue,
            };
            let value = value.trim();
            if slot.is_none() && !value.is_empty() {
                *slot = Some(value.to_string());
            }
        }
    }
}

/// `"Artist - Title.mp3"` → (Some("Artist"), "Title"); anything else → (None, file stem).
fn from_file_name(path: &Path) -> (Option<String>, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Unbenannt")
        .trim();
    match stem.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => {
            (Some(artist.trim().to_string()), title.trim().to_string())
        }
        _ => (None, stem.to_string()),
    }
}
