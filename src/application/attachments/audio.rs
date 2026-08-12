use std::{io::Cursor, path::Path};

use rodio::{Decoder, Source};

use crate::domain::{
    AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind,
    AudioAttachmentMetadata, AudioAttachmentSource, new_id,
};

use super::MAX_AUDIO_BYTES;

pub(super) fn is_supported_extension(extension: &str) -> bool {
    matches!(extension, "wav" | "mp3")
}

pub(super) fn load(
    path: &Path,
    name: String,
    extension: &str,
    size: u64,
    audio_input: bool,
) -> Result<AttachmentDraft, String> {
    if !audio_input {
        return Err(format!("{name} requires a model with audio support."));
    }
    if size == 0 {
        return Err(format!("Audio file {name} is empty."));
    }
    if size > MAX_AUDIO_BYTES {
        return Err(format!("{name} exceeds the 10 MiB audio limit."));
    }

    let bytes = std::fs::read(path).map_err(|error| format!("Could not read {name}: {error}"))?;
    let (actual_extension, media_type) = detect_format(&bytes)
        .ok_or_else(|| format!("Invalid audio {name}: file is not a valid WAV or MP3."))?;
    if extension != actual_extension {
        return Err(format!(
            "Invalid audio {name}: file content does not match its .{extension} extension."
        ));
    }
    let duration_ms =
        duration_ms(&bytes).map_err(|error| format!("Invalid audio {name}: {error}"))?;

    Ok(AttachmentDraft {
        id: new_id("attachment"),
        name,
        kind: AttachmentKind::Audio,
        files: vec![AttachmentDraftFile {
            name: format!("content.{actual_extension}"),
            kind: AttachmentFileKind::Audio,
            media_type: media_type.into(),
            bytes,
        }],
        audio: Some(AudioAttachmentMetadata {
            duration_ms,
            source: AudioAttachmentSource::Upload,
        }),
    })
}

fn detect_format(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
        Some(("wav", "audio/wav"))
    } else if bytes.starts_with(b"ID3") || is_mp3_frame(bytes) {
        Some(("mp3", "audio/mpeg"))
    } else {
        None
    }
}

fn is_mp3_frame(bytes: &[u8]) -> bool {
    bytes.len() >= 4
        && bytes[0] == 0xff
        && bytes[1] & 0xe0 == 0xe0
        && bytes[1] & 0x18 != 0x08
        && bytes[1] & 0x06 == 0x02
        && bytes[2] & 0xf0 != 0xf0
        && bytes[2] & 0x0c != 0x0c
}

fn duration_ms(bytes: &[u8]) -> Result<u64, String> {
    let decoder =
        Decoder::try_from(Cursor::new(bytes.to_vec())).map_err(|error| error.to_string())?;
    let duration = decoder
        .total_duration()
        .ok_or_else(|| "audio duration is unavailable".to_string())?;
    let duration_ms = u64::try_from(duration.as_nanos().div_ceil(1_000_000))
        .map_err(|_| "audio duration is invalid".to_string())?;
    (duration_ms > 0)
        .then_some(duration_ms)
        .ok_or_else(|| "file contains no audio samples".into())
}
