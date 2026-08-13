use gpui::{Context, Window};

use super::{ExportNotice, OneChat};
use crate::speech::{
    AudioClip, RunStatus, SpeechError,
    export::{export_mp3, export_wav},
};

impl OneChat {
    pub(crate) fn export_tts_wav(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export_tts_audio("wav", export_wav, window, cx);
    }

    pub(crate) fn export_tts_mp3(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.export_tts_audio("mp3", export_mp3, window, cx);
    }

    fn export_tts_audio(
        &mut self,
        extension: &'static str,
        encode: fn(&AudioClip) -> Result<Vec<u8>, SpeechError>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(run) = self.tts.controller.run.as_ref() else {
            return;
        };
        let Some(clip) = run.combined_clip.clone() else {
            return;
        };
        let partial = run.status != RunStatus::Completed;
        let label = if partial {
            "OneChat Speech - Partial"
        } else {
            "OneChat Speech"
        };
        self.export_file(
            label,
            extension,
            ExportNotice::speech(partial),
            move |path| {
                let bytes = encode(&clip).map_err(|error| error.to_string())?;
                std::fs::write(path, bytes).map_err(|error| error.to_string())
            },
            window,
            cx,
        );
    }
}
