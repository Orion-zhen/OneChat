use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{Conversation, Message, MessageRole, RequestInfo};

use super::codec::{read_jsonc, write_json};
use super::{Result, Storage, StorageError, conflict, missing};

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct ConversationFile {
    #[serde(flatten)]
    pub(super) conversation: Conversation,
    #[serde(default)]
    pub(super) messages: Vec<Message>,
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
            messages: Vec::new(),
            requests: Vec::new(),
        })
    }

    pub fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(&conversation.id)?;
        file.conversation = conversation.clone();
        self.write_conversation(&file)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let path = self.conversation_path(id)?;
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Err(missing("conversation", id))
            }
            Err(error) => Err(error.into()),
        }
    }

    pub fn clear_conversation_context(&self, conversation_id: &str) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        file.messages.clear();
        file.requests.clear();
        self.write_conversation(&file)
    }

    pub fn update_message(&self, message: &Message) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(&message.conversation_id)?;
        let stored = file
            .messages
            .iter_mut()
            .find(|item| item.id == message.id)
            .ok_or_else(|| missing("message", &message.id))?;
        *stored = message.clone();
        self.write_conversation(&file)
    }

    pub fn begin_generation(
        &self,
        user: &Message,
        assistant: &Message,
        request: &RequestInfo,
    ) -> Result<()> {
        let _guard = self.lock()?;
        ensure_same_conversation(user, assistant, request)?;
        let mut file = self.read_conversation(&user.conversation_id)?;
        for message in [user, assistant] {
            if file.messages.iter().any(|stored| stored.id == message.id) {
                return Err(conflict("message", &message.id));
            }
            file.messages.push(message.clone());
        }
        if file.requests.iter().any(|stored| stored.id == request.id) {
            return Err(conflict("request", &request.id));
        }
        file.requests.push(request.clone());
        file.conversation.updated_at = user.created_at;
        self.write_conversation(&file)
    }

    pub fn begin_regeneration(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        if assistant.conversation_id != request.conversation_id
            || assistant.id != request.assistant_message_id
        {
            return Err(StorageError::InvalidData(
                "regeneration records belong to different messages or conversations".into(),
            ));
        }
        let mut file = self.read_conversation(&assistant.conversation_id)?;
        let stored = file
            .messages
            .iter_mut()
            .find(|message| message.id == assistant.id && message.role == MessageRole::Assistant)
            .ok_or_else(|| missing("assistant message", &assistant.id))?;
        *stored = assistant.clone();
        file.requests
            .retain(|stored| stored.assistant_message_id != assistant.id);
        file.requests.push(request.clone());
        file.conversation.updated_at = assistant.updated_at;
        self.write_conversation(&file)
    }

    pub fn persist_generation(&self, assistant: &Message, request: &RequestInfo) -> Result<()> {
        let _guard = self.lock()?;
        if assistant.conversation_id != request.conversation_id
            || assistant.id != request.assistant_message_id
        {
            return Err(StorageError::InvalidData(
                "generation records belong to different messages or conversations".into(),
            ));
        }
        let mut file = self.read_conversation(&assistant.conversation_id)?;
        let stored_message = file
            .messages
            .iter_mut()
            .find(|message| message.id == assistant.id)
            .ok_or_else(|| missing("message", &assistant.id))?;
        *stored_message = assistant.clone();
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
        let mut paths = fs::read_dir(&self.conversations_dir)?
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<std::io::Result<Vec<_>>>()?;
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        paths.sort();
        paths
            .into_iter()
            .map(|path| {
                let file: ConversationFile = read_jsonc(&path)?;
                let expected_path = self.conversation_path(&file.conversation.id)?;
                if expected_path != path {
                    return Err(StorageError::InvalidData(format!(
                        "conversation id {} does not match file {}",
                        file.conversation.id,
                        path.display()
                    )));
                }
                Ok(file)
            })
            .collect()
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
        if id.is_empty()
            || Path::new(id).components().count() != 1
            || Path::new(id).file_name().is_none_or(|name| name != id)
        {
            return Err(StorageError::InvalidData(format!(
                "invalid conversation id: {id}"
            )));
        }
        Ok(self.conversations_dir.join(format!("{id}.json")))
    }
}

fn ensure_same_conversation(
    user: &Message,
    assistant: &Message,
    request: &RequestInfo,
) -> Result<()> {
    if user.conversation_id != assistant.conversation_id
        || user.conversation_id != request.conversation_id
        || assistant.id != request.assistant_message_id
    {
        return Err(StorageError::InvalidData(
            "generation records belong to different messages or conversations".into(),
        ));
    }
    Ok(())
}
