use std::fs;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rig_core::{
    OneOrMany,
    completion::Message,
    message::{ImageMediaType, UserContent},
};

use crate::{
    domain::{AttachmentFile, AttachmentFileKind, AttachmentKind, UserMessage},
    storage::{Result, Storage, StorageError},
};

impl Storage {
    pub fn message_for_user(
        &self,
        conversation_id: &str,
        user: &UserMessage,
        include_document_images: bool,
    ) -> Result<Message> {
        let _guard = self.lock()?;
        let mut content = Vec::new();
        if !user.content.trim().is_empty() {
            content.push(UserContent::text(user.content.clone()));
        }
        for attachment in &user.attachments {
            attachment.validate_files().map_err(|error| {
                StorageError::InvalidData(format!(
                    "invalid attachment {}: {error}",
                    attachment.name
                ))
            })?;

            match attachment.kind {
                AttachmentKind::Text => {
                    let file = attachment
                        .files
                        .iter()
                        .find(|file| file.kind == AttachmentFileKind::Text)
                        .expect("validated text attachment");
                    let text =
                        fs::read_to_string(self.attachment_path(conversation_id, &file.path)?)?;
                    content.push(UserContent::text(format!(
                        "<attachment name=\"{}\">\n{}\n</attachment>",
                        attachment.name, text
                    )));
                }
                AttachmentKind::Image => {
                    let file = attachment
                        .files
                        .iter()
                        .find(|file| file.kind == AttachmentFileKind::Image)
                        .expect("validated image attachment");
                    content.push(UserContent::text(format!(
                        "Image attachment: {}",
                        attachment.name
                    )));
                    content.push(self.image_content(conversation_id, file)?);
                }
                AttachmentKind::Pdf => {
                    let mut files = attachment.files.iter().collect::<Vec<_>>();
                    files.sort_by(|left, right| left.name.cmp(&right.name));
                    content.push(UserContent::text(format!(
                        "PDF attachment: {} ({} pages)",
                        attachment.name,
                        files.len()
                    )));
                    for (index, file) in files.into_iter().enumerate() {
                        content.push(UserContent::text(format!("Page {}", index + 1)));
                        content.push(self.image_content(conversation_id, file)?);
                    }
                }
                AttachmentKind::Document => {
                    let markdown = attachment
                        .files
                        .iter()
                        .find(|file| file.kind == AttachmentFileKind::Text)
                        .expect("validated document attachment");
                    let markdown =
                        fs::read_to_string(self.attachment_path(conversation_id, &markdown.path)?)?;
                    content.push(UserContent::text(format!(
                        "<attachment name=\"{}\">\n{}\n</attachment>",
                        attachment.name, markdown
                    )));
                    if include_document_images {
                        let mut images = attachment
                            .files
                            .iter()
                            .filter(|file| file.kind == AttachmentFileKind::Image)
                            .collect::<Vec<_>>();
                        images.sort_by(|left, right| left.name.cmp(&right.name));
                        for image in images {
                            content.push(UserContent::text(format!(
                                "Embedded image from {}: {}",
                                attachment.name, image.name
                            )));
                            content.push(self.image_content(conversation_id, image)?);
                        }
                    }
                }
            }
        }
        let content = OneOrMany::many(content).map_err(|_| {
            StorageError::InvalidData("a user message must contain text or an attachment".into())
        })?;
        Ok(Message::User { content })
    }

    fn image_content(&self, conversation_id: &str, file: &AttachmentFile) -> Result<UserContent> {
        if file.kind != AttachmentFileKind::Image {
            return Err(StorageError::InvalidData(format!(
                "attachment file is not an image: {}",
                file.name
            )));
        }
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
}
