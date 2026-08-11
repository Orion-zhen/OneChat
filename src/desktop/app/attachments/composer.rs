use super::*;

impl OneChat {
    pub(crate) fn add_attachments(&mut self, cx: &mut Context<Self>) {
        let Some(load) = self.begin_composer_attachment_load(cx) else {
            return;
        };
        let revision = load.revision;
        let paths = cx.prompt_for_paths(attachment_path_prompt_options());

        cx.spawn(async move |this, cx| {
            let selected = match normalize_attachment_path_selection(paths.await) {
                AttachmentPathSelection::Selected(paths) => paths,
                AttachmentPathSelection::Cancelled => {
                    let _ = this.update(cx, |this, cx| {
                        this.cancel_composer_attachment_load(revision, None, cx)
                    });
                    return;
                }
                AttachmentPathSelection::Error(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.cancel_composer_attachment_load(revision, Some(error), cx)
                    });
                    return;
                }
            };

            let _ = this.update(cx, |this, cx| {
                this.load_composer_attachment_paths(selected, load, cx)
            });
        })
        .detach();
    }

    pub(crate) fn add_dropped_attachments(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let Some(load) = self.begin_composer_attachment_load(cx) else {
            return;
        };
        self.load_composer_attachment_paths(paths, load, cx);
    }

    fn begin_composer_attachment_load(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<ComposerAttachmentLoad> {
        if self.is_current_generating() || self.recording_active() || self.chat.attachments_loading
        {
            return None;
        }
        let Some(model) = self.current_model() else {
            self.data.error = Some("Choose a model before adding attachments.".into());
            cx.notify();
            return None;
        };
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            self.data.error = Some("Create or select a conversation first.".into());
            cx.notify();
            return None;
        };
        if self.chat.attachments.len() >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments."
            ));
            cx.notify();
            return None;
        }

        let load = ComposerAttachmentLoad {
            conversation_id,
            options: LoadManyOptions {
                remaining: MAX_ATTACHMENTS - self.chat.attachments.len(),
                vision: model.capabilities.vision,
                audio_input: model.capabilities.audio_input,
                parse_document_images: self.settings().parse_document_images,
            },
            revision: self.chat.attachments_revision.wrapping_add(1),
        };
        self.chat.attachments_loading = true;
        self.chat.attachments_revision = load.revision;
        cx.notify();
        Some(load)
    }

    fn cancel_composer_attachment_load(
        &mut self,
        revision: u64,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.chat.attachments_revision != revision {
            return;
        }
        self.chat.attachments_loading = false;
        if let Some(error) = error {
            self.data.error = Some(error);
        }
        cx.notify();
    }

    fn load_composer_attachment_paths(
        &mut self,
        paths: Vec<PathBuf>,
        load: ComposerAttachmentLoad,
        cx: &mut Context<Self>,
    ) {
        let ComposerAttachmentLoad {
            conversation_id,
            options,
            revision,
        } = load;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { load_many(paths, options) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.attachments_revision != revision
                    || this.current_conversation().map(|value| &value.id) != Some(&conversation_id)
                {
                    return;
                }
                this.chat.attachments_loading = false;
                let capabilities = this
                    .current_model()
                    .map(|model| model.capabilities.clone())
                    .unwrap_or_default();
                match result {
                    Ok(attachments) => {
                        if let Some(error) =
                            attachment_capability_error(&capabilities, &attachments)
                        {
                            this.data.error = Some(error.into());
                            cx.notify();
                            return;
                        }
                        for attachment in &attachments {
                            if let Some(preview) = attachment_preview(attachment) {
                                this.chat
                                    .attachment_previews
                                    .insert(attachment.id.clone(), preview);
                            }
                        }
                        this.chat.attachments.extend(attachments);
                    }
                    Err(error) => this.data.error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn paste_composer_image(&mut self, cx: &mut Context<Self>) {
        let Some(image) = cx
            .read_from_clipboard()
            .and_then(|clipboard| clipboard.entries.into_iter().next())
            .and_then(|entry| match entry {
                gpui::ClipboardEntry::Image(image) => Some(image),
                _ => None,
            })
        else {
            return;
        };
        if !self
            .current_model()
            .is_some_and(|model| model.capabilities.vision)
        {
            return;
        }
        cx.stop_propagation();
        if self.is_current_generating() || self.recording_active() || self.chat.attachments_loading
        {
            return;
        }
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.chat.attachments.len() >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments."
            ));
            cx.notify();
            return;
        }

        let number = self.chat.attachments.len() + 1;
        self.chat.attachments_loading = true;
        self.chat.attachments_revision = self.chat.attachments_revision.wrapping_add(1);
        let revision = self.chat.attachments_revision;
        cx.notify();

        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { clipboard_image_attachment(image, number) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.attachments_revision != revision
                    || this.current_conversation().map(|value| &value.id) != Some(&conversation_id)
                {
                    return;
                }
                this.chat.attachments_loading = false;
                if !this
                    .current_model()
                    .is_some_and(|model| model.capabilities.vision)
                {
                    this.data.error =
                        Some("The selected model does not accept pasted images.".into());
                } else {
                    match result {
                        Ok(attachment) => {
                            if let Some(preview) = attachment_preview(&attachment) {
                                this.chat
                                    .attachment_previews
                                    .insert(attachment.id.clone(), preview);
                            }
                            this.chat.attachments.push(attachment);
                        }
                        Err(error) => this.data.error = Some(error),
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn remove_attachment(&mut self, id: String, cx: &mut Context<Self>) {
        self.stop_audio_playback_if(&id);
        self.chat
            .attachments
            .retain(|attachment| attachment.id != id);
        self.chat.attachment_previews.remove(&id);
        cx.notify();
    }
}
