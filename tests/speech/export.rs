use std::{f32::consts::PI, io::Cursor};

use onechat::speech::{
    AudioClip,
    audio::decode_wav,
    export::{export_mp3, export_wav},
};
use rodio::Decoder;

fn clip(sample_rate: u32, channels: u16) -> AudioClip {
    let samples: Vec<f32> = (0..sample_rate / 2)
        .flat_map(|index| {
            let sample = (2.0 * PI * 440.0 * index as f32 / sample_rate as f32).sin() * 0.2;
            std::iter::repeat_n(sample, usize::from(channels))
        })
        .collect();
    AudioClip::new(samples, sample_rate, channels).unwrap()
}

#[test]
fn wav_export_is_decodable() {
    let source = clip(24_000, 2);
    let decoded = decode_wav(&export_wav(&source).unwrap()).unwrap();
    assert_eq!(decoded.sample_rate(), source.sample_rate());
    assert_eq!(decoded.channels(), source.channels());
    assert_eq!(decoded.frames(), source.frames());
}

#[test]
fn mp3_export_is_decodable_without_ffmpeg() {
    let bytes = export_mp3(&clip(24_000, 1)).unwrap();
    let mut decoder = Decoder::try_from(Cursor::new(bytes)).unwrap();
    assert!(decoder.by_ref().take(100).any(|sample| sample != 0.0));
}

#[test]
fn mp3_export_rejects_unsupported_formats() {
    assert!(export_mp3(&clip(24_000, 3)).is_err());
    assert!(export_mp3(&clip(12_345, 1)).is_err());
}
