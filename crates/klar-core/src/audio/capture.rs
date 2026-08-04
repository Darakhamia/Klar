//! Microphone capture.
//!
//! The stream runs on its own thread — `cpal::Stream` is not `Send` on every
//! backend — and pushes raw device-format frames down a channel. Conversion to
//! 16 kHz mono happens on the consumer side rather than in the audio callback:
//! the callback runs on a realtime thread and must not do work it can avoid.
//!
//! Nothing here writes to disk. Audio only reaches a file through
//! [`super::wav`], and only when the caller explicitly asks.

use super::{AudioError, SAMPLE_RATE, downmix_to_mono, resample_mono};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

/// An input device the user could pick in settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Device {
    pub name: String,
    pub is_default: bool,
}

/// Everything the caller gets to choose about capture. Deliberately small.
#[derive(Debug, Clone, Default)]
pub struct CaptureConfig {
    /// Device name as reported by [`capture_devices`]. `None` means the system
    /// default, which is what the app uses unless the user overrode it.
    pub device: Option<String>,
}

/// The format the device actually gave us, which is rarely the one we want.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl SourceFormat {
    /// True when the device is already handing us exactly what whisper wants,
    /// so the conversion below is a memcpy.
    pub const fn is_whisper_ready(&self) -> bool {
        self.sample_rate == SAMPLE_RATE && self.channels == 1
    }
}

/// Interleaved device-format frames → 16 kHz mono.
pub fn to_whisper_input(interleaved: &[f32], format: SourceFormat) -> Result<Vec<f32>, AudioError> {
    let mono = downmix_to_mono(interleaved, format.channels);
    resample_mono(&mono, format.sample_rate)
}

/// List the input devices, default first.
pub fn capture_devices() -> Result<Vec<Device>, AudioError> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.description().ok())
        .map(|d| d.name().to_owned());

    let devices = host
        .input_devices()
        .map_err(|e| AudioError::NoConfig(e.to_string()))?;

    let mut listed: Vec<Device> = devices
        .filter_map(|device| device.description().ok())
        .map(|description| {
            let name = description.name().to_owned();
            let is_default = default_name.as_deref() == Some(name.as_str());
            Device { name, is_default }
        })
        .collect();

    listed.sort_by_key(|d| (!d.is_default, d.name.clone()));
    Ok(listed)
}

/// A running capture. Dropping it stops the stream.
pub struct Capture {
    format: SourceFormat,
    device_name: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Capture {
    /// Open the device and start streaming.
    ///
    /// The returned receiver yields interleaved frames in the device's own
    /// format; pass them through [`to_whisper_input`] before doing anything
    /// with them.
    pub fn start(config: &CaptureConfig) -> Result<(Self, Receiver<Vec<f32>>), AudioError> {
        let host = cpal::default_host();

        let device = match &config.device {
            None => host.default_input_device().ok_or(AudioError::NoDevice)?,
            Some(wanted) => host
                .input_devices()
                .map_err(|e| AudioError::NoConfig(e.to_string()))?
                .find(|d| d.description().is_ok_and(|desc| desc.name() == wanted))
                .ok_or_else(|| AudioError::UnknownDevice {
                    name: wanted.clone(),
                })?,
        };

        let device_name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| "unknown".to_owned());

        let supported = pick_config(&device)?;
        let format = SourceFormat {
            sample_rate: supported.sample_rate(),
            channels: supported.channels(),
        };
        let sample_format = supported.sample_format();
        let stream_config = supported.config();

        let (tx, rx) = channel::<Vec<f32>>();
        let (ready_tx, ready_rx) = channel::<Result<(), AudioError>>();

        let stop = Arc::new(AtomicBool::new(false));
        let stop_in_thread = Arc::clone(&stop);

        // The stream is built, played and dropped entirely on this thread.
        let thread = std::thread::Builder::new()
            .name("klar-capture".into())
            .spawn(move || {
                let stream = match build_stream(&device, &stream_config, sample_format, tx) {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        stream
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                        return;
                    }
                };

                if let Err(error) = stream.play() {
                    tracing::error!(%error, "could not start the input stream");
                    return;
                }

                while !stop_in_thread.load(Ordering::Relaxed) {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }

                // Explicit, so the order against the channel drop is obvious.
                drop(stream);
            })
            .map_err(|e| AudioError::Stream(e.to_string()))?;

        match ready_rx.recv() {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(_) => return Err(AudioError::Disconnected),
        }

        tracing::info!(
            device = %device_name,
            sample_rate = format.sample_rate,
            channels = format.channels,
            whisper_ready = format.is_whisper_ready(),
            "capture started"
        );

        Ok((
            Self {
                format,
                device_name,
                stop,
                thread: Some(thread),
            },
            rx,
        ))
    }

    pub const fn format(&self) -> SourceFormat {
        self.format
    }

    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Stop the stream and wait for the capture thread to finish.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            // A capture thread that will not join is not worth crashing over:
            // this path is reachable from a hotkey release.
            if thread.join().is_err() {
                tracing::error!("the capture thread panicked");
            }
        }
    }
}

impl Drop for Capture {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Drain everything queued so far without blocking. Returns `None` once the
/// capture thread is gone.
pub fn drain(rx: &Receiver<Vec<f32>>, into: &mut Vec<f32>) -> Option<()> {
    loop {
        match rx.try_recv() {
            Ok(chunk) => into.extend_from_slice(&chunk),
            Err(TryRecvError::Empty) => return Some(()),
            Err(TryRecvError::Disconnected) => return None,
        }
    }
}

/// Prefer a configuration that is already 16 kHz mono — then capture costs
/// nothing beyond the copy. Otherwise take the device's default and resample.
fn pick_config(device: &cpal::Device) -> Result<cpal::SupportedStreamConfig, AudioError> {
    let supported = device
        .supported_input_configs()
        .map_err(|e| AudioError::NoConfig(e.to_string()))?;

    let mut best: Option<cpal::SupportedStreamConfig> = None;
    for range in supported {
        if range.channels() != 1 {
            continue;
        }
        if !matches!(
            range.sample_format(),
            cpal::SampleFormat::F32 | cpal::SampleFormat::I16
        ) {
            continue;
        }
        if range.min_sample_rate() <= SAMPLE_RATE && SAMPLE_RATE <= range.max_sample_rate() {
            best = Some(range.with_sample_rate(SAMPLE_RATE));
            break;
        }
    }

    match best {
        Some(config) => Ok(config),
        None => device
            .default_input_config()
            .map_err(|e| AudioError::NoConfig(e.to_string())),
    }
}

fn build_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: cpal::SampleFormat,
    tx: Sender<Vec<f32>>,
) -> Result<cpal::Stream, AudioError> {
    // A dropped receiver means the consumer went away; that is a normal end of
    // capture, not something to log on every callback.
    let on_error = |error: cpal::Error| tracing::error!(%error, "input stream error");

    let stream = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            *config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                let _ = tx.send(data.to_vec());
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            *config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                let _ = tx.send(data.iter().map(|s| f32::from(*s) / 32_768.0).collect());
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            *config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let _ = tx.send(
                    data.iter()
                        .map(|s| (f32::from(*s) - 32_768.0) / 32_768.0)
                        .collect(),
                );
            },
            on_error,
            None,
        ),
        other => {
            return Err(AudioError::NoConfig(format!(
                "unsupported sample format {other:?}"
            )));
        }
    };

    stream.map_err(|e| AudioError::Stream(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whisper_ready_means_exactly_16k_mono() {
        assert!(
            SourceFormat {
                sample_rate: 16_000,
                channels: 1
            }
            .is_whisper_ready()
        );
        assert!(
            !SourceFormat {
                sample_rate: 48_000,
                channels: 1
            }
            .is_whisper_ready()
        );
        assert!(
            !SourceFormat {
                sample_rate: 16_000,
                channels: 2
            }
            .is_whisper_ready()
        );
    }

    #[test]
    fn conversion_of_a_ready_format_is_a_passthrough() {
        let input = vec![0.1, -0.2, 0.3, 0.4];
        let format = SourceFormat {
            sample_rate: 16_000,
            channels: 1,
        };
        assert_eq!(to_whisper_input(&input, format).unwrap(), input);
    }

    #[test]
    fn conversion_downmixes_then_resamples() {
        // 48 kHz stereo, one second: 96 000 interleaved samples in, ~16 000 out.
        let interleaved: Vec<f32> = (0..96_000).map(|i| (i % 100) as f32 / 100.0).collect();
        let format = SourceFormat {
            sample_rate: 48_000,
            channels: 2,
        };
        let out = to_whisper_input(&interleaved, format).unwrap();
        assert!(
            (out.len() as i32 - 16_000).abs() < 160,
            "got {} samples",
            out.len()
        );
    }

    // Opening a real device needs hardware and a granted permission, so capture
    // itself is exercised by `klar-cli record` on each platform rather than
    // here. Everything above the device — format selection, conversion and the
    // drain loop — is unit-tested.
    #[test]
    fn drain_reports_a_dropped_sender() {
        let (tx, rx) = channel::<Vec<f32>>();
        tx.send(vec![1.0, 2.0]).unwrap();
        let mut buffer = Vec::new();
        assert_eq!(drain(&rx, &mut buffer), Some(()));
        assert_eq!(buffer, vec![1.0, 2.0]);

        drop(tx);
        assert_eq!(drain(&rx, &mut buffer), None);
    }
}
