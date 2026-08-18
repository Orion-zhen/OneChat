use gpui::{
    App, AppContext as _, Context, Entity, Focusable as _, IntoElement, SharedString, Window,
};
use gpui_component::{
    input::{InputEvent, TextareaState},
    searchable_list::SearchableListItem,
    select::{SelectEvent, SelectState},
};

use super::state::TranslationPromptKind;
use crate::{
    desktop::app::{OneChat, ShellOverlay},
    domain::{DEFAULT_TRANSLATION_SYSTEM_PROMPT, DEFAULT_TRANSLATION_USER_PROMPT},
};

#[derive(Clone)]
pub(crate) struct LanguageOption(String);

impl LanguageOption {
    fn new(name: &str) -> Self {
        Self(name.into())
    }
}

impl SearchableListItem for LanguageOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.0.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.title()
    }
}

pub(crate) struct TranslationControls {
    pub(crate) source: Entity<TextareaState>,
    pub(crate) system_prompt: Entity<TextareaState>,
    pub(crate) user_prompt: Entity<TextareaState>,
    pub(crate) source_language: Entity<SelectState<Vec<LanguageOption>>>,
    pub(crate) target_language: Entity<SelectState<Vec<LanguageOption>>>,
}

impl TranslationControls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let source = cx.new(|cx| {
            TextareaState::new(window, cx)
                .soft_wrap(true)
                .placeholder("Paste text to translate")
        });
        cx.subscribe(&source, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.translation.source = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let system_prompt = textarea_with_value(DEFAULT_TRANSLATION_SYSTEM_PROMPT, window, cx);
        cx.subscribe(&system_prompt, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.translation.system_prompt = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let user_prompt = textarea_with_value(DEFAULT_TRANSLATION_USER_PROMPT, window, cx);
        cx.subscribe(&user_prompt, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.translation.user_prompt = input.read(cx).value().to_string();
                cx.notify();
            }
        })
        .detach();

        let source_language = cx.new(|cx| {
            SelectState::new(
                language_options(true),
                Some(gpui_component::IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(
            &source_language,
            |this, _, event: &SelectEvent<Vec<LanguageOption>>, cx| {
                let SelectEvent::Confirm(language) = event;
                if let Some(language) = language {
                    this.translation.source_language = language.clone();
                    cx.notify();
                }
            },
        )
        .detach();

        let target_language = cx.new(|cx| {
            SelectState::new(
                language_options(false),
                Some(gpui_component::IndexPath::default()),
                window,
                cx,
            )
            .searchable(true)
        });
        cx.subscribe(
            &target_language,
            |this, _, event: &SelectEvent<Vec<LanguageOption>>, cx| {
                let SelectEvent::Confirm(language) = event;
                if let Some(language) = language {
                    this.translation.target_language = language.clone();
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            source,
            system_prompt,
            user_prompt,
            source_language,
            target_language,
        }
    }
}

fn textarea_with_value(
    value: &str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> Entity<TextareaState> {
    cx.new(|cx| {
        let mut input = TextareaState::new(window, cx).soft_wrap(true);
        input.insert(value.to_string(), window, cx);
        input
    })
}

impl OneChat {
    pub(crate) fn sync_translation_prompt_controls(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        sync_textarea(
            &self.translation.controls.system_prompt,
            &self.translation.system_prompt,
            window,
            cx,
        );
        sync_textarea(
            &self.translation.controls.user_prompt,
            &self.translation.user_prompt,
            window,
            cx,
        );
    }

    pub(in crate::desktop::app) fn set_translation_prompts(
        &mut self,
        system_prompt: String,
        user_prompt: String,
        cx: &mut Context<Self>,
    ) {
        self.translation.system_prompt = system_prompt;
        self.translation.user_prompt = user_prompt;
        self.translation.error = None;
        cx.notify();
    }

    pub(crate) fn reset_translation_prompt_override(&mut self, cx: &mut Context<Self>) {
        self.set_translation_prompts(
            self.settings().translation_system_prompt.clone(),
            self.settings().translation_user_prompt.clone(),
            cx,
        );
    }

    pub(crate) fn save_translation_prompts_as_default(&mut self, cx: &mut Context<Self>) {
        if !super::content::prompts_include_text(
            &self.translation.system_prompt,
            &self.translation.user_prompt,
        ) {
            self.data.error = Some("A translation prompt must include {{text}}.".into());
            cx.notify();
            return;
        }
        let system_prompt = self.translation.system_prompt.trim().to_string();
        let user_prompt = self.translation.user_prompt.trim().to_string();
        self.data.snapshot.settings.translation_system_prompt = system_prompt.clone();
        self.data.snapshot.settings.translation_user_prompt = user_prompt.clone();
        self.set_translation_prompts(system_prompt, user_prompt, cx);
        self.save_settings(cx);
    }

    pub(crate) fn open_translation_prompt_editor(
        &mut self,
        kind: TranslationPromptKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let (overlay, input) = match kind {
            TranslationPromptKind::System => (
                ShellOverlay::TranslationSystemPrompt,
                self.translation.controls.system_prompt.clone(),
            ),
            TranslationPromptKind::User => (
                ShellOverlay::TranslationUserPrompt,
                self.translation.controls.user_prompt.clone(),
            ),
        };
        self.open_shell_overlay(overlay, true, window, cx);
        window.focus(&input.read(cx).focus_handle(cx), cx);
    }
}

fn sync_textarea(
    input: &Entity<TextareaState>,
    value: &str,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) {
    if input.read(cx).value() != value {
        input.update(cx, |input, cx| {
            input.set_value(value.to_string(), window, cx)
        });
    }
}

fn language_options(include_auto: bool) -> Vec<LanguageOption> {
    let languages = [
        "English",
        "Simplified Chinese",
        "Traditional Chinese",
        "Japanese",
        "Korean",
        "French",
        "German",
        "Spanish",
        "Portuguese",
        "Italian",
        "Russian",
        "Arabic",
        "Hindi",
        "Thai",
        "Vietnamese",
        "Indonesian",
        "Turkish",
        "Dutch",
        "Polish",
        "Ukrainian",
    ];
    include_auto
        .then(|| LanguageOption::new("Auto Detect"))
        .into_iter()
        .chain(languages.into_iter().map(LanguageOption::new))
        .collect()
}
