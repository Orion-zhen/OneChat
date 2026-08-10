use super::*;

impl OneChat {
    pub(crate) fn select_default_model(
        &mut self,
        role: DefaultModelRole,
        model_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(model_id) = model_id.as_deref() {
            let Some(model) = self
                .data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
            else {
                return;
            };
            if let Err(reason) = self.model_availability(model) {
                self.data.error = Some(format!("Model is unavailable: {reason}."));
                cx.notify();
                return;
            }
        } else if role == DefaultModelRole::Primary {
            return;
        }

        let updates_title_reasoning = role == DefaultModelRole::TitleGeneration
            || (role == DefaultModelRole::Primary
                && self
                    .data
                    .snapshot
                    .settings
                    .title_generation_model_id
                    .is_none());
        let stored_id = match role {
            DefaultModelRole::Primary => &mut self.data.snapshot.settings.primary_model_id,
            DefaultModelRole::TitleGeneration => {
                &mut self.data.snapshot.settings.title_generation_model_id
            }
        };
        if *stored_id == model_id {
            cx.notify();
            return;
        }
        *stored_id = model_id;

        if updates_title_reasoning {
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

        self.save_settings(cx);
        cx.notify();
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
