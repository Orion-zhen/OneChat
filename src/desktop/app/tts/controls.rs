use gpui::{App, AppContext as _, Context, Entity, IntoElement, SharedString, Window};
use gpui_component::{
    input::{InputEvent, InputState},
    searchable_list::SearchableListItem,
    select::{SelectEvent, SelectState},
    slider::{SliderEvent, SliderState},
};

use crate::{
    desktop::{app::OneChat, ui::controls::sync_slider},
    speech::{AudioValidationConfig, MergeConfig, SegmentationConfig, SpeechConfig},
};

#[derive(Clone)]
pub(crate) struct TtsSelectOption(String);

impl TtsSelectOption {
    fn new(value: String) -> Self {
        Self(value)
    }
}

impl SearchableListItem for TtsSelectOption {
    type Value = String;

    fn title(&self) -> SharedString {
        self.0.clone().into()
    }

    fn value(&self) -> &Self::Value {
        &self.0
    }

    fn render(&self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        crate::desktop::ui::spaced_select_item(self.title(), cx)
    }
}

pub(crate) struct TtsConnectionControls {
    pub(crate) endpoint: Entity<InputState>,
    pub(crate) token: Entity<InputState>,
}

pub(crate) struct TtsTuningControls {
    pub(crate) min_chars: Entity<SliderState>,
    pub(crate) target_chars: Entity<SliderState>,
    pub(crate) max_chars: Entity<SliderState>,
    pub(crate) spread: Entity<SliderState>,
    pub(crate) min_duration: Entity<SliderState>,
    pub(crate) min_rms: Entity<SliderState>,
    pub(crate) min_active_ratio: Entity<SliderState>,
    pub(crate) noise_flatness: Entity<SliderState>,
    pub(crate) noise_zcr: Entity<SliderState>,
    pub(crate) noise_active_ratio: Entity<SliderState>,
    pub(crate) trim_max_silence: Entity<SliderState>,
    pub(crate) trim_keep_silence: Entity<SliderState>,
    pub(crate) merge_silence: Entity<SliderState>,
    pub(crate) similarity: Entity<SliderState>,
}

pub(crate) struct TtsControls {
    pub(crate) connection: TtsConnectionControls,
    pub(crate) source: Entity<InputState>,
    pub(crate) model: Entity<SelectState<Vec<TtsSelectOption>>>,
    pub(crate) voice: Entity<SelectState<Vec<TtsSelectOption>>>,
    pub(crate) tuning: TtsTuningControls,
    synced_models: Vec<String>,
    synced_voices: Vec<String>,
}

impl TtsControls {
    pub(super) fn new(window: &mut Window, cx: &mut Context<OneChat>) -> Self {
        let defaults = SpeechConfig::default();
        let endpoint = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://127.0.0.1:8080")
                .default_value(defaults.endpoint.clone())
        });
        let token = cx.new(|cx| InputState::new(window, cx).placeholder("Optional bearer token"));
        for input in [&endpoint, &token] {
            cx.subscribe(input, |this, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.sync_tts_connection_draft(cx);
                    cx.notify();
                }
            })
            .detach();
        }

        let source = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .soft_wrap(true)
                .placeholder("Paste text to turn into speech")
        });
        cx.subscribe(&source, |this, input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::Change) {
                this.tts
                    .controller
                    .set_source(input.read(cx).value().to_string());
                cx.notify();
            }
        })
        .detach();

        let model = cx.new(|cx| {
            SelectState::new(Vec::<TtsSelectOption>::new(), None, window, cx).searchable(true)
        });
        cx.subscribe(
            &model,
            |this, _, event: &SelectEvent<Vec<TtsSelectOption>>, cx| {
                let SelectEvent::Confirm(model) = event;
                this.select_tts_model(model.clone(), cx);
            },
        )
        .detach();
        let voice = cx.new(|cx| {
            SelectState::new(Vec::<TtsSelectOption>::new(), None, window, cx).searchable(true)
        });
        cx.subscribe(
            &voice,
            |this, _, event: &SelectEvent<Vec<TtsSelectOption>>, cx| {
                let SelectEvent::Confirm(voice) = event;
                this.select_tts_voice(voice.clone(), cx);
            },
        )
        .detach();

        let segmentation = SegmentationConfig::default();
        let audio = AudioValidationConfig::default();
        let tuning = TtsTuningControls {
            min_chars: slider(1.0, 320.0, segmentation.min_chars as f32, cx),
            target_chars: slider(1.0, 640.0, segmentation.target_chars as f32, cx),
            max_chars: slider(1.0, 1000.0, segmentation.max_chars as f32, cx),
            spread: slider(1.0, 320.0, segmentation.spread as f32, cx),
            min_duration: slider_with_step(0.05, 5.0, 0.05, audio.min_duration_sec, cx),
            min_rms: slider_with_step(0.0, 0.1, 0.0005, audio.min_rms, cx),
            min_active_ratio: slider_with_step(0.0, 1.0, 0.01, audio.min_active_ratio, cx),
            noise_flatness: slider_with_step(0.0, 1.0, 0.01, audio.noise_flatness, cx),
            noise_zcr: slider_with_step(0.0, 1.0, 0.01, audio.noise_zcr, cx),
            noise_active_ratio: slider_with_step(0.0, 1.0, 0.01, audio.noise_active_ratio, cx),
            trim_max_silence: slider_with_step(0.0, 2.0, 0.01, audio.trim_max_edge_silence_sec, cx),
            trim_keep_silence: slider_with_step(
                0.0,
                1.0,
                0.01,
                audio.trim_keep_edge_silence_sec,
                cx,
            ),
            merge_silence: slider_with_step(
                0.0,
                3.0,
                0.05,
                MergeConfig::default().min_silence_sec,
                cx,
            ),
            similarity: slider(
                0.0,
                1.0,
                defaults.transcript_validation.similarity_threshold,
                cx,
            ),
        };
        for slider in tuning.all() {
            cx.subscribe(slider, |this, _, _: &SliderEvent, cx| {
                this.sync_tts_tuning_draft(cx);
                cx.notify();
            })
            .detach();
        }

        Self {
            connection: TtsConnectionControls { endpoint, token },
            source,
            model,
            voice,
            tuning,
            synced_models: Vec::new(),
            synced_voices: Vec::new(),
        }
    }
}

impl TtsTuningControls {
    fn all(&self) -> [&Entity<SliderState>; 14] {
        [
            &self.min_chars,
            &self.target_chars,
            &self.max_chars,
            &self.spread,
            &self.min_duration,
            &self.min_rms,
            &self.min_active_ratio,
            &self.noise_flatness,
            &self.noise_zcr,
            &self.noise_active_ratio,
            &self.trim_max_silence,
            &self.trim_keep_silence,
            &self.merge_silence,
            &self.similarity,
        ]
    }
}

impl OneChat {
    pub(crate) fn sync_tts_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let models = self
            .tts
            .controller
            .discovery
            .catalog
            .tts
            .iter()
            .map(|model| model.id.clone())
            .collect::<Vec<_>>();
        sync_select(
            &self.tts.controls.model,
            &mut self.tts.controls.synced_models,
            models,
            (!self.tts.controller.config.generation.model.is_empty())
                .then(|| self.tts.controller.config.generation.model.clone()),
            window,
            cx,
        );

        let voices = self.tts.controller.discovery.voices.clone();
        sync_select(
            &self.tts.controls.voice,
            &mut self.tts.controls.synced_voices,
            voices,
            self.tts.controller.config.generation.voice.clone(),
            window,
            cx,
        );

        let tuning = &self.tts.controls.tuning;
        let segmentation = self.tts.controller.config.segmentation;
        for (slider, value) in [
            (&tuning.min_chars, segmentation.min_chars as f32),
            (&tuning.target_chars, segmentation.target_chars as f32),
            (&tuning.max_chars, segmentation.max_chars as f32),
            (&tuning.spread, segmentation.spread as f32),
        ] {
            sync_slider(slider, value, window, cx);
        }
        let audio = self.tts.controller.config.audio_validation;
        for (slider, value) in [
            (&tuning.min_duration, audio.min_duration_sec),
            (&tuning.min_rms, audio.min_rms),
            (&tuning.min_active_ratio, audio.min_active_ratio),
            (&tuning.noise_flatness, audio.noise_flatness),
            (&tuning.noise_zcr, audio.noise_zcr),
            (&tuning.noise_active_ratio, audio.noise_active_ratio),
            (&tuning.trim_max_silence, audio.trim_max_edge_silence_sec),
            (&tuning.trim_keep_silence, audio.trim_keep_edge_silence_sec),
            (
                &tuning.merge_silence,
                self.tts.controller.config.merge.min_silence_sec,
            ),
            (
                &tuning.similarity,
                self.tts
                    .controller
                    .config
                    .transcript_validation
                    .similarity_threshold,
            ),
        ] {
            sync_slider(slider, value, window, cx);
        }
    }
}

fn slider(min: f32, max: f32, value: f32, cx: &mut Context<OneChat>) -> Entity<SliderState> {
    slider_with_step(min, max, if max <= 1.0 { 0.01 } else { 1.0 }, value, cx)
}

fn slider_with_step(
    min: f32,
    max: f32,
    step: f32,
    value: f32,
    cx: &mut Context<OneChat>,
) -> Entity<SliderState> {
    cx.new(|_| {
        SliderState::new()
            .min(min)
            .max(max)
            .step(step)
            .default_value(value)
    })
}

fn sync_select(
    state: &Entity<SelectState<Vec<TtsSelectOption>>>,
    synced: &mut Vec<String>,
    items: Vec<String>,
    selected: Option<String>,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) {
    let items_changed = *synced != items;
    if items_changed {
        synced.clone_from(&items);
    }
    let selected_changed = state.read(cx).selected_value().cloned() != selected;
    if items_changed || selected_changed {
        state.update(cx, |select, cx| {
            if items_changed {
                select.set_items(
                    items.into_iter().map(TtsSelectOption::new).collect(),
                    window,
                    cx,
                );
            }
            match selected.as_ref() {
                Some(selected) => select.set_selected_value(selected, window, cx),
                None => select.set_selected_index(None, window, cx),
            }
        });
    }
}
