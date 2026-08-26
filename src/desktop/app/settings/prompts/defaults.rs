use super::*;

impl OneChat {
    pub(crate) fn select_primary_model(
        &mut self,
        model_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(model_id) = model_id else {
            return;
        };
        if !self.model_is_available(&model_id, cx) {
            return;
        }
        if self.data.snapshot.settings.primary_model_id.as_deref() == Some(&model_id) {
            cx.notify();
            return;
        }
        self.data.snapshot.settings.primary_model_id = Some(model_id);
        if self.data.snapshot.settings.title_generation_model == TitleModelSource::Primary {
            self.sync_title_reasoning_preset();
        }
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn select_title_generation_model(
        &mut self,
        source: TitleModelSource,
        cx: &mut Context<Self>,
    ) {
        if let TitleModelSource::Model(model_id) = &source
            && !self.model_is_available(model_id, cx)
        {
            return;
        }
        if self.data.snapshot.settings.title_generation_model == source {
            cx.notify();
            return;
        }
        let uses_own_reasoning = source != TitleModelSource::Current;
        self.data.snapshot.settings.title_generation_model = source;
        if uses_own_reasoning {
            self.sync_title_reasoning_preset();
        }
        self.save_settings(cx);
        cx.notify();
    }

    fn model_is_available(&mut self, model_id: &str, cx: &mut Context<Self>) -> bool {
        let Some(model) = self
            .data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == model_id)
        else {
            return false;
        };
        if let Err(reason) = self.model_availability(model) {
            self.data.error = Some(format!("Model is unavailable: {reason}."));
            cx.notify();
            return false;
        }
        true
    }

    fn sync_title_reasoning_preset(&mut self) {
        let requested = self
            .data
            .snapshot
            .settings
            .title_generation_reasoning_preset
            .clone();
        self.data
            .snapshot
            .settings
            .title_generation_reasoning_preset = self
            .title_generation_model()
            .and_then(|model| model.reasoning.as_ref())
            .map(|reasoning| {
                requested
                    .filter(|requested| {
                        reasoning
                            .preset_options()
                            .iter()
                            .any(|(id, _)| id == requested)
                    })
                    .unwrap_or_else(|| reasoning.default_preset().to_string())
            });
    }

    pub(crate) fn select_title_generation_reasoning_preset(
        &mut self,
        preset: String,
        cx: &mut Context<Self>,
    ) {
        let valid = self
            .title_generation_model()
            .and_then(|model| model.reasoning.as_ref())
            .is_some_and(|reasoning| {
                reasoning
                    .preset_options()
                    .iter()
                    .any(|(id, _)| id == &preset)
            });
        if !valid {
            return;
        }
        if self
            .data
            .snapshot
            .settings
            .title_generation_reasoning_preset
            .as_deref()
            == Some(&preset)
        {
            cx.notify();
            return;
        }
        self.data
            .snapshot
            .settings
            .title_generation_reasoning_preset = Some(preset);
        self.save_settings(cx);
        cx.notify();
    }
}
