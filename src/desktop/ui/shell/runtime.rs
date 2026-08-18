use std::collections::HashMap;

use super::*;

pub(super) fn prepare_render(
    app: &mut OneChat,
    window: &mut Window,
    cx: &mut Context<OneChat>,
) -> HashMap<String, String> {
    let theme_color = app.settings().theme_color.clone();
    component_theme::sync_component_theme(
        app.theme(),
        &theme_color,
        &mut app.applied_component_theme,
        window,
        cx,
    );
    component_theme::sync_fonts(
        &app.settings().ui_font_families,
        &app.settings().code_font_families,
        cx,
    );
    settings::sync_controls(app, window, cx);
    inspector::sync_controls(app, window, cx);
    app.sync_translation_prompt_controls(window, cx);
    app.sync_tts_controls(window, cx);
    let playback_progress = app.playback.seek_preview.unwrap_or_else(|| {
        if app.playback.snapshot.duration_ms == 0 {
            0.0
        } else {
            app.playback.snapshot.position_ms as f32 / app.playback.snapshot.duration_ms as f32
        }
    });
    crate::desktop::ui::controls::sync_slider(
        &app.playback.seek_slider,
        playback_progress,
        window,
        cx,
    );

    if let Some(message) = app.tts.completion_notice.take() {
        window.push_notification(Notification::success(message).title("Text to Speech"), cx);
    }

    if let Some(message) = app.data.error.take() {
        window.push_notification(
            Notification::error(message)
                .title("OneChat")
                .id::<AppErrorNotification>()
                .autohide(false),
            cx,
        );
    }

    if let Some(pending) = app.navigation.pending_focus.take() {
        if pending == PendingFocus::Root {
            window.focus(&app.root_focus, cx);
        }
        let focus = match pending {
            PendingFocus::Root => None,
            PendingFocus::SystemPrompt => app
                .chat
                .system_prompt_editor
                .as_ref()
                .map(|input| input.read(cx).focus_handle(cx)),
            PendingFocus::AssistantOpening => app
                .chat
                .assistant_opening_editor
                .as_ref()
                .map(|input| input.read(cx).focus_handle(cx)),
            PendingFocus::SettingsPrompt => {
                if let Some(workspace) = &app.settings_ui.prompt_preset_workspace {
                    if workspace.is_editing() {
                        if workspace.editor.original_name().is_none() {
                            Some(workspace.editor.name.read(cx).focus_handle(cx))
                        } else {
                            Some(workspace.editor.focus_handle(workspace.section, cx))
                        }
                    } else {
                        None
                    }
                } else if let Some(editor) = &app.settings_ui.prompt_variable_editor {
                    Some(editor.focus_handle(cx))
                } else if let Some(editor) = &app.settings_ui.title_prompt_editor {
                    Some(editor.read(cx).focus_handle(cx))
                } else {
                    app.settings_ui
                        .translation_system_prompt_editor
                        .as_ref()
                        .map(|input| input.read(cx).focus_handle(cx))
                }
            }
            PendingFocus::MessageEditor => app
                .active_message_editor()
                .map(|input| input.read(cx).focus_handle(cx)),
            PendingFocus::Composer if app.navigation.page == Page::Chat => {
                Some(app.chat.composer.read(cx).focus_handle(cx))
            }
            PendingFocus::Composer => None,
        };
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
    }

    if app.navigation.page == Page::Chat {
        app.resolve_pending_search_jump(window, cx);
        app.advance_message_scroll(window);
        app.animated_titles(window)
    } else {
        HashMap::new()
    }
}
