use std::{
    collections::HashSet,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::domain::{
    AutoTitleState, Conversation, RequestInfo, Turn, UserMessage, active_turns, new_id,
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
    pub fn load_conversation_turns(&self, conversation_id: &str) -> Result<Vec<Turn>> {
        let _guard = self.lock()?;
        Ok(self.read_conversation(conversation_id)?.turns)
    }

    pub fn export_conversation_archive(
        &self,
        conversation_id: &str,
        markdown: &str,
        destination: &Path,
    ) -> Result<()> {
        let _guard = self.lock()?;
        let source_json = fs::read(self.conversation_path(conversation_id)?)?;
        let attachments_dir = self.conversation_dir(conversation_id)?.join("attachments");
        let attachment_files = files_below(&attachments_dir)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let file_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("conversation.zip");
        let temporary =
            destination.with_file_name(format!(".{file_name}.{}.tmp", new_id("export")));

        let result = write_archive(
            &temporary,
            &source_json,
            markdown.as_bytes(),
            &attachments_dir,
            &attachment_files,
        );
        if let Err(error) = result {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }

        #[cfg(windows)]
        if destination.exists() {
            fs::remove_file(destination)?;
        }
        if let Err(error) = fs::rename(&temporary, destination) {
            let _ = fs::remove_file(&temporary);
            return Err(error.into());
        }
        Ok(())
    }

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

    pub fn restart_auto_title(
        &self,
        conversation_id: &str,
    ) -> Result<Option<Vec<(UserMessage, String)>>> {
        let _guard = self.lock()?;
        if !self.conversation_path(conversation_id)?.exists() {
            return Ok(None);
        }
        let mut file = self.read_conversation(conversation_id)?;
        if file.conversation.auto_title_state == AutoTitleState::Running {
            return Ok(None);
        }
        let conversation = active_turns(&file.turns)
            .into_iter()
            .filter_map(|turn| {
                turn.continuation_response()
                    .filter(|response| !response.content.trim().is_empty())
                    .map(|response| (turn.user.clone(), response.content.clone()))
            })
            .take(3)
            .collect::<Vec<_>>();
        if conversation.is_empty() {
            return Ok(None);
        }
        file.conversation.auto_title_state = AutoTitleState::Running;
        self.write_conversation(&file)?;
        Ok(Some(conversation))
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

    pub fn select_turn_path(&self, conversation_id: &str, turn_id: &str) -> Result<()> {
        self.edit_conversation(conversation_id, |file| {
            let mut path = Vec::new();
            let mut current_id = turn_id.to_string();
            loop {
                let turn = file
                    .turns
                    .iter()
                    .find(|turn| turn.id == current_id)
                    .ok_or_else(|| missing("turn", &current_id))?;
                path.push((turn.id.clone(), turn.parent_response_id.clone()));
                let Some(parent_response_id) = &turn.parent_response_id else {
                    break;
                };
                current_id = file
                    .turns
                    .iter()
                    .find(|candidate| candidate.response(parent_response_id).is_some())
                    .ok_or_else(|| missing("parent response", parent_response_id))?
                    .id
                    .clone();
            }

            for (selected_id, parent_response_id) in path.iter().rev() {
                for turn in &mut file.turns {
                    if turn.parent_response_id == *parent_response_id {
                        turn.selected = turn.id == *selected_id;
                    }
                }
                if let Some(parent_response_id) = parent_response_id
                    && let Some(parent) = file
                        .turns
                        .iter_mut()
                        .find(|turn| turn.response(parent_response_id).is_some())
                {
                    parent.continuation_response_id = Some(parent_response_id.clone());
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

fn files_below(directory: &Path) -> Result<Vec<PathBuf>> {
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let mut pending = vec![directory.to_path_buf()];
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let file_type = entry.file_type()?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                files.push(entry.path());
            }
        }
    }
    files.sort();
    Ok(files)
}

fn write_archive(
    destination: &Path,
    conversation_json: &[u8],
    markdown: &[u8],
    attachments_dir: &Path,
    attachment_files: &[PathBuf],
) -> Result<()> {
    let file = fs::File::create(destination)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    archive
        .start_file("conversation.json", options)
        .map_err(zip_error)?;
    archive.write_all(conversation_json)?;
    archive
        .start_file("conversation.md", options)
        .map_err(zip_error)?;
    archive.write_all(markdown)?;

    for path in attachment_files {
        let relative = path.strip_prefix(attachments_dir).map_err(|_| {
            StorageError::InvalidData(format!(
                "attachment path is outside its conversation: {}",
                path.display()
            ))
        })?;
        let name = relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        archive
            .start_file(format!("attachments/{name}"), options)
            .map_err(zip_error)?;
        let mut attachment = fs::File::open(path)?;
        std::io::copy(&mut attachment, &mut archive)?;
    }

    archive.finish().map_err(zip_error)?;
    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> StorageError {
    StorageError::Io(std::io::Error::other(error))
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

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn complete_archive_contains_json_markdown_and_attachments() {
        let temporary = tempdir().unwrap();
        let storage = Storage::open(
            temporary.path().join("settings.jsonc"),
            temporary.path().join("state"),
        )
        .unwrap();
        let conversation = Conversation::new("Archive test", None, "private prompt");
        storage.insert_conversation(&conversation).unwrap();
        let attachments = storage
            .conversation_dir(&conversation.id)
            .unwrap()
            .join("attachments")
            .join("attachment-1");
        fs::create_dir_all(&attachments).unwrap();
        fs::write(attachments.join("notes.txt"), b"attachment contents").unwrap();

        let destination = temporary.path().join("export.zip");
        storage
            .export_conversation_archive(&conversation.id, "# Exported\n", &destination)
            .unwrap();

        let mut archive = zip::ZipArchive::new(fs::File::open(destination).unwrap()).unwrap();
        let mut json = String::new();
        archive
            .by_name("conversation.json")
            .unwrap()
            .read_to_string(&mut json)
            .unwrap();
        assert!(json.contains("Archive test"));
        let mut markdown = String::new();
        archive
            .by_name("conversation.md")
            .unwrap()
            .read_to_string(&mut markdown)
            .unwrap();
        assert_eq!(markdown, "# Exported\n");
        let mut attachment = String::new();
        archive
            .by_name("attachments/attachment-1/notes.txt")
            .unwrap()
            .read_to_string(&mut attachment)
            .unwrap();
        assert_eq!(attachment, "attachment contents");
    }
}
