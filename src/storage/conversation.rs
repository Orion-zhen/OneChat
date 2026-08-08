use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use rig_core::{
    OneOrMany,
    completion::Message,
    message::{ImageMediaType, UserContent},
};
use serde::{Deserialize, Serialize};

use crate::domain::{
    AssistantResponse, Attachment, AttachmentDraft, AttachmentFile, AttachmentKind, AutoTitleState,
    Conversation, MessageStatus, RequestInfo, Turn, UserMessage, active_turns, new_id,
    now_timestamp,
};

use super::codec::{read_jsonc, write_json};
use super::{Result, Storage, StorageError, conflict, missing};

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ConversationFile {
    #[serde(flatten)]
    pub(super) conversation: Conversation,
    pub(super) turns: Vec<Turn>,
    #[serde(default)]
    pub(super) requests: Vec<RequestInfo>,
}

impl Storage {
    pub fn insert_conversation(&self, conversation: &Conversation) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.conversation_path(&conversation.id)?;
        if path.exists() {
            return Err(conflict("conversation", &conversation.id));
        }
        self.write_conversation(&ConversationFile {
            conversation: conversation.clone(),
            turns: Vec::new(),
            requests: Vec::new(),
        })
    }

    pub fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(&conversation.id)?;
        let title = file.conversation.title.clone();
        let auto_title_state = file.conversation.auto_title_state;
        let updated_at = file.conversation.updated_at.max(conversation.updated_at);
        file.conversation = conversation.clone();
        file.conversation.title = title;
        file.conversation.auto_title_state = auto_title_state;
        file.conversation.updated_at = updated_at;
        self.write_conversation(&file)
    }

    pub fn rename_conversation(&self, conversation_id: &str, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(StorageError::InvalidData(
                "conversation title cannot be empty".into(),
            ));
        }
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        file.conversation.title = title.to_string();
        file.conversation.auto_title_state = AutoTitleState::Finished;
        file.conversation.updated_at = now_timestamp();
        self.write_conversation(&file)
    }

    pub fn claim_auto_title(&self, conversation_id: &str) -> Result<bool> {
        let _guard = self.lock()?;
        if !self.conversation_path(conversation_id)?.exists() {
            return Ok(false);
        }
        let mut file = self.read_conversation(conversation_id)?;
        if file.conversation.auto_title_state != AutoTitleState::Pending {
            return Ok(false);
        }
        file.conversation.auto_title_state = AutoTitleState::Running;
        self.write_conversation(&file)?;
        Ok(true)
    }

    pub fn finish_auto_title(&self, conversation_id: &str, title: Option<&str>) -> Result<bool> {
        let _guard = self.lock()?;
        if !self.conversation_path(conversation_id)?.exists() {
            return Ok(false);
        }
        let mut file = self.read_conversation(conversation_id)?;
        if file.conversation.auto_title_state != AutoTitleState::Running {
            return Ok(false);
        }
        if let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) {
            file.conversation.title = title.to_string();
            file.conversation.updated_at = now_timestamp();
        }
        file.conversation.auto_title_state = AutoTitleState::Finished;
        self.write_conversation(&file)?;
        Ok(true)
    }

    pub fn fork_conversation(
        &self,
        source_conversation_id: &str,
        response_id: &str,
        conversation: &Conversation,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.conversation_path(&conversation.id)?;
        if path.exists() {
            return Err(conflict("conversation", &conversation.id));
        }

        let source = self.read_conversation(source_conversation_id)?;
        let (turns, requests) = fork_path(&source, response_id, &conversation.id)?;
        let mut conversation = conversation.clone();
        conversation.auto_title_state = AutoTitleState::Finished;
        let file = ConversationFile {
            conversation,
            turns,
            requests,
        };
        self.write_conversation(&file)?;
        if let Err(error) = self.copy_attachment_assets(source_conversation_id, &file) {
            let _ = fs::remove_dir_all(self.conversation_dir(&file.conversation.id)?);
            return Err(error);
        }
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let _guard = self.lock()?;
        if !self.conversation_path(id)?.exists() {
            return Err(missing("conversation", id));
        }
        fs::remove_dir_all(self.conversation_dir(id)?)?;
        Ok(())
    }

    pub fn clear_conversation_context(&self, conversation_id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        file.turns.clear();
        file.requests.clear();
        self.write_conversation(&file)?;
        let attachments = self.conversation_dir(conversation_id)?.join("attachments");
        match fs::remove_dir_all(attachments) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }

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

    pub fn update_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        let stored = response_mut(&mut file, turn_id, &response.id)?;
        *stored = response.clone();
        self.write_conversation(&file)
    }

    pub fn set_continuation_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response_id: &str,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        if !active_turns(&file.turns)
            .iter()
            .any(|turn| turn.id == turn_id)
        {
            return Err(StorageError::InvalidData(
                "only an active turn can change context".into(),
            ));
        }
        let turn = file
            .turns
            .iter_mut()
            .find(|turn| turn.id == turn_id)
            .ok_or_else(|| missing("turn", turn_id))?;
        let response = turn
            .response(response_id)
            .ok_or_else(|| missing("response", response_id))?;
        if response.status != MessageStatus::Completed || response.content.is_empty() {
            return Err(StorageError::InvalidData(
                "only a completed response can be used as context".into(),
            ));
        }
        turn.continuation_response_id = Some(response_id.to_string());
        self.write_conversation(&file)
    }

    pub fn begin_turn(&self, turn: &Turn, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        let response = turn
            .response(&request.response_id)
            .ok_or_else(|| missing("response", &request.response_id))?;
        ensure_request(request, &turn.id, response)?;
        let mut file = self.read_conversation(&request.conversation_id)?;
        if file.turns.iter().any(|stored| stored.id == turn.id) {
            return Err(conflict("turn", &turn.id));
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        if let Some(parent_response_id) = turn.parent_response_id.as_deref()
            && !file
                .turns
                .iter()
                .any(|stored| stored.response(parent_response_id).is_some())
        {
            return Err(missing("parent response", parent_response_id));
        }
        for sibling in &mut file.turns {
            if sibling.parent_response_id == turn.parent_response_id {
                sibling.selected = false;
            }
        }
        let mut turn = turn.clone();
        turn.selected = true;
        file.conversation.updated_at = turn.user.created_at;
        file.turns.push(turn);
        file.requests.push(request.clone());
        self.write_conversation(&file)
    }

    pub fn select_user_branch(&self, conversation_id: &str, turn_id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        let parent_response_id = file
            .turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .ok_or_else(|| missing("turn", turn_id))?
            .parent_response_id
            .clone();
        for turn in &mut file.turns {
            if turn.parent_response_id == parent_response_id {
                turn.selected = turn.id == turn_id;
            }
        }
        self.write_conversation(&file)
    }

    pub fn begin_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, turn_id, response)?;
        if request.conversation_id != conversation_id {
            return Err(StorageError::InvalidData(
                "response and request belong to different conversations".into(),
            ));
        }
        let mut file = self.read_conversation(conversation_id)?;
        let turn = file
            .turns
            .iter_mut()
            .find(|turn| turn.id == turn_id)
            .ok_or_else(|| missing("turn", turn_id))?;
        if turn.responses.len() >= 4 {
            return Err(StorageError::InvalidData(
                "a turn can contain at most four responses".into(),
            ));
        }
        if turn
            .responses
            .iter()
            .any(|stored| stored.id == response.id || stored.model_id == response.model_id)
        {
            return Err(conflict("response model", &response.model_id));
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        turn.responses.push(response.clone());
        file.requests.push(request.clone());
        file.conversation.updated_at = response.created_at;
        self.write_conversation(&file)
    }

    pub fn begin_regeneration(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, turn_id, response)?;
        if request.conversation_id != conversation_id {
            return Err(StorageError::InvalidData(
                "response and request belong to different conversations".into(),
            ));
        }
        let mut file = self.read_conversation(conversation_id)?;
        *response_mut(&mut file, turn_id, &response.id)? = response.clone();
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        file.requests.push(request.clone());
        file.conversation.updated_at = response.updated_at;
        self.write_conversation(&file)
    }

    pub fn persist_generation(
        &self,
        response: &AssistantResponse,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_request(request, &request.turn_id, response)?;
        let mut file = self.read_conversation(&request.conversation_id)?;
        let turn = file
            .turns
            .iter_mut()
            .find(|turn| turn.id == request.turn_id)
            .ok_or_else(|| missing("turn", &request.turn_id))?;
        let stored_response = turn
            .responses
            .iter_mut()
            .find(|stored| stored.id == response.id)
            .ok_or_else(|| missing("response", &response.id))?;
        *stored_response = response.clone();
        let continuation_is_unusable = turn
            .continuation_response_id
            .as_deref()
            .and_then(|id| turn.response(id))
            .is_none_or(|response| {
                response.status != MessageStatus::Completed || response.content.is_empty()
            });
        if response.status == MessageStatus::Completed
            && !response.content.is_empty()
            && continuation_is_unusable
        {
            turn.continuation_response_id = Some(response.id.clone());
        }
        let stored_request = file
            .requests
            .iter_mut()
            .find(|stored| stored.id == request.id)
            .ok_or_else(|| missing("request", &request.id))?;
        *stored_request = request.clone();
        self.write_conversation(&file)
    }

    pub(super) fn clear_conversation_models(&self, removed_models: &[String]) -> Result<()> {
        if removed_models.is_empty() {
            return Ok(());
        }
        for mut file in self.read_conversations()? {
            if file
                .conversation
                .model_id
                .as_ref()
                .is_some_and(|id| removed_models.contains(id))
            {
                file.conversation.model_id = None;
                self.write_conversation(&file)?;
            }
        }
        Ok(())
    }

    pub(super) fn read_conversations(&self) -> Result<Vec<ConversationFile>> {
        let mut directories = fs::read_dir(&self.conversations_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        directories.retain(|path| path.is_dir());
        directories.sort();

        let mut files = Vec::with_capacity(directories.len());
        for directory in directories {
            let Some(id) = directory.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let path = directory.join(format!("{id}.json"));
            if !path.is_file() {
                continue;
            }
            let file: ConversationFile = match read_jsonc(&path) {
                Ok(file) => file,
                Err(StorageError::Parse { .. }) => continue,
                Err(error) => return Err(error),
            };
            let expected_path = self.conversation_path(&file.conversation.id)?;
            if expected_path != path {
                return Err(StorageError::InvalidData(format!(
                    "conversation id {} does not match file {}",
                    file.conversation.id,
                    path.display()
                )));
            }
            files.push(file);
        }
        Ok(files)
    }

    fn read_conversation(&self, id: &str) -> Result<ConversationFile> {
        let path = self.conversation_path(id)?;
        let file: ConversationFile = match read_jsonc(&path) {
            Ok(file) => file,
            Err(StorageError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(missing("conversation", id));
            }
            Err(error) => return Err(error),
        };
        if file.conversation.id != id {
            return Err(StorageError::InvalidData(format!(
                "conversation id {} does not match file {}",
                file.conversation.id,
                path.display()
            )));
        }
        Ok(file)
    }

    pub(super) fn write_conversation(&self, file: &ConversationFile) -> Result<()> {
        let path = self.conversation_path(&file.conversation.id)?;
        write_json(&path, file)
    }

    fn conversation_path(&self, id: &str) -> Result<PathBuf> {
        Ok(self.conversation_dir(id)?.join(format!("{id}.json")))
    }

    fn conversation_dir(&self, id: &str) -> Result<PathBuf> {
        validate_component("conversation id", id)?;
        Ok(self.conversations_dir.join(id))
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

    fn copy_attachment_assets(
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

fn validate_component(kind: &str, value: &str) -> Result<()> {
    if value.is_empty()
        || Path::new(value).components().count() != 1
        || Path::new(value)
            .file_name()
            .is_none_or(|name| name != value)
    {
        return Err(StorageError::InvalidData(format!(
            "invalid {kind}: {value}"
        )));
    }
    Ok(())
}

fn fork_path(
    source: &ConversationFile,
    response_id: &str,
    conversation_id: &str,
) -> Result<(Vec<Turn>, Vec<RequestInfo>)> {
    let mut source_path = Vec::new();
    let mut visited = HashSet::new();
    let mut current_response_id = response_id.to_string();

    loop {
        if !visited.insert(current_response_id.clone()) {
            return Err(StorageError::InvalidData(
                "conversation history contains a response cycle".into(),
            ));
        }
        let (turn, response) = source
            .turns
            .iter()
            .find_map(|turn| {
                turn.response(&current_response_id)
                    .map(|response| (turn, response))
            })
            .ok_or_else(|| missing("response", &current_response_id))?;
        source_path.push((turn, response));
        let Some(parent_response_id) = turn.parent_response_id.as_ref() else {
            break;
        };
        current_response_id.clone_from(parent_response_id);
    }

    let Some((_, terminal_response)) = source_path.first() else {
        return Err(missing("response", response_id));
    };
    if terminal_response.status != MessageStatus::Completed || terminal_response.content.is_empty()
    {
        return Err(StorageError::InvalidData(
            "only a completed response can be forked".into(),
        ));
    }

    source_path.reverse();
    let mut turns = Vec::with_capacity(source_path.len());
    let mut requests = Vec::with_capacity(source_path.len());
    let mut parent_response_id = None;

    for (source_turn, source_response) in source_path {
        let turn_id = new_id("turn");
        let response_id = new_id("response");
        let mut response = source_response.clone();
        response.id.clone_from(&response_id);
        response.request_id = source_response
            .request_id
            .as_deref()
            .and_then(|request_id| {
                source
                    .requests
                    .iter()
                    .find(|request| request.id == request_id)
            })
            .map(|source_request| {
                let mut request = source_request.clone();
                request.id = new_id("request");
                request.conversation_id = conversation_id.to_string();
                request.turn_id.clone_from(&turn_id);
                request.response_id.clone_from(&response_id);
                let request_id = request.id.clone();
                requests.push(request);
                request_id
            });

        let mut turn = source_turn.clone();
        turn.id.clone_from(&turn_id);
        turn.parent_response_id = parent_response_id;
        turn.selected = true;
        turn.user.id = new_id("message");
        turn.responses = vec![response];
        turn.continuation_response_id = Some(response_id.clone());
        parent_response_id = Some(response_id);
        turns.push(turn);
    }

    Ok((turns, requests))
}

fn response_mut<'a>(
    file: &'a mut ConversationFile,
    turn_id: &str,
    response_id: &str,
) -> Result<&'a mut AssistantResponse> {
    file.turns
        .iter_mut()
        .find(|turn| turn.id == turn_id)
        .ok_or_else(|| missing("turn", turn_id))?
        .responses
        .iter_mut()
        .find(|response| response.id == response_id)
        .ok_or_else(|| missing("response", response_id))
}

fn ensure_request(
    request: &RequestInfo,
    turn_id: &str,
    response: &AssistantResponse,
) -> Result<()> {
    if request.turn_id != turn_id
        || request.response_id != response.id
        || response.request_id.as_deref() != Some(&request.id)
    {
        return Err(StorageError::InvalidData(
            "generation records belong to different turns or responses".into(),
        ));
    }
    Ok(())
}
