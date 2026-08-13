use std::f32::consts::PI;

use rustfft::{FftPlanner, num_complex::Complex};

use super::AudioClip;
use crate::speech::{config::AudioValidationConfig, run::AudioValidationResult};

pub fn mono_samples(clip: &AudioClip) -> Vec<f32> {
    let channels = usize::from(clip.channels());
    if channels == 1 {
        return clip.samples().to_vec();
    }
    clip.samples()
        .chunks_exact(channels)
        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
        .collect()
}

pub fn frame_rms(samples: &[f32], sample_rate: u32, frame_ms: f32, hop_ms: f32) -> Vec<f32> {
    let frame_size = ((sample_rate as f32 * frame_ms / 1000.0) as usize).max(1);
    let hop_size = ((sample_rate as f32 * hop_ms / 1000.0) as usize).max(1);
    if samples.len() < frame_size {
        return vec![rms(samples)];
    }
    let count = 1 + (samples.len() - frame_size) / hop_size;
    (0..count)
        .map(|index| rms(&samples[index * hop_size..index * hop_size + frame_size]))
        .collect()
}

pub fn spectral_flatness(samples: &[f32], sample_rate: u32) -> f32 {
    let frame_size = ((sample_rate as f32 * 0.04) as usize).max(256);
    let mut frame = vec![0.0; frame_size];
    if samples.len() < frame_size {
        frame[..samples.len()].copy_from_slice(samples);
    } else {
        let start = (samples.len() - frame_size) / 2;
        frame.copy_from_slice(&samples[start..start + frame_size]);
    }

    let denominator = (frame_size.saturating_sub(1)).max(1) as f32;
    let mut spectrum: Vec<Complex<f32>> = frame
        .into_iter()
        .enumerate()
        .map(|(index, sample)| {
            let window = 0.5 - 0.5 * (2.0 * PI * index as f32 / denominator).cos();
            Complex::new(sample * window, 0.0)
        })
        .collect();
    FftPlanner::new()
        .plan_fft_forward(frame_size)
        .process(&mut spectrum);

    let magnitudes: Vec<f32> = spectrum[..=frame_size / 2]
        .iter()
        .map(|value| value.norm() + 1e-12)
        .collect();
    let arithmetic = magnitudes.iter().sum::<f32>() / magnitudes.len() as f32;
    let geometric =
        (magnitudes.iter().map(|value| value.ln()).sum::<f32>() / magnitudes.len() as f32).exp();
    geometric / arithmetic.max(1e-12)
}

pub fn zero_crossing_rate(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let crossings = samples
        .windows(2)
        .filter(|pair| pair[0].is_sign_negative() != pair[1].is_sign_negative())
        .count();
    crossings as f32 / (samples.len() - 1) as f32
}

pub fn validate_audio(clip: &AudioClip, config: AudioValidationConfig) -> AudioValidationResult {
    let samples = mono_samples(clip);
    let duration_sec = clip.duration_sec();
    let peak = samples
        .iter()
        .map(|sample| sample.abs())
        .fold(0.0, f32::max);
    let rms = rms(&samples);
    let rms_values = if samples.is_empty() {
        Vec::new()
    } else {
        frame_rms(&samples, clip.sample_rate(), 25.0, 10.0)
    };
    let active_threshold = config.min_rms.max(rms * 0.2);
    let active_ratio = if rms_values.is_empty() {
        0.0
    } else {
        rms_values
            .iter()
            .filter(|value| **value > active_threshold)
            .count() as f32
            / rms_values.len() as f32
    };
    let flatness = if samples.is_empty() {
        1.0
    } else {
        spectral_flatness(&samples, clip.sample_rate())
    };
    let zcr = zero_crossing_rate(&samples);

    let (ok, reason) = if samples.is_empty() {
        (false, "generated no samples")
    } else if duration_sec < config.min_duration_sec {
        (false, "generated audio is too short")
    } else if peak <= config.min_rms || rms <= config.min_rms {
        (false, "generated audio is silent or nearly silent")
    } else if active_ratio < config.min_active_ratio {
        (false, "generated audio is mostly silence")
    } else if flatness >= config.noise_flatness
        && zcr >= config.noise_zcr
        && active_ratio >= config.noise_active_ratio
    {
        (false, "generated audio looks like broadband noise")
    } else {
        (true, "audio passed validation")
    };

    AudioValidationResult {
        ok,
        reason: reason.into(),
        duration_sec,
        rms,
        peak,
        active_ratio,
        spectral_flatness: flatness,
        zero_crossing_rate: zcr,
    }
}

pub(super) fn rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    (samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32).sqrt()
}
