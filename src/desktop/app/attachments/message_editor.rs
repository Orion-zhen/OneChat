use super::*;

impl OneChat {
    pub(crate) fn add_message_edit_attachments(&mut self, turn_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.recording_active() {
            return;
        }
        let Some(model) = self.current_model() else {
            self.data.error = Some("Choose a model before adding attachments.".into());
            cx.notify();
            return;
        };
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        let Some(editor) = self.chat.message_editor.as_ref().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id),
        ) else {
            return;
        };
        if editor.attachment_load_id.is_some() {
            return;
        }
        let attachment_count = editor.attachments.len() + editor.attachment_drafts.len();
        if attachment_count >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments."
            ));
            cx.notify();
            return;
        }

        let vision = model.capabilities.vision;
        let audio_input = model.capabilities.audio_input;
        let parse_document_images = self.settings().parse_document_images;
        let remaining = MAX_ATTACHMENTS - attachment_count;
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select Attachments".into()),
        });
        let load_id = new_id("attachment-load");
        if let Some(editor) = self.chat.message_editor.as_mut() {
            editor.attachment_load_id = Some(load_id.clone());
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let selected = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        let Some(editor) = this.chat.message_editor.as_mut().filter(|editor| {
                            matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id)
                                && editor.attachment_load_id.as_deref() == Some(&load_id)
                        }) else {
                            return;
                        };
                        editor.attachment_load_id = None;
                        cx.notify();
                    });
                    return;
                }
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        let Some(editor) = this.chat.message_editor.as_mut().filter(|editor| {
                            matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id)
                                && editor.attachment_load_id.as_deref() == Some(&load_id)
                        }) else {
                            return;
                        };
                        editor.attachment_load_id = None;
                        this.data.error = Some(format!("Could not open attachments: {error}"));
                        cx.notify();
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        let Some(editor) = this.chat.message_editor.as_mut().filter(|editor| {
                            matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id)
                                && editor.attachment_load_id.as_deref() == Some(&load_id)
                        }) else {
                            return;
                        };
                        editor.attachment_load_id = None;
                        this.data.error =
                            Some(format!("Attachment picker closed unexpectedly: {error}"));
                        cx.notify();
                    });
                    return;
                }
            };

            let result = cx
                .background_spawn(async move {
                    if selected.len() > remaining {
                        return Err(format!(
                            "Select at most {remaining} more attachment{}.",
                            if remaining == 1 { "" } else { "s" }
                        ));
                    }
                    selected
                        .into_iter()
                        .map(|path| {
                            load_attachment(&path, vision, audio_input, parse_document_images)
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.current_conversation().map(|value| &value.id) != Some(&conversation_id) {
                    return;
                }
                let capabilities = this
                    .current_model()
                    .map(|model| model.capabilities.clone())
                    .unwrap_or_default();
                let capability_error = result
                    .as_ref()
                    .ok()
                    .and_then(|attachments| attachment_capability_error(&capabilities, attachments));
                let Some(editor) = this.chat.message_editor.as_mut().filter(|editor| {
                    matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id)
                        && editor.attachment_load_id.as_deref() == Some(&load_id)
                }) else {
                    return;
                };
                editor.attachment_load_id = None;
                match result {
                    Ok(attachments) if capability_error.is_none() => {
                        for attachment in &attachments {
                            if let Some(preview) = attachment_preview(attachment) {
                                editor
                                    .attachment_previews
                                    .insert(attachment.id.clone(), preview);
                            }
                        }
                        editor.attachment_drafts.extend(attachments);
                    }
                    Ok(_) => this.data.error = capability_error.map(str::to_owned),
                    Err(error) => this.data.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_message_edit_attachment(
        &mut self,
        turn_id: String,
        attachment_id: String,
        cx: &mut Context<Self>,
    ) {
        self.stop_audio_playback_if(&attachment_id);
        let Some(editor) = self.chat.message_editor.as_mut().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id),
        ) else {
            return;
        };
        editor
            .attachments
            .retain(|attachment| attachment.id != attachment_id);
        editor
            .attachment_drafts
            .retain(|attachment| attachment.id != attachment_id);
        editor.attachment_previews.remove(&attachment_id);
        cx.notify();
    }
}
