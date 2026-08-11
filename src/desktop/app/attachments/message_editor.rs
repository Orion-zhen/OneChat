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

        let options = LoadManyOptions {
            remaining: MAX_ATTACHMENTS - attachment_count,
            vision: model.capabilities.vision,
            audio_input: model.capabilities.audio_input,
            parse_document_images: self.settings().parse_document_images,
        };
        let paths = cx.prompt_for_paths(attachment_path_prompt_options());
        let load_id = new_id("attachment-load");
        if let Some(editor) = self.chat.message_editor.as_mut() {
            editor.attachment_load_id = Some(load_id.clone());
        }
        cx.notify();

        cx.spawn(async move |this, cx| {
            let selected = match normalize_attachment_path_selection(paths.await) {
                AttachmentPathSelection::Selected(paths) => paths,
                selection => {
                    let error = match selection {
                        AttachmentPathSelection::Cancelled => None,
                        AttachmentPathSelection::Error(error) => Some(error),
                        AttachmentPathSelection::Selected(_) => unreachable!(),
                    };
                    let _ = this.update(cx, |this, cx| {
                        let Some(editor) = this.chat.message_editor.as_mut().filter(|editor| {
                            matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id)
                                && editor.attachment_load_id.as_deref() == Some(&load_id)
                        }) else {
                            return;
                        };
                        editor.attachment_load_id = None;
                        if let Some(error) = error {
                            this.data.error = Some(error);
                        }
                        cx.notify();
                    });
                    return;
                }
            };

            let result = cx
                .background_spawn(async move { load_many(selected, options) })
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
