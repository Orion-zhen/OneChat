use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

use cpal::{
    Device, FromSample, Sample, SampleFormat, Stream, StreamConfig, SupportedStreamConfig,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

const SAMPLE_QUEUE_CAPACITY: usize = 64;
const CAPTURE_OVERFLOW_ERROR: &str = "Microphone capture fell behind and audio was dropped. Close other audio-heavy apps and try again.";

pub(super) trait ActiveInput: Send {}
impl ActiveInput for Stream {}

pub(super) struct InputSession {
    pub(super) sample_rate: u32,
    pub(super) samples: mpsc::Receiver<Vec<f32>>,
    pub(super) errors: mpsc::Receiver<String>,
    pub(super) _stream: Box<dyn ActiveInput>,
}

pub(super) trait RecordingBackend: Send {
    fn start(&mut self) -> Result<InputSession, String>;
}

#[derive(Default)]
pub(super) struct CpalBackend;

impl RecordingBackend for CpalBackend {
    fn start(&mut self) -> Result<InputSession, String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(no_microphone_error)?;
        let config = device.default_input_config().map_err(|error| {
            microphone_access_error("Could not read the microphone configuration", &error)
        })?;
        let sample_rate = config.sample_rate();
        let channels = usize::from(config.channels());
        if channels == 0 || sample_rate == 0 {
            return Err("The microphone reported an invalid audio configuration.".into());
        }
        let (sample_tx, samples) = mpsc::sync_channel(SAMPLE_QUEUE_CAPACITY);
        let (error_tx, errors) = mpsc::channel();
        let stream = build_input_stream(&device, &config, channels, sample_tx, error_tx)?;
        stream.play().map_err(|error| {
            microphone_access_error("Could not start microphone capture", &error)
        })?;
        Ok(InputSession {
            sample_rate,
            samples,
            errors,
            _stream: Box::new(stream),
        })
    }
}

fn build_input_stream(
    device: &Device,
    config: &SupportedStreamConfig,
    channels: usize,
    samples: mpsc::SyncSender<Vec<f32>>,
    errors: mpsc::Sender<String>,
) -> Result<Stream, String> {
    let stream_config: StreamConfig = config.clone().into();
    let result = match config.sample_format() {
        SampleFormat::I8 => {
            build_typed_input_stream::<i8>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::I16 => {
            build_typed_input_stream::<i16>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::I24 => {
            build_typed_input_stream::<cpal::I24>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::I32 => {
            build_typed_input_stream::<i32>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::I64 => {
            build_typed_input_stream::<i64>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::U8 => {
            build_typed_input_stream::<u8>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::U16 => {
            build_typed_input_stream::<u16>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::U24 => {
            build_typed_input_stream::<cpal::U24>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::U32 => {
            build_typed_input_stream::<u32>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::U64 => {
            build_typed_input_stream::<u64>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::F32 => {
            build_typed_input_stream::<f32>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::F64 => {
            build_typed_input_stream::<f64>(device, &stream_config, channels, samples, errors)
        }
        SampleFormat::DsdU8 | SampleFormat::DsdU16 | SampleFormat::DsdU32 => {
            return Err("DSD microphones are not supported for voice recording.".into());
        }
        format => {
            return Err(format!(
                "The microphone uses unsupported sample format {format}."
            ));
        }
    };
    result.map_err(|error| microphone_access_error("Could not open the microphone", &error))
}

fn build_typed_input_stream<T>(
    device: &Device,
    config: &StreamConfig,
    channels: usize,
    samples: mpsc::SyncSender<Vec<f32>>,
    errors: mpsc::Sender<String>,
) -> Result<Stream, cpal::BuildStreamError>
where
    T: cpal::SizedSample + Copy + Send + 'static,
    f32: FromSample<T>,
{
    let callback_errors = errors.clone();
    let overflow_reported = Arc::new(AtomicBool::new(false));
    let callback_overflow_reported = overflow_reported.clone();
    device.build_input_stream(
        config,
        move |data: &[T], _| {
            let mono = downmix(data, channels);
            if !mono.is_empty() {
                enqueue_samples(
                    &samples,
                    &callback_errors,
                    &callback_overflow_reported,
                    mono,
                );
            }
        },
        move |error| {
            let _ = errors.send(microphone_access_error(
                "Microphone capture stopped",
                &error,
            ));
        },
        None,
    )
}

fn downmix<T>(samples: &[T], channels: usize) -> Vec<f32>
where
    T: Sample + Copy,
    f32: FromSample<T>,
{
    if channels == 0 {
        return Vec::new();
    }
    samples
        .chunks_exact(channels)
        .map(|frame| frame.iter().copied().map(f32::from_sample).sum::<f32>() / channels as f32)
        .collect()
}

fn enqueue_samples(
    samples: &mpsc::SyncSender<Vec<f32>>,
    errors: &mpsc::Sender<String>,
    overflow_reported: &AtomicBool,
    data: Vec<f32>,
) {
    if matches!(samples.try_send(data), Err(mpsc::TrySendError::Full(_)))
        && !overflow_reported.swap(true, Ordering::Relaxed)
    {
        let _ = errors.send(CAPTURE_OVERFLOW_ERROR.into());
    }
}

fn microphone_access_error(context: &str, error: &dyn std::fmt::Display) -> String {
    let detail = error.to_string();
    let normalized = detail.to_ascii_lowercase();
    if normalized.contains("permission")
        || normalized.contains("denied")
        || normalized.contains("not authorized")
        || normalized.contains("not permitted")
        || normalized.contains("0x80070005")
    {
        microphone_permission_error().into()
    } else {
        format!("{context}: {detail} {}", microphone_troubleshooting_hint())
    }
}

#[cfg(target_os = "macos")]
fn no_microphone_error() -> String {
    "No microphone is available. Connect or enable an input device in System Settings and try again."
        .into()
}

#[cfg(target_os = "windows")]
fn no_microphone_error() -> String {
    "No microphone is available. Connect or enable an input device in Windows Sound settings and try again."
        .into()
}

#[cfg(target_os = "linux")]
fn no_microphone_error() -> String {
    "No microphone is available. Configure a default input device in your desktop audio settings and try again."
        .into()
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn no_microphone_error() -> String {
    "No microphone is available. Connect or enable an input device and try again.".into()
}

#[cfg(target_os = "macos")]
fn microphone_permission_error() -> &'static str {
    "Microphone access was denied. Enable it in System Settings and try again."
}

#[cfg(target_os = "windows")]
fn microphone_permission_error() -> &'static str {
    "Microphone access was denied. Enable Microphone access and Let desktop apps access your microphone in Windows Settings, then try again."
}

#[cfg(target_os = "linux")]
fn microphone_permission_error() -> &'static str {
    "Microphone access was denied. Allow OneChat to use the microphone in your desktop or sandbox settings, then try again."
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn microphone_permission_error() -> &'static str {
    "Microphone access was denied. Allow OneChat to use the microphone in system settings and try again."
}

#[cfg(target_os = "macos")]
fn microphone_troubleshooting_hint() -> &'static str {
    "Check the input device in System Settings."
}

#[cfg(target_os = "windows")]
fn microphone_troubleshooting_hint() -> &'static str {
    "Check the input device and microphone privacy controls in Windows Settings."
}

#[cfg(target_os = "linux")]
fn microphone_troubleshooting_hint() -> &'static str {
    "Check the default input device and the PipeWire, PulseAudio, or ALSA session."
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn microphone_troubleshooting_hint() -> &'static str {
    "Check the system input device and audio service."
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downmixes_interleaved_channels_to_mono() {
        let mono = downmix(&[1.0_f32, -1.0, 0.5, 0.25], 2);
        assert_eq!(mono, [0.0, 0.375]);
    }

    #[test]
    fn converts_signed_unsigned_and_float_input_to_f32() {
        assert_eq!(downmix(&[i16::MIN, 0, i16::MAX], 1)[1], 0.0);
        assert_eq!(downmix(&[u16::MIN, 1 << 15, u16::MAX], 1)[1], 0.0);
        assert_eq!(downmix(&[-0.5_f64, 0.5], 1), [-0.5, 0.5]);
    }

    #[test]
    fn reports_capture_overflow_once_instead_of_silently_dropping_audio() {
        let (sample_tx, samples) = mpsc::sync_channel(1);
        let (error_tx, errors) = mpsc::channel();
        let overflow_reported = AtomicBool::new(false);

        enqueue_samples(&sample_tx, &error_tx, &overflow_reported, vec![0.1]);
        enqueue_samples(&sample_tx, &error_tx, &overflow_reported, vec![0.2]);
        enqueue_samples(&sample_tx, &error_tx, &overflow_reported, vec![0.3]);

        assert_eq!(samples.recv().unwrap(), [0.1]);
        assert_eq!(errors.recv().unwrap(), CAPTURE_OVERFLOW_ERROR);
        assert!(errors.try_recv().is_err());
    }

    #[test]
    fn classifies_permission_errors_and_preserves_other_details() {
        assert_eq!(
            microphone_access_error("Could not open the microphone", &"Access is denied"),
            microphone_permission_error()
        );
        let unavailable =
            microphone_access_error("Could not start microphone capture", &"device unavailable");
        assert!(unavailable.contains("device unavailable"));
        assert!(unavailable.contains(microphone_troubleshooting_hint()));
    }
}
