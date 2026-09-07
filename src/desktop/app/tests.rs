mod model_controls;

use super::*;
use gpui::{AvailableSpace, TestAppContext, VisualTestContext, WindowHandle, div, point, px, size};
use gpui_component::Root;
use tempfile::{TempDir, tempdir};
use tokio_util::sync::CancellationToken;

use crate::{
    domain::{Model, Provider, ProviderKind},
    mcp::McpManager,
    storage::Storage,
};

fn app(cx: &mut TestAppContext) -> (TempDir, Entity<OneChat>, WindowHandle<Root>) {
    let directory = tempdir().unwrap();
    let storage = Arc::new(
        Storage::open(
            directory.path().join("settings.jsonc"),
            directory.path().join("state"),
        )
        .unwrap(),
    );
    let runtime = Arc::new(tokio::runtime::Runtime::new().unwrap());
    let mcp = Arc::new(McpManager::new(directory.path().join("mcp.jsonc")));
    cx.update(crate::desktop::ui::init);
    let mut app = None;
    let window = cx.add_window(|window, cx| {
        let entity = cx.new(|cx| OneChat::build(storage, runtime, mcp, window, cx));
        app = Some(entity.clone());
        Root::new(entity, window, cx)
    });
    (directory, app.unwrap(), window)
}

#[gpui::test]
fn conversation_reset_preserves_background_work_but_replaces_view_state(cx: &mut TestAppContext) {
    let (_directory, app, window) = app(cx);
    window
        .update(cx, |_, _, cx| {
            app.update(cx, |app, _| {
                let composer = app.chat.composer.entity_id();
                let token = CancellationToken::new();
                assert!(app.chat.generations.start(
                    "background".into(),
                    "request".into(),
                    "response".into(),
                    token.clone()
                ));
                app.chat
                    .presentation
                    .thinking_started_at
                    .insert("request".into(), Instant::now());
                app.chat.presentation.markdown_documents.insert(
                    "chat-output".into(),
                    CachedMarkdown {
                        source: "chat".into(),
                        document: MarkdownDocument::parse("chat"),
                    },
                );
                app.translation
                    .output
                    .presentation
                    .markdown_documents
                    .insert(
                        "translation-output".into(),
                        CachedMarkdown {
                            source: "translation".into(),
                            document: MarkdownDocument::parse("translation"),
                        },
                    );
                app.chat
                    .visible_response_ids
                    .insert("turn".into(), "response".into());
                app.chat.expanded_error_ids.insert("response".into());
                app.chat.attachments_loading = true;
                app.chat.follow_latest = false;
                app.chat.context_usage_popover_open = true;
                app.chat.attachments_revision = 7;
                app.chat.generation_config_save_revision = 11;

                app.chat.reset_conversation();

                assert_eq!(app.chat.composer.entity_id(), composer);
                assert!(app.chat.generations.is_active("background"));
                assert!(!token.is_cancelled());
                assert!(
                    app.chat
                        .presentation
                        .thinking_started_at
                        .contains_key("request")
                );
                assert!(app.chat.presentation.markdown_documents.is_empty());
                assert!(app.chat.visible_response_ids.is_empty());
                assert!(app.chat.expanded_error_ids.is_empty());
                assert!(!app.chat.attachments_loading);
                assert!(app.chat.follow_latest);
                assert!(!app.chat.context_usage_popover_open);
                assert_eq!(app.chat.attachments_revision, 8);
                assert_eq!(app.chat.generation_config_save_revision, 12);
                assert!(app.chat.controls_dirty);
                assert!(
                    app.translation
                        .output
                        .presentation
                        .markdown_for("translation-output", "translation")
                        .is_some()
                );
            });
        })
        .unwrap();
}

#[gpui::test]
fn shell_renders_only_the_visible_pages_pending_controls(cx: &mut TestAppContext) {
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
                app.settings_ui.controls_dirty = true;
                app.chat.controls_dirty = true;
                app.tts
                    .controller
                    .update_config(|config| config.segmentation.min_chars = 3);
                let _ = shell::render(app, window, cx);
                assert!(app.settings_ui.controls_dirty);
                assert!(app.chat.controls_dirty);
                assert_ne!(
                    app.tts.controls.tuning.min_chars.read(cx).value().start(),
                    3.0
                );

                app.navigation.page = Page::Settings;
                let _ = shell::render(app, window, cx);
                assert!(!app.settings_ui.controls_dirty);
                assert!(app.chat.controls_dirty);
                assert_ne!(
                    app.tts.controls.tuning.min_chars.read(cx).value().start(),
                    3.0
                );
                let _ = shell::render(app, window, cx);
                assert!(!app.settings_ui.controls_dirty);

                app.navigation.page = Page::Tts;
                let _ = shell::render(app, window, cx);
                assert_eq!(
                    app.tts.controls.tuning.min_chars.read(cx).value().start(),
                    3.0
                );
                assert!(app.chat.controls_dirty);

                app.navigation.page = Page::Chat;
                app.navigation.inspector_open = true;
                let _ = shell::render(app, window, cx);
                assert!(!app.chat.controls_dirty);
            });
            div()
        },
    );
}

#[gpui::test]
fn catalog_and_translation_prompt_changes_refresh_their_controls(cx: &mut TestAppContext) {
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
                app.navigation.page = Page::Settings;
                let _ = shell::render(app, window, cx);
                assert!(!app.settings_ui.controls_dirty);
                let provider = Provider::new("Provider", ProviderKind::OpenAi);
                app.services.storage.insert_provider(&provider).unwrap();
                let model = Model::new(&provider.id, "model", "Model", provider.kind);
                let catalog = app.services.storage.insert_model(&model).unwrap();
                app.apply_model_catalog(catalog, cx);
                app.data.snapshot.settings.primary_model_id = Some(model.id.clone());
                assert!(app.settings_ui.controls_dirty);
                let _ = shell::render(app, window, cx);
                assert_eq!(
                    app.settings_ui
                        .primary_model_select
                        .read(cx)
                        .selected_value(),
                    Some(&Some(model.id.clone()))
                );

                app.set_translation_prompts("new system {{text}}".into(), "new user".into(), cx);
                app.open_translation_prompt_editor(TranslationPromptKind::System, window, cx);
                assert_eq!(
                    app.translation.controls.system_prompt.read(cx).value(),
                    "new system {{text}}"
                );
                assert_eq!(
                    app.translation.controls.user_prompt.read(cx).value(),
                    "new user"
                );
            });
            div()
        },
    );
}
