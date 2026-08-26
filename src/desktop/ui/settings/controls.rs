use super::*;

pub(crate) fn sync_controls(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) {
    let theme_color = crate::desktop::ui::theme::parse_theme_color(&app.settings().theme_color);
    app.settings_ui.theme_color.sync(theme_color, window, cx);

    let message_font_size = app.settings().message_font_size();
    sync_slider(
        &app.settings_ui.message_font_size_slider,
        message_font_size,
        window,
        cx,
    );

    let opacity = app.settings().background_opacity();
    sync_slider(
        &app.settings_ui.background_opacity_slider,
        opacity,
        window,
        cx,
    );

    let ratio = app.settings().message_width_ratio();
    sync_slider(&app.settings_ui.message_width_slider, ratio, window, cx);

    let history_limit = app.settings().history_limit.slider_value();
    sync_slider(
        &app.settings_ui.history_limit_slider,
        history_limit,
        window,
        cx,
    );

    let primary_items = primary_model_items(app);
    let primary_changed = primary_items != app.settings_ui.synced_primary_models;
    if primary_changed {
        app.settings_ui
            .synced_primary_models
            .clone_from(&primary_items);
        app.settings_ui
            .primary_model_select
            .update(cx, |select, cx| select.set_items(primary_items, window, cx));
    }
    let primary_value = app.settings().primary_model_id.clone().map(Some);
    if primary_changed
        || app
            .settings_ui
            .primary_model_select
            .read(cx)
            .selected_value()
            .cloned()
            != primary_value
    {
        app.settings_ui
            .primary_model_select
            .update(cx, |select, cx| match primary_value.as_ref() {
                Some(value) => select.set_selected_value(value, window, cx),
                None => select.set_selected_index(None, window, cx),
            });
    }

    let title_items = title_model_items(app);
    let title_changed = title_items != app.settings_ui.synced_title_models;
    if title_changed {
        app.settings_ui.synced_title_models.clone_from(&title_items);
        app.settings_ui
            .title_model_select
            .update(cx, |select, cx| select.set_items(title_items, window, cx));
    }
    let title_value = app.settings().title_generation_model.clone();
    if title_changed
        || app.settings_ui.title_model_select.read(cx).selected_value() != Some(&title_value)
    {
        app.settings_ui.title_model_select.update(cx, |select, cx| {
            select.set_selected_value(&title_value, window, cx)
        });
    }

    let (title_reasoning_items, title_reasoning_value) = app
        .title_generation_model()
        .and_then(|model| model.reasoning.as_ref())
        .map(|reasoning| {
            let options = reasoning.preset_options();
            let selected = app
                .settings()
                .title_generation_reasoning_preset
                .clone()
                .filter(|selected| options.iter().any(|(id, _)| id == selected))
                .unwrap_or_else(|| reasoning.default_preset().to_string());
            let items = options
                .into_iter()
                .map(|(id, label)| ReasoningPresetSelectItem::new(id, label))
                .collect::<Vec<_>>();
            (items, Some(selected))
        })
        .unwrap_or_default();
    let title_reasoning_changed =
        title_reasoning_items != app.settings_ui.synced_title_reasoning_presets;
    if title_reasoning_changed {
        app.settings_ui
            .synced_title_reasoning_presets
            .clone_from(&title_reasoning_items);
    }
    if title_reasoning_changed
        || app
            .settings_ui
            .title_reasoning_select
            .read(cx)
            .selected_value()
            .cloned()
            != title_reasoning_value
    {
        app.settings_ui
            .title_reasoning_select
            .update(cx, |select, cx| {
                if title_reasoning_changed {
                    select.set_items(title_reasoning_items, window, cx);
                }
                match title_reasoning_value.as_ref() {
                    Some(value) => select.set_selected_value(value, window, cx),
                    None => select.set_selected_index(None, window, cx),
                }
            });
    }

    let prompt_items = default_prompt_items(app);
    let prompts_changed = prompt_items != app.settings_ui.synced_prompts;
    if prompts_changed {
        app.settings_ui.synced_prompts.clone_from(&prompt_items);
        app.settings_ui
            .default_prompt_select
            .update(cx, |select, cx| select.set_items(prompt_items, window, cx));
    }
    let prompt_value = Some(app.settings().default_prompt_preset.clone());
    if prompts_changed
        || app
            .settings_ui
            .default_prompt_select
            .read(cx)
            .selected_value()
            .cloned()
            != prompt_value
    {
        app.settings_ui
            .default_prompt_select
            .update(cx, |select, cx| {
                select.set_selected_value(&app.settings().default_prompt_preset.clone(), window, cx)
            });
    }

    if let Some(editor) = &mut app.settings_ui.model_editor {
        editor.sync_combobox(window, cx);
    }
}

fn primary_model_items(app: &OneChat) -> Vec<DefaultModelItem> {
    let selected_id = app.settings().primary_model_id.as_deref();
    let mut items = Vec::new();
    for model in &app.data.snapshot.models {
        let Some((provider, detail, disabled)) = model_item(app, model, selected_id) else {
            continue;
        };
        items.push(DefaultModelItem::new(
            Some(model.id.clone()),
            model.display_name.clone(),
            provider,
            detail,
            disabled,
        ));
    }
    items
}

fn title_model_items(app: &OneChat) -> Vec<TitleModelItem> {
    let selected_id = app.settings().title_generation_model.model_id();
    let mut items = vec![
        TitleModelItem::new(
            TitleModelSource::Current,
            "Use Current Model",
            None,
            "Follow the conversation settings",
            false,
        ),
        TitleModelItem::new(
            TitleModelSource::Primary,
            "Use Primary Model",
            None,
            "Follow the primary model setting",
            false,
        ),
    ];
    for model in &app.data.snapshot.models {
        let Some((provider, detail, disabled)) = model_item(app, model, selected_id) else {
            continue;
        };
        items.push(TitleModelItem::new(
            TitleModelSource::Model(model.id.clone()),
            model.display_name.clone(),
            provider,
            detail,
            disabled,
        ));
    }
    if let Some(selected_id) = selected_id
        && !items
            .iter()
            .any(|item| item.value() == &TitleModelSource::Model(selected_id.to_string()))
    {
        items.push(TitleModelItem::new(
            TitleModelSource::Model(selected_id.to_string()),
            format!("Missing · {selected_id}"),
            None,
            "The configured model no longer exists",
            true,
        ));
    }
    items
}

fn model_item(
    app: &OneChat,
    model: &Model,
    selected_id: Option<&str>,
) -> Option<(Option<SharedString>, String, bool)> {
    let availability = app.model_availability(model);
    if availability.is_err() && selected_id != Some(model.id.as_str()) {
        return None;
    }
    let provider = app
        .provider_for_model(model)
        .map(|provider| provider.name.as_str())
        .unwrap_or("Missing provider");
    let detail = availability.as_ref().map_or_else(
        |reason| format!("Unavailable · {reason}"),
        |_| format!("{} · {provider}", model.remote_id),
    );
    Some((Some(provider.into()), detail, availability.is_err()))
}

fn default_prompt_items(app: &OneChat) -> Vec<PromptSelectItem> {
    let selected = app.settings().default_prompt_preset.as_deref();
    let mut items = vec![PromptSelectItem::new(None, "No Prompt Preset", false)];
    items.extend(app.data.snapshot.prompt_presets.iter().map(|preset| {
        PromptSelectItem::new(Some(preset.name.clone()), preset.name.clone(), false)
    }));
    if let Some(selected) = selected
        && !app
            .data
            .snapshot
            .prompt_presets
            .iter()
            .any(|preset| preset.name == selected)
    {
        items.push(PromptSelectItem::new(
            Some(selected.to_string()),
            format!("Missing · {selected}"),
            true,
        ));
    }
    items
}
