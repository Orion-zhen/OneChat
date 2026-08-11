use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{
    AutoTitleState, Conversation, RequestInfo, Turn, active_turns, new_id, now_timestamp,
};

use super::{
    Result, Storage, StorageError,
    codec::{read_jsonc, write_json},
    conflict, missing,
};

mod attachments;
mod generation;

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
        self.edit_conversation(&conversation.id, |file| {
            let title = file.conversation.title.clone();
            let auto_title_state = file.conversation.auto_title_state;
            let updated_at = file.conversation.updated_at.max(conversation.updated_at);
            file.conversation = conversation.clone();
            file.conversation.title = title;
            file.conversation.auto_title_state = auto_title_state;
            file.conversation.updated_at = updated_at;
            Ok(())
        })
    }

    pub fn rename_conversation(&self, conversation_id: &str, title: &str) -> Result<()> {
        let title = title.trim();
        if title.is_empty() {
            return Err(StorageError::InvalidData(
                "conversation title cannot be empty".into(),
            ));
        }
        self.edit_conversation(conversation_id, |file| {
            file.conversation.title = title.to_string();
            file.conversation.auto_title_state = AutoTitleState::Finished;
            file.conversation.updated_at = now_timestamp();
            Ok(())
        })
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

    pub fn restart_auto_title(&self, conversation_id: &str) -> Result<Option<(String, String)>> {
        let _guard = self.lock()?;
        if !self.conversation_path(conversation_id)?.exists() {
            return Ok(None);
        }
        let mut file = self.read_conversation(conversation_id)?;
        if file.conversation.auto_title_state == AutoTitleState::Running {
            return Ok(None);
        }
        let Some((user_message, assistant_response)) =
            active_turns(&file.turns).first().and_then(|turn| {
                turn.continuation_response()
                    .filter(|response| !response.content.trim().is_empty())
                    .map(|response| (turn.user.content.clone(), response.content.clone()))
            })
        else {
            return Ok(None);
        };
        file.conversation.auto_title_state = AutoTitleState::Running;
        self.write_conversation(&file)?;
        Ok(Some((user_message, assistant_response)))
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

    pub fn set_continuation_response(
        &self,
        conversation_id: &str,
        turn_id: &str,
        response_id: &str,
    ) -> Result<()> {
        self.edit_conversation(conversation_id, |file| {
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
            if !response.is_usable_as_context() {
                return Err(StorageError::InvalidData(
                    "only a completed response can be used as context".into(),
                ));
            }
            turn.continuation_response_id = Some(response_id.to_string());
            Ok(())
        })
    }

    pub fn select_user_branch(&self, conversation_id: &str, turn_id: &str) -> Result<()> {
        self.edit_conversation(conversation_id, |file| {
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
            Ok(())
        })
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

    fn edit_conversation(
        &self,
        conversation_id: &str,
        edit: impl FnOnce(&mut ConversationFile) -> Result<()>,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let mut file = self.read_conversation(conversation_id)?;
        edit(&mut file)?;
        self.write_conversation(&file)
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
    if !terminal_response.is_usable_as_context() {
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
