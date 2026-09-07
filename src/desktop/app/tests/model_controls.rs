use super::*;
use crate::{desktop::ui::settings::ModelEditor, providers::AvailableModel};

#[gpui::test]
fn model_discovery_refreshes_options_without_overwriting_unsaved_fields(cx: &mut TestAppContext) {
    let (_directory, app, window) = app(cx);
    let mut visual = VisualTestContext::from_window(window.into(), cx);
    visual.draw(
        point(px(0.0), px(0.0)),
        size(
            AvailableSpace::Definite(px(800.0)),
            AvailableSpace::Definite(px(600.0)),
        ),
        |window, cx| {
            app.update(cx, |app, cx| {
                let provider = Provider::new("Provider", ProviderKind::OpenAi);
                app.services.storage.insert_provider(&provider).unwrap();
                let model = Model::new(&provider.id, "model", "Model", provider.kind);
                let catalog = app.services.storage.insert_model(&model).unwrap();
                app.apply_model_catalog(catalog, cx);
                app.navigation.page = Page::Settings;
                let mut editor =
                    ModelEditor::new(provider.id, provider.kind, Some(model), window, cx);
                editor
                    .display_name
                    .update(cx, |input, cx| input.set_value("unsaved name", window, cx));
                editor
                    .context_window
                    .update(cx, |input, cx| input.set_value("99999", window, cx));
                editor.finish_fetch(
                    vec![AvailableModel {
                        id: "model".into(),
                        reasoning: None,
                        tools: true,
                        vision: true,
                        audio_input: false,
                        context_window_tokens: Some(128_000),
                    }],
                    cx,
                );
                assert!(editor.combobox_dirty);
                app.settings_ui.model_editor = Some(editor);

                let _ = shell::render(app, window, cx);

                let editor = app.settings_ui.model_editor.as_ref().unwrap();
                assert!(!editor.combobox_dirty);
                assert_eq!(editor.remote_id(cx), "model");
                assert_eq!(editor.display_name.read(cx).value(), "unsaved name");
                assert_eq!(editor.context_window.read(cx).value(), "99999");
                assert!(editor.capabilities.vision);
            });
            div()
        },
    );
}
