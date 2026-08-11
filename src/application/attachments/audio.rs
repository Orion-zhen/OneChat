use std::{io::Cursor, path::Path};

use symphonia::core::{
    codecs::audio::well_known::CODEC_ID_MP3,
    errors::Error as SymphoniaError,
    formats::{FormatOptions, TrackType, probe::Hint},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::Timestamp,
};

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
    let duration_ms = duration_ms(&bytes, actual_extension)
        .map_err(|error| format!("Invalid audio {name}: {error}"))?;

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

fn duration_ms(bytes: &[u8], extension: &str) -> Result<u64, String> {
    let source = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|error| error.to_string())?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or_else(|| "file contains no audio track".to_string())?;
    let track_id = track.id;
    if extension == "mp3"
        && track
            .codec_params
            .as_ref()
            .and_then(|params| params.audio())
            .is_none_or(|params| params.codec != CODEC_ID_MP3)
    {
        return Err("file does not contain MPEG Layer III audio".into());
    }
    let time_base = track
        .time_base
        .ok_or_else(|| "audio duration is unavailable".to_string())?;
    let mut end = track.duration.map(|duration| duration.get()).unwrap_or(0);

    loop {
        match format.next_packet() {
            Ok(Some(packet)) if packet.track_id == track_id => {
                let packet_end = packet
                    .pts
                    .get()
                    .max(0)
                    .saturating_add_unsigned(packet.dur.get());
                end = end.max(packet_end as u64);
            }
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(SymphoniaError::ResetRequired) => break,
            Err(error) => return Err(error.to_string()),
        }
    }

    if end == 0 {
        return Err("file contains no audio samples".into());
    }
    let end = i64::try_from(end).unwrap_or(i64::MAX);
    let time = time_base.calc_time_saturating(Timestamp::new(end));
    let duration_ms = u64::try_from((time.as_nanos() + 999_999) / 1_000_000)
        .map_err(|_| "audio duration is invalid".to_string())?;
    (duration_ms > 0)
        .then_some(duration_ms)
        .ok_or_else(|| "file contains no audio samples".into())
}
