use crate::speech::{
    audio::{merge_audio, validate_audio},
    run::{RunSnapshot, RunStatus, SegmentResult, SegmentStatus, SpeechRun},
};

pub(super) fn finish(
    snapshot: RunSnapshot,
    segments: Vec<SegmentResult>,
    cancelled: bool,
) -> SpeechRun {
    let ready: Vec<_> = segments
        .iter()
        .filter(|result| result.status == SegmentStatus::Ready)
        .filter_map(|result| result.clip.clone())
        .collect();
    let mut error = None;
    let combined_clip = if ready.is_empty() {
        None
    } else {
        match merge_audio(&ready, snapshot.config.merge.min_silence_sec) {
            Ok(clip) => Some(clip),
            Err(merge_error) => {
                error = Some(merge_error);
                None
            }
        }
    };
    let final_validation = combined_clip
        .as_ref()
        .map(|clip| validate_audio(clip, snapshot.config.audio_validation));
    let status = if cancelled {
        RunStatus::Cancelled
    } else if error.is_some() || ready.is_empty() {
        RunStatus::Failed
    } else if ready.len() < segments.len() {
        RunStatus::Partial
    } else {
        RunStatus::Completed
    };
    SpeechRun {
        snapshot,
        status,
        segments,
        combined_clip,
        final_validation,
        error,
    }
}
