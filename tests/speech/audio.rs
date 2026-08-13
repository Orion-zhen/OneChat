use std::f32::consts::PI;

use onechat::speech::{
    AudioClip, AudioValidationConfig,
    audio::{decode_wav, encode_wav, merge_audio, trim_audio, validate_audio},
};

fn sine(sample_rate: u32, seconds: f32, channels: u16) -> AudioClip {
    let frames = (sample_rate as f32 * seconds) as usize;
    let mut samples = Vec::with_capacity(frames * usize::from(channels));
    for frame in 0..frames {
        let value = (2.0 * PI * 220.0 * frame as f32 / sample_rate as f32).sin() * 0.25;
        samples.extend(std::iter::repeat_n(value, usize::from(channels)));
    }
    AudioClip::new(samples, sample_rate, channels).unwrap()
}

#[test]
fn wav_round_trip_preserves_format_and_signal() {
    let clip = sine(24_000, 0.5, 2);
    let decoded = decode_wav(&encode_wav(&clip).unwrap()).unwrap();
    assert_eq!(decoded.sample_rate(), 24_000);
    assert_eq!(decoded.channels(), 2);
    assert_eq!(decoded.frames(), clip.frames());
    assert!((decoded.samples()[100] - clip.samples()[100]).abs() < 0.0001);
}

#[test]
fn validation_distinguishes_silence_signal_and_noise() {
    let config = AudioValidationConfig::default();
    let silence = AudioClip::new(vec![0.0; 24_000], 24_000, 1).unwrap();
    assert!(!validate_audio(&silence, config).ok);
    assert!(validate_audio(&sine(24_000, 1.0, 1), config).ok);

    let mut state = 1_u32;
    let noise: Vec<f32> = (0..24_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state as f32 / u32::MAX as f32 - 0.5) * 0.8
        })
        .collect();
    let noise = AudioClip::new(noise, 24_000, 1).unwrap();
    assert!(!validate_audio(&noise, config).ok);
}

#[test]
fn trim_preserves_channels_and_keeps_short_edge_silence() {
    let sample_rate = 1_000;
    let mut samples = vec![0.0; 500 * 2];
    samples.extend(sine(sample_rate, 0.5, 2).samples());
    samples.extend(vec![0.0; 500 * 2]);
    let clip = AudioClip::new(samples, sample_rate, 2).unwrap();
    let trimmed = trim_audio(&clip, AudioValidationConfig::default()).unwrap();
    assert_eq!(trimmed.channels(), 2);
    assert!(trimmed.frames() < clip.frames());
    assert!(trimmed.frames() >= 700);
}

#[test]
fn smart_merge_only_adds_missing_silence() {
    let sample_rate = 1_000;
    let first = AudioClip::new(vec![0.2; 300], sample_rate, 1).unwrap();
    let second = AudioClip::new(vec![0.2; 300], sample_rate, 1).unwrap();
    let merged = merge_audio(&[first, second], 0.2).unwrap();
    assert_eq!(merged.frames(), 800);
    assert!(
        merged.samples()[300..500]
            .iter()
            .all(|sample| *sample == 0.0)
    );
}

#[test]
fn merge_rejects_format_mismatch() {
    let left = sine(16_000, 0.3, 1);
    let wrong_rate = sine(24_000, 0.3, 1);
    let wrong_channels = sine(16_000, 0.3, 2);
    assert!(merge_audio(&[left.clone(), wrong_rate], 0.1).is_err());
    assert!(merge_audio(&[left, wrong_channels], 0.1).is_err());
}
