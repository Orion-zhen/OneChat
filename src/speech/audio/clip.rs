use std::sync::Arc;

use crate::speech::error::SpeechError;

#[derive(Debug, Clone, PartialEq)]
pub struct AudioClip {
    samples: Arc<[f32]>,
    sample_rate: u32,
    channels: u16,
}

impl AudioClip {
    pub fn new(
        samples: impl Into<Arc<[f32]>>,
        sample_rate: u32,
        channels: u16,
    ) -> Result<Self, SpeechError> {
        let samples = samples.into();
        if sample_rate == 0 {
            return Err(SpeechError::audio(
                "audio sample rate must be greater than zero",
            ));
        }
        if channels == 0 {
            return Err(SpeechError::audio(
                "audio must contain at least one channel",
            ));
        }
        if !samples.len().is_multiple_of(usize::from(channels)) {
            return Err(SpeechError::audio(
                "interleaved sample count is not divisible by the channel count",
            ));
        }
        if samples.iter().any(|sample| !sample.is_finite()) {
            return Err(SpeechError::audio(
                "audio samples contain NaN or infinite values",
            ));
        }
        Ok(Self {
            samples,
            sample_rate,
            channels,
        })
    }

    pub fn samples(&self) -> &[f32] {
        &self.samples
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn frames(&self) -> usize {
        self.samples.len() / usize::from(self.channels)
    }

    pub fn duration_sec(&self) -> f32 {
        self.frames() as f32 / self.sample_rate as f32
    }
}
