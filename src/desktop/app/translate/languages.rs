use gpui::{Context, Window};
use gpui_component::select::SelectState;

use super::controls::LanguageOption;
use crate::desktop::app::OneChat;

impl OneChat {
    pub(crate) fn swap_translation_languages(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.translation.source_language == "Auto Detect" {
            let detected = detected_source_language(&self.translation.source)
                .unwrap_or("English")
                .to_string();
            self.translation.source_language = self.translation.target_language.clone();
            self.translation.target_language = detected;
        } else {
            std::mem::swap(
                &mut self.translation.source_language,
                &mut self.translation.target_language,
            );
        }
        set_language(
            &self.translation.controls.source_language,
            &self.translation.source_language,
            window,
            cx,
        );
        set_language(
            &self.translation.controls.target_language,
            &self.translation.target_language,
            window,
            cx,
        );
        cx.notify();
    }
}

pub(super) fn set_language(
    select: &gpui::Entity<SelectState<Vec<LanguageOption>>>,
    language: &str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) {
    select.update(cx, |select, cx| {
        select.set_selected_value(&language.to_string(), window, cx)
    });
}

pub(super) fn resolved_source_language(selection: &str, source: &str) -> String {
    if selection != "Auto Detect" {
        return selection.to_string();
    }
    detected_source_language(source)
        .map(str::to_string)
        .or_else(|| whatlang::detect(source).map(|info| info.lang().eng_name().to_string()))
        .unwrap_or_else(|| "the detected source language".into())
}

pub(super) fn same_language(source: &str, target: &str) -> bool {
    source.eq_ignore_ascii_case(target)
}

fn detected_source_language(source: &str) -> Option<&'static str> {
    use whatlang::Lang;

    Some(match whatlang::detect(source)?.lang() {
        Lang::Eng => "English",
        Lang::Cmn => "Simplified Chinese",
        Lang::Jpn => "Japanese",
        Lang::Kor => "Korean",
        Lang::Fra => "French",
        Lang::Deu => "German",
        Lang::Spa => "Spanish",
        Lang::Por => "Portuguese",
        Lang::Ita => "Italian",
        Lang::Rus => "Russian",
        Lang::Ara => "Arabic",
        Lang::Hin => "Hindi",
        Lang::Tha => "Thai",
        Lang::Vie => "Vietnamese",
        Lang::Ind => "Indonesian",
        Lang::Tur => "Turkish",
        Lang::Nld => "Dutch",
        Lang::Pol => "Polish",
        Lang::Ukr => "Ukrainian",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{detected_source_language, resolved_source_language, same_language};

    #[test]
    fn auto_detection_uses_selectable_language_names() {
        assert_eq!(
            detected_source_language("这是一个中文测试句子。"),
            Some("Simplified Chinese")
        );
        assert_eq!(
            detected_source_language("This is a complete English sentence."),
            Some("English")
        );
    }

    #[test]
    fn explicit_and_detected_source_languages_can_be_compared_with_the_target() {
        assert!(same_language("English", "English"));
        assert!(!same_language("English", "Japanese"));
        assert!(same_language(
            &resolved_source_language("Auto Detect", "This is a complete English sentence."),
            "English"
        ));
    }
}
