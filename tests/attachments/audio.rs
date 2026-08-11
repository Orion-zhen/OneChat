use super::*;

fn pcm16_wav(sample_rate: u32, samples: u32) -> Vec<u8> {
    let data_len = samples * 2;
    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);
    bytes
}

#[test]
fn wav_and_mp3_uploads_are_detected_with_duration() {
    let directory = tempdir().unwrap();
    let wav = directory.path().join("speech.wav");
    fs::write(&wav, pcm16_wav(16_000, 16_000)).unwrap();

    let wav = load_audio(&wav, true).unwrap();
    assert_eq!(wav.kind, AttachmentKind::Audio);
    assert_eq!(wav.files[0].name, "content.wav");
    assert_eq!(wav.files[0].kind, AttachmentFileKind::Audio);
    assert_eq!(wav.files[0].media_type, "audio/wav");
    assert_eq!(wav.audio.as_ref().unwrap().duration_ms, 1_000);
    assert_eq!(
        wav.audio.as_ref().unwrap().source,
        AudioAttachmentSource::Upload
    );

    let mp3_path = directory.path().join("speech.mp3");
    let mp3_bytes = include_bytes!("../fixtures/audio/minimal.mp3");
    fs::write(&mp3_path, mp3_bytes).unwrap();
    let mp3 = load_audio(&mp3_path, true).unwrap();
    assert_eq!(mp3.kind, AttachmentKind::Audio);
    assert_eq!(mp3.files[0].name, "content.mp3");
    assert_eq!(mp3.files[0].media_type, "audio/mpeg");
    assert!(mp3.audio.unwrap().duration_ms > 0);
}

#[test]
fn audio_upload_requires_model_capability() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("speech.wav");
    fs::write(&path, pcm16_wav(16_000, 1_600)).unwrap();

    assert!(
        load_audio(&path, false)
            .unwrap_err()
            .contains("audio support")
    );
}

#[test]
fn damaged_empty_and_disguised_audio_is_rejected() {
    let directory = tempdir().unwrap();

    let damaged = directory.path().join("damaged.wav");
    fs::write(&damaged, b"RIFF\x04\0\0\0WAVE").unwrap();
    assert!(
        load_audio(&damaged, true)
            .unwrap_err()
            .contains("Invalid audio")
    );

    let damaged_mp3 = directory.path().join("damaged.mp3");
    fs::write(&damaged_mp3, b"ID3\x04\0\0\0\0\0\0broken").unwrap();
    assert!(
        load_audio(&damaged_mp3, true)
            .unwrap_err()
            .contains("Invalid audio")
    );

    let empty = directory.path().join("empty.mp3");
    fs::write(&empty, []).unwrap();
    assert!(load_audio(&empty, true).unwrap_err().contains("empty"));

    let disguised = directory.path().join("disguised.wav");
    fs::write(&disguised, include_bytes!("../fixtures/audio/minimal.mp3")).unwrap();
    assert!(
        load_audio(&disguised, true)
            .unwrap_err()
            .contains("does not match")
    );
}

#[test]
fn audio_uploads_over_ten_mib_are_rejected_before_reading() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("large.wav");
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_AUDIO_BYTES + 1).unwrap();

    assert!(
        load_audio(&path, true)
            .unwrap_err()
            .contains("10 MiB audio limit")
    );
}
