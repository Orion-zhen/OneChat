use std::{
    collections::HashSet,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use crate::domain::{
    AutoTitleState, Conversation, ConversationSession, RequestInfo, Turn, UserMessage,
    active_turns, new_id,
};

use super::{Result, Storage, StorageError, codec::write_json, conflict, missing};

mod attachments;
mod generation;
mod migration;
mod state;

pub(super) use state::Sessions;

impl Storage {
    pub fn load_conversation_turns(&self, conversation_id: &str) -> Result<Vec<Turn>> {
        Ok(self.load_conversation(conversation_id)?.turns)
    }

    pub fn export_conversation_archive(
        &self,
        conversation_id: &str,
        markdown: &str,
        destination: &Path,
    ) -> Result<()> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let source_json =
            serde_json::to_vec_pretty(&self.session_for_use(sessions, conversation_id)?)?;
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
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let path = self.conversation_path(&conversation.id)?;
        if sessions.contains(&conversation.id) || path.exists() {
            return Err(conflict("conversation", &conversation.id));
        }
        self.commit_session(sessions, &ConversationSession::new(conversation.clone()))
    }

    pub fn update_conversation(&self, conversation: &Conversation) -> Result<()> {
        self.update_session(&conversation.id, |session| {
            session.update_conversation(conversation);
            Ok(())
        })
        .map(|_| ())
    }

    pub fn claim_auto_title(&self, conversation_id: &str) -> Result<bool> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        if !sessions.contains(conversation_id) {
            return Ok(false);
        }
        let mut file = sessions.get(conversation_id)?.clone();
        if file.conversation.auto_title_state != AutoTitleState::Pending {
            return Ok(false);
        }
        file.conversation.auto_title_state = AutoTitleState::Running;
        self.commit_session(sessions, &file)?;
        Ok(true)
    }

    pub fn restart_auto_title(
        &self,
        conversation_id: &str,
    ) -> Result<Option<Vec<(UserMessage, String)>>> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        if !sessions.contains(conversation_id) {
            return Ok(None);
        }
        let mut file = sessions.get(conversation_id)?.clone();
        if file.conversation.auto_title_state == AutoTitleState::Running {
            return Ok(None);
        }
        let conversation = active_turns(&file.turns)
            .into_iter()
            .filter_map(|turn| {
                let text = turn.continuation_response()?.output_text();
                (!text.trim().is_empty()).then(|| (turn.user.clone(), text))
            })
            .take(3)
            .collect::<Vec<_>>();
        if conversation.is_empty() {
            return Ok(None);
        }
        file.conversation.auto_title_state = AutoTitleState::Running;
        self.commit_session(sessions, &file)?;
        Ok(Some(conversation))
    }

    pub fn finish_auto_title(
        &self,
        conversation_id: &str,
        title: Option<&str>,
    ) -> Result<Option<Conversation>> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        if !sessions.contains(conversation_id) {
            return Ok(None);
        }
        let mut file = sessions.get(conversation_id)?.clone();
        if file.conversation.auto_title_state != AutoTitleState::Running {
            return Ok(None);
        }
        if let Some(title) = title.map(str::trim).filter(|title| !title.is_empty()) {
            file.conversation.title = title.to_string();
        }
        file.conversation.auto_title_state = AutoTitleState::Finished;
        self.commit_session(sessions, &file)?;
        Ok(Some(file.conversation))
    }

    pub fn fork_conversation(
        &self,
        source_conversation_id: &str,
        response_id: &str,
        conversation: &Conversation,
    ) -> Result<ConversationSession> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let path = self.conversation_path(&conversation.id)?;
        if sessions.contains(&conversation.id) || path.exists() {
            return Err(conflict("conversation", &conversation.id));
        }

        let source = self.session_for_use(sessions, source_conversation_id)?;
        let (turns, requests) = fork_path(&source, response_id, &conversation.id)?;
        let mut conversation = conversation.clone();
        conversation.auto_title_state = AutoTitleState::Finished;
        let file = ConversationSession {
            conversation,
            turns,
            requests,
        };
        if let Err(error) = self
            .copy_attachment_assets(source_conversation_id, &file)
            .and_then(|()| self.commit_session(sessions, &file))
        {
            let _ = fs::remove_dir_all(self.conversation_dir(&file.conversation.id)?);
            return Err(error);
        }
        Ok(file)
    }

    pub fn delete_conversation(&self, id: &str) -> Result<()> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        sessions.get(id)?;
        fs::remove_dir_all(self.conversation_dir(id)?)?;
        sessions.remove(id);
        Ok(())
    }

    pub fn clear_conversation_context(&self, conversation_id: &str) -> Result<ConversationSession> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let mut file = sessions.get(conversation_id)?.clone();
        file.clear();
        self.commit_session(sessions, &file)?;
        let attachments = self.conversation_dir(conversation_id)?.join("attachments");
        match fs::remove_dir_all(attachments) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(file)
    }

    pub(super) fn clear_conversation_models(
        &self,
        sessions: &mut Sessions,
        removed_models: &[String],
    ) -> Result<()> {
        let affected = sessions
            .values()
            .filter(|file| {
                file.conversation
                    .model_id
                    .as_ref()
                    .is_some_and(|id| removed_models.contains(id))
            })
            .cloned()
            .collect::<Vec<_>>();
        for mut file in affected {
            file.conversation.model_id = None;
            self.commit_session(sessions, &file)?;
        }
        Ok(())
    }

    pub fn update_session(
        &self,
        conversation_id: &str,
        edit: impl FnOnce(&mut ConversationSession) -> std::result::Result<(), String>,
    ) -> Result<ConversationSession> {
        let mut state = self.lock()?;
        let sessions = self.sessions(&mut state)?;
        let mut session = sessions.get(conversation_id)?.clone();
        edit(&mut session).map_err(StorageError::InvalidData)?;
        self.commit_session(sessions, &session)?;
        Ok(session)
    }

    pub fn load_conversation(&self, conversation_id: &str) -> Result<ConversationSession> {
        let mut state = self.lock()?;
        self.session_for_use(self.sessions(&mut state)?, conversation_id)
    }

    pub(super) fn write_conversation(&self, file: &ConversationSession) -> Result<()> {
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
    source: &ConversationSession,
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
