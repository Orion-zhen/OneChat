use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use gpui::{AppContext as _, ClipboardItem, Context, Image, ImageFormat, Window};
use gpui_component::{WindowExt as _, notification::Notification};

use super::{ExportNotice, OneChat, prompt_export_path, show_export_result};
use crate::{
    application::export::{ExportTheme, conversation_html, conversation_markdown},
    desktop::{html_snapshot, ui::theme::component_mode},
    domain::{
        AssistantResponse, AttachmentFile, AttachmentFileKind, Conversation, Turn, now_timestamp,
    },
};

struct ConversationExport<'a> {
    conversation: &'a Conversation,
    turns: Vec<(&'a Turn, &'a AssistantResponse)>,
}

impl OneChat {
    pub(crate) fn copy_conversation_markdown(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(export) = self.conversation_export(response_id) else {
            return;
        };
        let markdown = conversation_markdown(export.conversation, &export.turns, now_timestamp());
        cx.write_to_clipboard(ClipboardItem::new_string(markdown));
        window.push_notification(
            Notification::success("The visible conversation was copied as Markdown.")
                .title("Conversation copied"),
            cx,
        );
    }

    pub(crate) fn copy_conversation_png(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(html) =
            self.conversation_export_html(response_id, export_theme(self, window), now_timestamp())
        else {
            return;
        };
        cx.spawn_in(window, async move |this, cx| {
            let result = html_snapshot::render_png(html, cx).await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok(png) => {
                    let image = Image::from_bytes(ImageFormat::Png, png);
                    cx.write_to_clipboard(ClipboardItem::new_image(&image));
                    window.push_notification(
                        Notification::success("The visible conversation was copied as a PNG.")
                            .title("Conversation copied"),
                        cx,
                    );
                }
                Err(error) => {
                    this.data.error = Some(format!("Could not copy conversation as PNG: {error}"));
                    cx.notify();
                }
            });
        })
        .detach();
    }

    pub(crate) fn export_conversation_markdown(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(export) = self.conversation_export(response_id) else {
            return;
        };
        let title = export.conversation.title.clone();
        let markdown = conversation_markdown(export.conversation, &export.turns, now_timestamp());
        self.export_file(
            &title,
            "md",
            ExportNotice::CONVERSATION,
            move |path| std::fs::write(path, markdown).map_err(|error| error.to_string()),
            window,
            cx,
        );
    }

    pub(crate) fn export_conversation_html(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(export) = self.conversation_export(response_id) else {
            return;
        };
        let title = export.conversation.title.clone();
        let html = self.export_html(&export, ExportTheme::Auto, now_timestamp());
        self.export_file(
            &title,
            "html",
            ExportNotice::CONVERSATION,
            move |path| std::fs::write(path, html).map_err(|error| error.to_string()),
            window,
            cx,
        );
    }

    pub(crate) fn export_conversation_png(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(export) = self.conversation_export(response_id) else {
            return;
        };
        let title = export.conversation.title.clone();
        let html = self.export_html(&export, export_theme(self, window), now_timestamp());
        let path = prompt_export_path(&title, "png", cx);
        cx.spawn_in(window, async move |this, cx| {
            let Some(path) = path.await else {
                return;
            };
            let notification_path = path.clone();
            let result = match html_snapshot::render_png(html, cx).await {
                Ok(png) => cx
                    .background_spawn(async move { std::fs::write(path, png) })
                    .await
                    .map_err(|error| error.to_string()),
                Err(error) => Err(error),
            };
            show_export_result(
                this,
                cx,
                notification_path,
                result,
                ExportNotice::CONVERSATION,
            );
        })
        .detach();
    }

    pub(crate) fn export_conversation_archive(
        &mut self,
        response_id: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(export) = self.conversation_export(response_id) else {
            return;
        };
        let conversation_id = export.conversation.id.clone();
        let title = export.conversation.title.clone();
        let markdown = conversation_markdown(export.conversation, &export.turns, now_timestamp());
        let storage = self.services.storage.clone();
        self.export_file(
            &title,
            "zip",
            ExportNotice::CONVERSATION,
            move |path| {
                storage
                    .export_conversation_archive(&conversation_id, &markdown, &path)
                    .map_err(|error| error.to_string())
            },
            window,
            cx,
        );
    }

    fn conversation_export(&self, response_id: &str) -> Option<ConversationExport<'_>> {
        let conversation = self.current_conversation()?;
        let turns = self
            .current_turns()
            .into_iter()
            .filter_map(|turn| self.visible_response(turn).map(|response| (turn, response)))
            .collect::<Vec<_>>();
        if turns.last()?.1.id != response_id {
            return None;
        }
        Some(ConversationExport {
            conversation,
            turns,
        })
    }

    fn conversation_export_html(
        &self,
        response_id: &str,
        theme: ExportTheme,
        exported_at: i64,
    ) -> Option<String> {
        let export = self.conversation_export(response_id)?;
        Some(self.export_html(&export, theme, exported_at))
    }

    fn export_html(
        &self,
        export: &ConversationExport<'_>,
        theme: ExportTheme,
        exported_at: i64,
    ) -> String {
        let conversation_id = &export.conversation.id;
        conversation_html(
            export.conversation,
            &export.turns,
            exported_at,
            &self.settings().theme_color,
            theme,
            |file| self.attachment_data_url(conversation_id, file),
        )
    }

    fn attachment_data_url(&self, conversation_id: &str, file: &AttachmentFile) -> Option<String> {
        if file.kind != AttachmentFileKind::Image || !file.media_type.starts_with("image/") {
            return None;
        }
        let path = self
            .services
            .storage
            .attachment_path(conversation_id, &file.path)
            .ok()?;
        let bytes = std::fs::read(path).ok()?;
        Some(format!(
            "data:{};base64,{}",
            file.media_type,
            BASE64.encode(bytes)
        ))
    }
}

fn export_theme(app: &OneChat, window: &Window) -> ExportTheme {
    if component_mode(app.theme(), window.appearance()).is_dark() {
        ExportTheme::Dark
    } else {
        ExportTheme::Light
    }
}
