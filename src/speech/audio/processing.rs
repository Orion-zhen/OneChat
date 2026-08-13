use super::{
    AudioClip,
    analysis::{frame_rms, mono_samples, rms},
};
use crate::speech::{config::AudioValidationConfig, error::SpeechError};

pub fn trim_audio(
    clip: &AudioClip,
    config: AudioValidationConfig,
) -> Result<AudioClip, SpeechError> {
    let mono = mono_samples(clip);
    if mono.is_empty() {
        return Ok(clip.clone());
    }

    let sample_rate = clip.sample_rate();
    let max_silence = (config.trim_max_edge_silence_sec.max(0.0) * sample_rate as f32) as usize;
    let keep_silence = (config.trim_keep_edge_silence_sec.max(0.0) * sample_rate as f32) as usize;
    if max_silence <= keep_silence {
        return Ok(clip.clone());
    }

    let frame_size = ((sample_rate as f32 * 25.0 / 1000.0) as usize).max(1);
    let hop_size = ((sample_rate as f32 * 10.0 / 1000.0) as usize).max(1);
    let rms_values = frame_rms(&mono, sample_rate, 25.0, 10.0);
    let active_threshold = config.min_rms.max(rms(&mono) * 0.15);
    let Some(first_active) = rms_values
        .iter()
        .position(|value| *value > active_threshold)
    else {
        return Ok(clip.clone());
    };
    let last_active = rms_values
        .iter()
        .rposition(|value| *value > active_threshold)
        .unwrap();
    let speech_start = first_active * hop_size;
    let speech_end = clip.frames().min(last_active * hop_size + frame_size);
    let start = if speech_start > max_silence {
        speech_start.saturating_sub(keep_silence)
    } else {
        0
    };
    let end = if clip.frames() - speech_end > max_silence {
        (speech_end + keep_silence).min(clip.frames())
    } else {
        clip.frames()
    };
    if start >= end || (start == 0 && end == clip.frames()) {
        return Ok(clip.clone());
    }

    let channels = usize::from(clip.channels());
    AudioClip::new(
        clip.samples()[start * channels..end * channels].to_vec(),
        sample_rate,
        clip.channels(),
    )
}

pub fn merge_audio(clips: &[AudioClip], min_silence_sec: f32) -> Result<AudioClip, SpeechError> {
    let Some(first) = clips.first() else {
        return Err(SpeechError::audio("audio merge requires at least one clip"));
    };
    for (index, clip) in clips.iter().enumerate() {
        if clip.frames() == 0 {
            return Err(SpeechError::audio(format!(
                "audio clip {} contains no frames",
                index + 1
            )));
        }
        if clip.sample_rate() != first.sample_rate() {
            return Err(SpeechError::audio(format!(
                "audio sample rate mismatch at clip {}: expected {} Hz, got {} Hz",
                index + 1,
                first.sample_rate(),
                clip.sample_rate()
            )));
        }
        if clip.channels() != first.channels() {
            return Err(SpeechError::audio(format!(
                "audio channel mismatch at clip {}: expected {}, got {}",
                index + 1,
                first.channels(),
                clip.channels()
            )));
        }
    }
    if clips.len() == 1 {
        return Ok(first.clone());
    }

    let channels = usize::from(first.channels());
    let minimum_silence = (min_silence_sec.max(0.0) * first.sample_rate() as f32) as usize;
    let mut samples = Vec::new();
    for pair in clips.windows(2) {
        let current = &pair[0];
        let following = &pair[1];
        samples.extend_from_slice(current.samples());
        let trailing = edge_silence(&mono_samples(current), true, 0.01);
        let leading = edge_silence(&mono_samples(following), false, 0.01);
        let missing = minimum_silence.saturating_sub(trailing + leading);
        samples.resize(samples.len() + missing * channels, 0.0);
    }
    samples.extend_from_slice(clips.last().unwrap().samples());
    AudioClip::new(samples, first.sample_rate(), first.channels())
}

fn edge_silence(samples: &[f32], reverse: bool, threshold: f32) -> usize {
    if reverse {
        samples
            .iter()
            .rev()
            .position(|sample| sample.abs() > threshold)
            .unwrap_or(samples.len())
    } else {
        samples
            .iter()
            .position(|sample| sample.abs() > threshold)
            .unwrap_or(samples.len())
    }
}
