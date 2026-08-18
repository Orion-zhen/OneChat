use std::fs;

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rig_core::{
    completion::Message,
    message::{AudioMediaType, ImageMediaType, UserContent},
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
        Self::message_for_user_with(user, include_document_images, |file| {
            Ok(fs::read(
                self.attachment_path(conversation_id, &file.path)?,
            )?)
        })
    }

    pub fn message_for_user_with(
        user: &UserMessage,
        include_document_images: bool,
        read_file: impl Fn(&AttachmentFile) -> Result<Vec<u8>>,
    ) -> Result<Message> {
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
                    let text = attachment_text(read_file(file)?, &attachment.name)?;
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
                    content.push(image_content(file, read_file(file)?)?);
                }
                AttachmentKind::Audio => {
                    let file = attachment
                        .files
                        .iter()
                        .find(|file| file.kind == AttachmentFileKind::Audio)
                        .expect("validated audio attachment");
                    content.push(UserContent::text(format!(
                        "Audio attachment: {}",
                        attachment.name
                    )));
                    content.push(audio_content(file, read_file(file)?)?);
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
                        content.push(image_content(file, read_file(file)?)?);
                    }
                }
                AttachmentKind::Document => {
                    let markdown = attachment
                        .files
                        .iter()
                        .find(|file| file.kind == AttachmentFileKind::Text)
                        .expect("validated document attachment");
                    let markdown = attachment_text(read_file(markdown)?, &attachment.name)?;
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
                            content.push(image_content(image, read_file(image)?)?);
                        }
                    }
                }
            }
        }
        if content.is_empty() {
            return Err(StorageError::InvalidData(
                "a user message must contain text or an attachment".into(),
            ));
        }
        Ok(Message::User { content })
    }
}

fn attachment_text(bytes: Vec<u8>, name: &str) -> Result<String> {
    String::from_utf8(bytes)
        .map_err(|_| StorageError::InvalidData(format!("attachment is not valid UTF-8: {name}")))
}

fn audio_content(file: &AttachmentFile, bytes: Vec<u8>) -> Result<UserContent> {
    if file.kind != AttachmentFileKind::Audio {
        return Err(StorageError::InvalidData(format!(
            "attachment file is not audio: {}",
            file.name
        )));
    }
    let media_type = match file.media_type.as_str() {
        "audio/wav" => AudioMediaType::WAV,
        "audio/mpeg" => AudioMediaType::MP3,
        value => {
            return Err(StorageError::InvalidData(format!(
                "unsupported audio media type: {value}"
            )));
        }
    };
    Ok(UserContent::audio(BASE64.encode(bytes), Some(media_type)))
}

fn image_content(file: &AttachmentFile, bytes: Vec<u8>) -> Result<UserContent> {
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
    Ok(UserContent::image_base64(
        BASE64.encode(bytes),
        Some(media_type),
        None,
    ))
}
