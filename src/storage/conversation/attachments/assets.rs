use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    domain::{Attachment, AttachmentDraft, AttachmentFile},
    storage::{Result, Storage, StorageError, conflict, missing},
};

use super::super::{ConversationFile, validate_component};

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
                draft.validate_files().map_err(|error| {
                    StorageError::InvalidData(format!("invalid attachment {}: {error}", draft.name))
                })?;

                let mut names = HashSet::with_capacity(draft.files.len());
                for file in &draft.files {
                    validate_component("attachment file name", &file.name)?;
                    if !names.insert(file.name.as_str()) {
                        return Err(StorageError::InvalidData(format!(
                            "duplicate attachment file name: {}",
                            file.name
                        )));
                    }
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
                for file in &draft.files {
                    fs::write(directory.join(&file.name), &file.bytes)?;
                    files.push(AttachmentFile {
                        name: file.name.clone(),
                        kind: file.kind,
                        path: format!("attachments/{}/{}", draft.id, file.name),
                        media_type: file.media_type.clone(),
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

    pub(in crate::storage::conversation) fn copy_attachment_assets(
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
