use std::{
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rig_core::{
    OneOrMany,
    completion::Message,
    message::{ImageMediaType, UserContent},
};

use crate::{
    domain::{Attachment, AttachmentDraft, AttachmentFile, AttachmentKind, UserMessage},
    storage::{Result, Storage, StorageError, conflict, missing},
};

use super::{ConversationFile, validate_component};

impl Storage {
    pub fn store_attachments(
        &self,
        conversation_id: &str,
        drafts: &[AttachmentDraft],
    ) -> Result<Vec<Attachment>> {
        let _guard = self.lock()?;
        if !self.conversation_path(conversation_id)?.exists() {
            return Err(missing("conversation", conversation_id));
        }

        let mut created = Vec::with_capacity(drafts.len());
        let result = (|| {
            let mut stored = Vec::with_capacity(drafts.len());
            for draft in drafts {
                validate_component("attachment id", &draft.id)?;
                if draft.files.is_empty() {
                    return Err(StorageError::InvalidData(format!(
                        "attachment has no content: {}",
                        draft.name
                    )));
                }
                let directory = self
                    .conversation_dir(conversation_id)?
                    .join("attachments")
                    .join(&draft.id);
                if directory.exists() {
                    return Err(conflict("attachment", &draft.id));
                }
                fs::create_dir_all(&directory)?;
                created.push(directory.clone());

                let mut files = Vec::with_capacity(draft.files.len());
                for (index, file) in draft.files.iter().enumerate() {
                    if file.extension.is_empty()
                        || !file
                            .extension
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric())
                    {
                        return Err(StorageError::InvalidData(format!(
                            "invalid attachment extension: {}",
                            file.extension
                        )));
                    }
                    let file_name = if draft.files.len() == 1 {
                        format!("content.{}", file.extension)
                    } else {
                        format!("page-{:03}.{}", index + 1, file.extension)
                    };
                    fs::write(directory.join(&file_name), &file.bytes)?;
                    files.push(AttachmentFile {
                        path: format!("attachments/{}/{}", draft.id, file_name),
                        media_type: file.media_type.to_string(),
                    });
                }
                stored.push(Attachment {
                    id: draft.id.clone(),
                    name: draft.name.clone(),
                    kind: draft.kind,
                    files,
                });
            }
            Ok(stored)
        })();
        if result.is_err() {
            for directory in created {
                let _ = fs::remove_dir_all(directory);
            }
        }
        result
    }

    pub fn message_for_user(&self, conversation_id: &str, user: &UserMessage) -> Result<Message> {
        let _guard = self.lock()?;
        let mut content = Vec::new();
        if !user.content.trim().is_empty() {
            content.push(UserContent::text(user.content.clone()));
        }
        for attachment in &user.attachments {
            match attachment.kind {
                AttachmentKind::Text => {
                    let Some(file) = attachment.files.first() else {
                        return Err(StorageError::InvalidData(format!(
                            "text attachment has no content: {}",
                            attachment.name
                        )));
                    };
                    let text =
                        fs::read_to_string(self.attachment_path(conversation_id, &file.path)?)?;
                    content.push(UserContent::text(format!(
                        "<attachment name=\"{}\">\n{}\n</attachment>",
                        attachment.name, text
                    )));
                }
                AttachmentKind::Image => {
                    let Some(file) = attachment.files.first() else {
                        return Err(StorageError::InvalidData(format!(
                            "image attachment has no content: {}",
                            attachment.name
                        )));
                    };
                    content.push(UserContent::text(format!(
                        "Image attachment: {}",
                        attachment.name
                    )));
                    content.push(self.image_content(conversation_id, file)?);
                }
                AttachmentKind::Pdf => {
                    content.push(UserContent::text(format!(
                        "PDF attachment: {} ({} pages)",
                        attachment.name,
                        attachment.files.len()
                    )));
                    for (index, file) in attachment.files.iter().enumerate() {
                        content.push(UserContent::text(format!("Page {}", index + 1)));
                        content.push(self.image_content(conversation_id, file)?);
                    }
                }
            }
        }
        let content = OneOrMany::many(content).map_err(|_| {
            StorageError::InvalidData("a user message must contain text or an attachment".into())
        })?;
        Ok(Message::User { content })
    }

    pub fn remove_attachments(
        &self,
        conversation_id: &str,
        attachments: &[Attachment],
    ) -> Result<()> {
        let _guard = self.lock()?;
        for attachment in attachments {
            let path = self
                .conversation_dir(conversation_id)?
                .join("attachments")
                .join(&attachment.id);
            match fs::remove_dir_all(path) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    }

    pub fn attachment_path(&self, conversation_id: &str, relative: &str) -> Result<PathBuf> {
        let relative = Path::new(relative);
        if relative.is_absolute()
            || relative
                .components()
                .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(StorageError::InvalidData(format!(
                "invalid attachment path: {}",
                relative.display()
            )));
        }
        Ok(self.conversation_dir(conversation_id)?.join(relative))
    }

    fn image_content(&self, conversation_id: &str, file: &AttachmentFile) -> Result<UserContent> {
        let media_type = match file.media_type.as_str() {
            "image/jpeg" => ImageMediaType::JPEG,
            "image/png" => ImageMediaType::PNG,
            "image/gif" => ImageMediaType::GIF,
            "image/webp" => ImageMediaType::WEBP,
            value => {
                return Err(StorageError::InvalidData(format!(
                    "unsupported image media type: {value}"
                )));
            }
        };
        let bytes = fs::read(self.attachment_path(conversation_id, &file.path)?)?;
        Ok(UserContent::image_base64(
            BASE64.encode(bytes),
            Some(media_type),
            None,
        ))
    }

    pub(super) fn copy_attachment_assets(
        &self,
        source_conversation_id: &str,
        destination: &ConversationFile,
    ) -> Result<()> {
        for attachment in destination
            .turns
            .iter()
            .flat_map(|turn| &turn.user.attachments)
        {
            for file in &attachment.files {
                let source = self.attachment_path(source_conversation_id, &file.path)?;
                let target = self.attachment_path(&destination.conversation.id, &file.path)?;
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(source, target)?;
            }
        }
        Ok(())
    }
}
