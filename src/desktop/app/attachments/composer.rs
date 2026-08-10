use super::*;

impl OneChat {
    pub(crate) fn add_attachments(&mut self, cx: &mut Context<Self>) {
        let Some(load) = self.begin_composer_attachment_load(cx) else {
            return;
        };
        let revision = load.revision;
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select Attachments".into()),
        });

        cx.spawn(async move |this, cx| {
            let selected = match paths.await {
                Ok(Ok(Some(paths))) => paths,
                Ok(Ok(None)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.cancel_composer_attachment_load(revision, None, cx)
                    });
                    return;
                }
                Ok(Err(error)) => {
                    let _ = this.update(cx, |this, cx| {
                        this.cancel_composer_attachment_load(
                            revision,
                            Some(format!("Could not open attachments: {error}")),
                            cx,
                        )
                    });
                    return;
                }
                Err(error) => {
                    let _ = this.update(cx, |this, cx| {
                        this.cancel_composer_attachment_load(
                            revision,
                            Some(format!("Attachment picker closed unexpectedly: {error}")),
                            cx,
                        )
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
        if self.is_current_generating() || self.chat.attachments_loading {
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
            vision: model.capabilities.vision,
            parse_document_images: self.settings().parse_document_images,
            remaining: MAX_ATTACHMENTS - self.chat.attachments.len(),
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
            vision,
            parse_document_images,
            remaining,
            revision,
        } = load;
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if paths.len() > remaining {
                        return Err(format!(
                            "Select at most {remaining} more attachment{}.",
                            if remaining == 1 { "" } else { "s" }
                        ));
                    }
                    paths
                        .into_iter()
                        .map(|path| {
                            if path.is_dir() {
                                Err(format!(
                                    "Folders cannot be added as attachments: {}",
                                    path.display()
                                ))
                            } else {
                                load_attachment(&path, vision, parse_document_images)
                            }
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.chat.attachments_revision != revision
                    || this.current_conversation().map(|value| &value.id) != Some(&conversation_id)
                {
                    return;
                }
                this.chat.attachments_loading = false;
                match result {
                    Ok(attachments)
                        if this.current_model().is_some_and(|model| {
                            !model.capabilities.vision
                                && attachments
                                    .iter()
                                    .any(|attachment| attachment.kind.requires_vision())
                        }) =>
                    {
                        this.data.error =
                            Some("The selected model only accepts text attachments.".into());
                    }
                    Ok(attachments) => {
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
        if self.is_current_generating() || self.chat.attachments_loading {
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
        self.chat
            .attachments
            .retain(|attachment| attachment.id != id);
        self.chat.attachment_previews.remove(&id);
        cx.notify();
    }
}
