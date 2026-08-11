use std::io::Cursor;

use crate::application::attachments::MAX_AUDIO_BYTES;

use super::{MAX_RECORDING_DURATION_MS, RECORDING_SAMPLE_RATE, RecordingLimit};

const WAV_HEADER_BYTES: u64 = 44;
const MAX_RECORDING_SAMPLES: usize =
    (MAX_RECORDING_DURATION_MS as usize / 1_000) * RECORDING_SAMPLE_RATE as usize;

struct LinearResampler {
    step: f64,
    next_position: f64,
    source_index: u64,
    previous: Option<f32>,
}

impl LinearResampler {
    fn new(source_rate: u32) -> Self {
        Self {
            step: f64::from(source_rate) / f64::from(RECORDING_SAMPLE_RATE),
            next_position: 0.0,
            source_index: 0,
            previous: None,
        }
    }

    fn push(&mut self, input: &[f32], mut output: impl FnMut(f32)) {
        for &sample in input {
            let index = self.source_index;
            self.source_index = self.source_index.saturating_add(1);
            let Some(previous) = self.previous.replace(sample) else {
                output(sample);
                self.next_position = self.step;
                continue;
            };
            while self.next_position <= index as f64 {
                let fraction = (self.next_position - (index - 1) as f64) as f32;
                output(previous + (sample - previous) * fraction.clamp(0.0, 1.0));
                self.next_position += self.step;
            }
        }
    }
}

pub(super) struct RecordingBuffer {
    resampler: LinearResampler,
    pcm: Vec<i16>,
    max_samples: usize,
    max_bytes: u64,
}

impl RecordingBuffer {
    pub(super) fn new(source_rate: u32) -> Self {
        Self {
            resampler: LinearResampler::new(source_rate),
            pcm: Vec::new(),
            max_samples: MAX_RECORDING_SAMPLES,
            max_bytes: MAX_AUDIO_BYTES,
        }
    }

    #[cfg(test)]
    fn with_limits(source_rate: u32, max_samples: usize, max_bytes: u64) -> Self {
        Self {
            resampler: LinearResampler::new(source_rate),
            pcm: Vec::new(),
            max_samples,
            max_bytes,
        }
    }

    pub(super) fn push(&mut self, input: &[f32]) -> Option<RecordingLimit> {
        let max_samples = self
            .max_samples
            .min(((self.max_bytes.saturating_sub(WAV_HEADER_BYTES)) / 2) as usize);
        let pcm = &mut self.pcm;
        self.resampler.push(input, |sample| {
            if pcm.len() < max_samples {
                pcm.push(float_to_pcm16(sample));
            }
        });
        if self.pcm.len() < max_samples {
            None
        } else if self.max_samples
            <= ((self.max_bytes.saturating_sub(WAV_HEADER_BYTES)) / 2) as usize
        {
            Some(RecordingLimit::Duration)
        } else {
            Some(RecordingLimit::Size)
        }
    }

    pub(super) fn elapsed_ms(&self) -> u64 {
        (self.pcm.len() as u64 * 1_000).div_ceil(u64::from(RECORDING_SAMPLE_RATE))
    }

    pub(super) fn is_empty(&self) -> bool {
        self.pcm.is_empty()
    }

    pub(super) fn encode_wav(&self) -> Result<Vec<u8>, String> {
        encode_wav(&self.pcm)
    }
}

fn float_to_pcm16(sample: f32) -> i16 {
    let sample = sample.clamp(-1.0, 1.0);
    if sample < 0.0 {
        (sample * -(i16::MIN as f32)).round() as i16
    } else {
        (sample * i16::MAX as f32).round() as i16
    }
}

fn encode_wav(pcm: &[i16]) -> Result<Vec<u8>, String> {
    let mut output = Cursor::new(Vec::new());
    {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: RECORDING_SAMPLE_RATE,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        };
        let mut writer = hound::WavWriter::new(&mut output, spec)
            .map_err(|error| format!("Could not encode voice message: {error}"))?;
        for &sample in pcm {
            writer
                .write_sample(sample)
                .map_err(|error| format!("Could not encode voice message: {error}"))?;
        }
        writer
            .finalize()
            .map_err(|error| format!("Could not finalize voice message: {error}"))?;
    }
    Ok(output.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resamples_to_sixteen_kilohertz() {
        let mut resampler = LinearResampler::new(48_000);
        let mut output = Vec::new();
        resampler.push(&vec![0.25; 48_000], |sample| output.push(sample));
        assert_eq!(output.len(), 16_000);
        assert!(output.iter().all(|sample| (*sample - 0.25).abs() < 0.000_1));
    }

    #[test]
    fn converts_and_encodes_pcm16_mono_wav() {
        assert_eq!(float_to_pcm16(-1.0), i16::MIN);
        assert_eq!(float_to_pcm16(1.0), i16::MAX);
        let wav = encode_wav(&[i16::MIN, 0, i16::MAX]).unwrap();
        let mut reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, RECORDING_SAMPLE_RATE);
        assert_eq!(reader.spec().bits_per_sample, 16);
        assert_eq!(
            reader
                .samples::<i16>()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            [i16::MIN, 0, i16::MAX]
        );
    }

    #[test]
    fn recording_buffer_reports_duration_and_size_limits() {
        let mut duration = RecordingBuffer::with_limits(RECORDING_SAMPLE_RATE, 3, 1_000);
        assert_eq!(
            duration.push(&[0.0, 0.0, 0.0]),
            Some(RecordingLimit::Duration)
        );

        let mut size = RecordingBuffer::with_limits(RECORDING_SAMPLE_RATE, 100, 48);
        assert_eq!(size.push(&[0.0, 0.0]), Some(RecordingLimit::Size));
    }
}
