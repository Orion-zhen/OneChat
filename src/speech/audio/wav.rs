use std::io::Cursor;

use hound::{SampleFormat, WavReader, WavSpec, WavWriter};

use super::AudioClip;
use crate::speech::error::SpeechError;

pub fn decode_wav(bytes: &[u8]) -> Result<AudioClip, SpeechError> {
    if bytes.is_empty() {
        return Err(SpeechError::audio("WAV response is empty"));
    }
    if bytes.len() < 12 || &bytes[..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(SpeechError::audio("audio response is not a RIFF/WAVE file"));
    }

    let mut reader = WavReader::new(Cursor::new(bytes))
        .map_err(|error| SpeechError::audio(format!("could not decode WAV audio: {error}")))?;
    let spec = reader.spec();
    if spec.channels == 0 || spec.sample_rate == 0 {
        return Err(SpeechError::audio("decoded WAV has an invalid format"));
    }

    let samples = match spec.sample_format {
        SampleFormat::Float if spec.bits_per_sample == 32 => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| SpeechError::audio(format!("could not decode WAV audio: {error}")))?,
        SampleFormat::Int if (1..=32).contains(&spec.bits_per_sample) => {
            let scale = 2_f32.powi(i32::from(spec.bits_per_sample) - 1);
            reader
                .samples::<i32>()
                .map(|sample| sample.map(|sample| sample as f32 / scale))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|error| {
                    SpeechError::audio(format!("could not decode WAV audio: {error}"))
                })?
        }
        _ => {
            return Err(SpeechError::audio(format!(
                "unsupported WAV sample format: {:?}/{}-bit",
                spec.sample_format, spec.bits_per_sample
            )));
        }
    };
    if samples.is_empty() {
        return Err(SpeechError::audio("decoded WAV contains no audio frames"));
    }
    AudioClip::new(samples, spec.sample_rate, spec.channels)
}

pub fn encode_wav(clip: &AudioClip) -> Result<Vec<u8>, SpeechError> {
    if clip.frames() == 0 {
        return Err(SpeechError::audio("cannot encode WAV audio with no frames"));
    }
    let mut output = Vec::new();
    {
        let cursor = Cursor::new(&mut output);
        let spec = WavSpec {
            channels: clip.channels(),
            sample_rate: clip.sample_rate(),
            bits_per_sample: 16,
            sample_format: SampleFormat::Int,
        };
        let mut writer = WavWriter::new(cursor, spec)
            .map_err(|error| SpeechError::audio(format!("could not encode WAV audio: {error}")))?;
        for sample in clip.samples() {
            let quantized = (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16;
            writer.write_sample(quantized).map_err(|error| {
                SpeechError::audio(format!("could not encode WAV audio: {error}"))
            })?;
        }
        writer.finalize().map_err(|error| {
            SpeechError::audio(format!("could not finalize WAV audio: {error}"))
        })?;
    }
    Ok(output)
}
