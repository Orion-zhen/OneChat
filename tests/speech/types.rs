use std::time::Duration;

use onechat::speech::{AudioClip, SpeechConfig, derive_seed};

#[test]
fn audio_clip_enforces_pcm_invariants() {
    assert!(AudioClip::new(vec![0.0], 0, 1).is_err());
    assert!(AudioClip::new(vec![0.0], 24_000, 0).is_err());
    assert!(AudioClip::new(vec![0.0; 3], 24_000, 2).is_err());
    assert!(AudioClip::new(vec![f32::NAN], 24_000, 1).is_err());
}

#[test]
fn derived_seed_is_stable_and_attempt_specific() {
    assert_eq!(derive_seed(None, 3, 2), None);
    assert_eq!(derive_seed(Some(42), 3, 2), Some(582_581_613_408_406));
    assert_ne!(derive_seed(Some(42), 3, 2), derive_seed(Some(42), 3, 3));
    assert_ne!(derive_seed(Some(42), 3, 2), derive_seed(Some(42), 4, 2));
    assert!(derive_seed(Some(u64::MAX), usize::MAX, u32::MAX).unwrap() < 1_u64 << 53);
}

#[test]
fn speech_defaults_and_normalization_are_stable() {
    let defaults = SpeechConfig::default();
    assert_eq!(defaults.endpoint, "http://127.0.0.1:8080");
    assert_eq!(defaults.request_timeout, Duration::from_secs(600));
    assert_eq!(defaults.segmentation.min_chars, 15);
    assert_eq!(defaults.segmentation.target_chars, 160);
    assert_eq!(defaults.segmentation.max_chars, 320);
    assert_eq!(defaults.segmentation.spread, 80);
    assert_eq!(defaults.quality_retries, 2);
    assert_eq!(defaults.merge.min_silence_sec, 0.8);
    assert_eq!(defaults.transcript_validation.similarity_threshold, 0.98);

    let mut config = defaults;
    config.endpoint = "  http://localhost:8080///  ".into();
    config.bearer_token = Some("   ".into());
    config.generation.model = " tts ".into();
    config.generation.voice = Some(" voice ".into());
    let config = config.normalized().unwrap();
    assert_eq!(config.endpoint, "http://localhost:8080");
    assert_eq!(config.bearer_token, None);
    assert_eq!(config.generation.model, "tts");
    assert_eq!(config.generation.voice.as_deref(), Some("voice"));
}

#[test]
fn speech_config_rejects_invalid_runtime_values() {
    let mut config = SpeechConfig::default();
    config.generation.model = "tts".into();
    config.endpoint = "file:///tmp/audio.cpp".into();
    assert!(config.validate().is_err());

    config.endpoint = "http://localhost:8080".into();
    config.request_timeout = Duration::ZERO;
    assert!(config.validate().is_err());

    config.request_timeout = Duration::from_secs(1);
    config.segmentation.target_chars = config.segmentation.max_chars + 1;
    assert!(config.validate().is_err());

    config.segmentation = Default::default();
    config.audio_validation.min_active_ratio = 1.1;
    assert!(config.validate().is_err());

    config.audio_validation = Default::default();
    config.transcript_validation.enabled = true;
    assert!(config.validate().is_err());
}
