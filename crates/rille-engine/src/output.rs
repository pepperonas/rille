//! CoreAudio output via cpal: device listing, stream creation, latency and xrun detection.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, ErrorKind, OutputCallbackInfo, StreamConfig, StreamInstant};

use crate::EngineSlot;

/// Buffer sizes offered in the settings.
pub const BUFFER_SIZES: [u32; 6] = [64, 128, 256, 512, 1024, 2048];
pub const DEFAULT_BUFFER: u32 = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    /// Stable identifier (cpal `DeviceId` as string), used to reopen the device.
    pub id: String,
    pub name: String,
    pub max_channels: u16,
    pub is_default: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputRequest {
    /// `None` = system default output.
    pub device_id: Option<String>,
    pub buffer_frames: u32,
    pub sample_rate: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamInfo {
    pub device_id: String,
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Buffer size actually in use; `None` when the device refused a fixed size.
    pub buffer_frames: Option<u32>,
}

/// Values the audio callback publishes for the UI.
#[derive(Debug, Default)]
pub struct StreamStats {
    /// Time from callback to the samples reaching the DAC, as reported by CoreAudio.
    pub device_latency_us: AtomicU32,
    /// Frames delivered in the most recent callback.
    pub last_callback_frames: AtomicU32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamFailure {
    /// The device disappeared (unplugged, switched off).
    DeviceGone,
    Other(String),
}

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("Kein Audio-Ausgabegerät gefunden")]
    NoDevice,
    #[error("Audio-Gerät „{0}“ ist nicht verfügbar")]
    DeviceNotFound(String),
    #[error("Das Gerät unterstützt {wanted} Hz nicht (möglich: {supported:?})")]
    UnsupportedRate { wanted: u32, supported: Vec<u32> },
    #[error("Audio-Fehler: {0}")]
    Backend(String),
}

/// A running output stream. Dropping it stops audio for this device.
pub struct OutputStream {
    _stream: cpal::Stream,
    pub info: StreamInfo,
    pub stats: Arc<StreamStats>,
}

fn device_name(device: &cpal::Device) -> String {
    device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "Unbekanntes Gerät".into())
}

fn device_id(device: &cpal::Device) -> Option<String> {
    device.id().ok().map(|id| id.to_string())
}

pub fn list_devices() -> Vec<DeviceInfo> {
    let host = cpal::default_host();
    let default_id = host.default_output_device().and_then(|d| device_id(&d));
    let Ok(devices) = host.output_devices() else {
        return Vec::new();
    };
    devices
        .filter_map(|device| {
            let id = device_id(&device)?;
            let max_channels = device
                .supported_output_configs()
                .ok()?
                .map(|c| c.channels())
                .max()
                .unwrap_or(0);
            if max_channels < 2 {
                return None;
            }
            Some(DeviceInfo {
                is_default: default_id.as_deref() == Some(id.as_str()),
                name: device_name(&device),
                id,
                max_channels,
            })
        })
        .collect()
}

/// Supported (channels, min rate, max rate) of a device, reduced to plain numbers for testing.
pub type RateRange = (u16, u32, u32);

/// Pick the channel count for `rate`: the largest stereo-or-wider configuration supporting it.
pub fn choose_channels(ranges: &[RateRange], rate: u32) -> Option<u16> {
    ranges
        .iter()
        .filter(|(ch, min, max)| *ch >= 2 && (*min..=*max).contains(&rate))
        .map(|(ch, _, _)| *ch)
        .max()
}

/// Common rates the device supports, for error messages and engine re-creation.
pub fn supported_rates(ranges: &[RateRange]) -> Vec<u32> {
    [44_100, 48_000, 88_200, 96_000]
        .into_iter()
        .filter(|r| choose_channels(ranges, *r).is_some())
        .collect()
}

/// A callback that arrives this much later than its buffer length counts as an xrun.
const XRUN_FACTOR: f64 = 1.5;

/// Pure xrun check, used by the callback and in tests.
pub fn is_xrun(gap: Duration, frames: usize, sample_rate: u32) -> bool {
    if frames == 0 || sample_rate == 0 {
        return false;
    }
    let expected = frames as f64 / f64::from(sample_rate);
    gap.as_secs_f64() > expected * XRUN_FACTOR
}

fn find_device(host: &cpal::Host, id: Option<&str>) -> Result<cpal::Device, OutputError> {
    match id {
        None => host.default_output_device().ok_or(OutputError::NoDevice),
        Some(id) => {
            let parsed: cpal::DeviceId = id
                .parse()
                .map_err(|_| OutputError::DeviceNotFound(id.to_string()))?;
            host.device_by_id(&parsed)
                .ok_or_else(|| OutputError::DeviceNotFound(id.to_string()))
        }
    }
}

/// Open an output stream that renders `slot`'s engine. `on_failure` is called (from a CoreAudio
/// thread, not the render thread) when the stream breaks.
pub fn open(
    slot: &EngineSlot,
    request: &OutputRequest,
    on_failure: impl FnMut(StreamFailure) + Send + 'static,
) -> Result<OutputStream, OutputError> {
    let host = cpal::default_host();
    let device = find_device(&host, request.device_id.as_deref())?;
    let ranges: Vec<RateRange> = device
        .supported_output_configs()
        .map_err(|e| OutputError::Backend(e.to_string()))?
        .map(|c| (c.channels(), c.min_sample_rate(), c.max_sample_rate()))
        .collect();
    let channels = choose_channels(&ranges, request.sample_rate).ok_or_else(|| {
        OutputError::UnsupportedRate {
            wanted: request.sample_rate,
            supported: supported_rates(&ranges),
        }
    })?;

    let stats = Arc::new(StreamStats::default());

    let build = |buffer_size: BufferSize, on_failure: Box<dyn FnMut(StreamFailure) + Send>| {
        let config = StreamConfig {
            channels,
            sample_rate: request.sample_rate,
            buffer_size,
        };
        let slot = slot.clone();
        let stats = stats.clone();
        let channel_count = usize::from(channels);
        let sample_rate = request.sample_rate;
        let mut last_callback: Option<StreamInstant> = None;
        let mut on_failure = on_failure;
        device.build_output_stream::<f32, _, _>(
            config,
            move |data: &mut [f32], info: &OutputCallbackInfo| {
                let ts = info.timestamp();
                let frames = data.len() / channel_count;
                let latency = ts.playback.saturating_duration_since(ts.callback);
                stats.device_latency_us.store(
                    u32::try_from(latency.as_micros()).unwrap_or(u32::MAX),
                    Ordering::Relaxed,
                );
                stats
                    .last_callback_frames
                    .store(frames as u32, Ordering::Relaxed);
                let xrun = last_callback
                    .map(|prev| {
                        is_xrun(
                            ts.callback.saturating_duration_since(prev),
                            frames,
                            sample_rate,
                        )
                    })
                    .unwrap_or(false);
                last_callback = Some(ts.callback);
                let rendered = slot.try_with(|engine| {
                    if xrun {
                        engine.note_xrun();
                    }
                    render_guarded(engine, data, channel_count);
                });
                if rendered.is_none() {
                    data.fill(0.0);
                }
            },
            move |err: cpal::Error| {
                on_failure(match err.kind() {
                    ErrorKind::DeviceNotAvailable => StreamFailure::DeviceGone,
                    _ => StreamFailure::Other(err.to_string()),
                })
            },
            None,
        )
    };

    // The failure callback is moved into whichever stream build succeeds; share it so a failed
    // fixed-size attempt does not consume it.
    let shared = Arc::new(std::sync::Mutex::new(on_failure));
    let make_cb = || {
        let shared = shared.clone();
        Box::new(move |f: StreamFailure| {
            // Runs on a CoreAudio notification thread, never on the render thread.
            if let Ok(mut cb) = shared.lock() {
                cb(f);
            }
        }) as Box<dyn FnMut(StreamFailure) + Send>
    };

    let wanted = request.buffer_frames.clamp(16, 4096);
    let (stream, buffer_frames) = match build(BufferSize::Fixed(wanted), make_cb()) {
        Ok(stream) => (stream, Some(wanted)),
        Err(_) => (
            build(BufferSize::Default, make_cb())
                .map_err(|e| OutputError::Backend(e.to_string()))?,
            None,
        ),
    };
    stream
        .play()
        .map_err(|e| OutputError::Backend(e.to_string()))?;

    Ok(OutputStream {
        _stream: stream,
        info: StreamInfo {
            device_id: device_id(&device).unwrap_or_default(),
            device_name: device_name(&device),
            sample_rate: request.sample_rate,
            channels,
            buffer_frames,
        },
        stats,
    })
}

#[inline]
fn render_guarded(engine: &mut crate::Engine, data: &mut [f32], channels: usize) {
    #[cfg(debug_assertions)]
    assert_no_alloc::assert_no_alloc(|| engine.process(data, channels));
    #[cfg(not(debug_assertions))]
    engine.process(data, channels);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefers_widest_config_with_the_rate() {
        let ranges = [
            (2, 44_100, 96_000),
            (4, 44_100, 48_000),
            (1, 8_000, 192_000),
        ];
        assert_eq!(choose_channels(&ranges, 48_000), Some(4));
        assert_eq!(choose_channels(&ranges, 96_000), Some(2));
        assert_eq!(
            choose_channels(&ranges, 192_000),
            None,
            "mono only is not enough"
        );
    }

    #[test]
    fn lists_supported_common_rates() {
        assert_eq!(
            supported_rates(&[(2, 44_100, 48_000)]),
            vec![44_100, 48_000]
        );
        assert!(supported_rates(&[(1, 44_100, 48_000)]).is_empty());
    }

    #[test]
    fn xrun_threshold() {
        // 256 frames at 48 kHz = 5.33 ms; 1.5x = 8 ms
        assert!(!is_xrun(Duration::from_micros(5_400), 256, 48_000));
        assert!(!is_xrun(Duration::from_micros(7_900), 256, 48_000));
        assert!(is_xrun(Duration::from_micros(8_100), 256, 48_000));
        assert!(!is_xrun(Duration::from_millis(50), 0, 48_000));
    }
}
