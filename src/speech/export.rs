use mp3lame_encoder::{
    Bitrate, Builder, FlushGap, InterleavedPcm, MonoPcm, Quality, VbrMode, max_required_buffer_size,
};

use super::{
    audio::{AudioClip, encode_wav},
    error::SpeechError,
};

const MP3_SAMPLE_RATES: &[u32] = &[
    8_000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];

pub fn export_wav(clip: &AudioClip) -> Result<Vec<u8>, SpeechError> {
    encode_wav(clip).map_err(|error| SpeechError::export(error.message))
}

pub fn export_mp3(clip: &AudioClip) -> Result<Vec<u8>, SpeechError> {
    if clip.frames() == 0 {
        return Err(SpeechError::export(
            "cannot encode MP3 audio with no frames",
        ));
    }
    if !MP3_SAMPLE_RATES.contains(&clip.sample_rate()) {
        return Err(SpeechError::export(format!(
            "MP3 export does not support a {} Hz sample rate",
            clip.sample_rate()
        )));
    }
    if !matches!(clip.channels(), 1 | 2) {
        return Err(SpeechError::export(format!(
            "MP3 export supports mono or stereo audio, got {} channels",
            clip.channels()
        )));
    }

    let mut encoder = Builder::new()
        .ok_or_else(|| SpeechError::export("could not allocate the MP3 encoder"))?
        .with_num_channels(clip.channels() as u8)
        .and_then(|builder| builder.with_sample_rate(clip.sample_rate()))
        .and_then(|builder| builder.with_brate(Bitrate::Kbps192))
        .and_then(|builder| builder.with_quality(Quality::NearBest))
        .and_then(|builder| builder.with_vbr_mode(VbrMode::Mtrh))
        .and_then(|builder| builder.with_vbr_quality(Quality::NearBest))
        .and_then(|builder| builder.with_to_write_vbr_tag(false))
        .and_then(Builder::build)
        .map_err(|error| {
            SpeechError::export(format!("could not initialize MP3 encoder: {error}"))
        })?;

    let mut output = Vec::with_capacity(max_required_buffer_size(clip.frames()) + 7_200);
    let encoded = match clip.channels() {
        1 => encoder.encode_to_vec(MonoPcm(clip.samples()), &mut output),
        2 => encoder.encode_to_vec(InterleavedPcm(clip.samples()), &mut output),
        _ => unreachable!(),
    };
    encoded.map_err(|error| SpeechError::export(format!("could not encode MP3 audio: {error}")))?;
    output.reserve(7_200);
    encoder
        .flush_to_vec::<FlushGap>(&mut output)
        .map_err(|error| SpeechError::export(format!("could not finalize MP3 audio: {error}")))?;
    if output.is_empty() {
        return Err(SpeechError::export("MP3 encoder produced no data"));
    }
    Ok(output)
}
